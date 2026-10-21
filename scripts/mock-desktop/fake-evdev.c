// LD_PRELOAD shim that gives a process fake /dev/uinput and /dev/input/event*
// nodes, for containers without the kernel modules. Every node is a Unix
// socket served by fake-input.py (see docs/mock-desktop.md):
//
//   /dev/uinput          -> $FAKE_EVDEV_DIR/uinput.sock; written events are
//                           replayed into the desktop by the broker.
//   /dev/input/eventN    -> $FAKE_EVDEV_DIR/eventN.sock; the broker writes the
//                           device's events, EVIOCGRAB is sent back to it.
//   opendir(/dev/input)  -> $FAKE_EVDEV_DIR/devinput (one empty file per node)
//
// Capability ioctls are answered from $FAKE_EVDEV_DIR/eventN.conf and the held
// keys (EVIOCGKEY) from eventN.held, both kept by the broker.
//
// Build: cc -shared -fPIC -O2 -o fake-evdev.so fake-evdev.c -ldl
#define _GNU_SOURCE
#include <dirent.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

#define MAX_FDS 4096
enum { NONE, UINPUT, EVDEV };
static unsigned char kinds[MAX_FDS];
static int nodes[MAX_FDS];

static const char *dir(void) {
    const char *d = getenv("FAKE_EVDEV_DIR");
    return d && *d ? d : NULL;
}

// Which fake node `path` names: -1 for /dev/uinput, N for /dev/input/eventN,
// -2 for anything else.
static int classify(const char *path) {
    if (!path || !dir()) return -2;
    if (strcmp(path, "/dev/uinput") == 0 || strcmp(path, "/dev/input/uinput") == 0) return -1;
    const char *prefix = "/dev/input/event";
    size_t n = strlen(prefix);
    if (strncmp(path, prefix, n) == 0 && path[n]) {
        char *end;
        long idx = strtol(path + n, &end, 10);
        if (!*end && idx >= 0 && idx < 1000) return (int)idx;
    }
    return -2;
}

static int connect_node(int node) {
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    if (node < 0)
        snprintf(addr.sun_path, sizeof addr.sun_path, "%s/uinput.sock", dir());
    else
        snprintf(addr.sun_path, sizeof addr.sun_path, "%s/event%d.sock", dir(), node);
    int fd = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;
    if (connect(fd, (struct sockaddr *)&addr, sizeof addr) < 0) {
        int err = errno == ECONNREFUSED ? ENODEV : ENOENT;
        close(fd);
        errno = err;
        return -1;
    }
    if (fd < MAX_FDS) {
        kinds[fd] = node < 0 ? UINPUT : EVDEV;
        nodes[fd] = node;
    }
    return fd;
}

static int (*real_open)(const char *, int, ...);
static int (*real_open64)(const char *, int, ...);
static int (*real_openat)(int, const char *, int, ...);
static int (*real_openat64)(int, const char *, int, ...);
static int (*real_close)(int);
static ssize_t (*real_write)(int, const void *, size_t);
static int (*real_ioctl)(int, unsigned long, ...);
static DIR *(*real_opendir)(const char *);

#define REAL(name) (real_##name ? real_##name : (real_##name = dlsym(RTLD_NEXT, #name)))

#define OPEN_BODY(call)                                         \
    int node = classify(path);                                  \
    if (node != -2) return connect_node(node);                  \
    mode_t mode = 0;                                            \
    if (flags & (O_CREAT | O_TMPFILE)) {                        \
        va_list ap;                                             \
        va_start(ap, flags);                                    \
        mode = va_arg(ap, mode_t);                              \
        va_end(ap);                                             \
    }                                                           \
    return call;

int open(const char *path, int flags, ...) { OPEN_BODY(REAL(open)(path, flags, mode)) }
int open64(const char *path, int flags, ...) { OPEN_BODY(REAL(open64)(path, flags, mode)) }
int openat(int at, const char *path, int flags, ...) { OPEN_BODY(REAL(openat)(at, path, flags, mode)) }
int openat64(int at, const char *path, int flags, ...) { OPEN_BODY(REAL(openat64)(at, path, flags, mode)) }

int close(int fd) {
    if (fd >= 0 && fd < MAX_FDS) kinds[fd] = NONE;
    return REAL(close)(fd);
}

// An empty write is a no-op on a device node, but on a SEQPACKET socket it
// sends an empty packet, which the broker can't tell from a hang-up.
ssize_t write(int fd, const void *buf, size_t len) {
    if (len == 0 && fd >= 0 && fd < MAX_FDS && kinds[fd] != NONE) return 0;
    return REAL(write)(fd, buf, len);
}

DIR *opendir(const char *path) {
    if (dir() && path && (strcmp(path, "/dev/input") == 0 || strcmp(path, "/dev/input/") == 0)) {
        char redirected[4096];
        snprintf(redirected, sizeof redirected, "%s/devinput", dir());
        return REAL(opendir)(redirected);
    }
    return REAL(opendir)(path);
}

// Reads the node's "key value..." config line for `key` into `out`.
static int conf_line(int node, const char *suffix, const char *key, char *out, size_t len) {
    char path[4096], line[8192];
    snprintf(path, sizeof path, "%s/event%d.%s", dir(), node, suffix);
    FILE *f = fopen(path, "re");
    if (!f) return -1;
    int found = -1;
    size_t klen = strlen(key);
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\n")] = 0;
        if (!*key || (strncmp(line, key, klen) == 0 && (line[klen] == ' ' || !line[klen]))) {
            snprintf(out, len, "%s", *key ? line + klen + (line[klen] == ' ') : line);
            found = 0;
            break;
        }
    }
    fclose(f);
    return found;
}

// Sets bit N in `buf` for each number in the space separated `codes`.
static void set_bits(unsigned char *buf, size_t len, const char *codes) {
    char *copy = strdup(codes), *save = NULL;
    for (char *tok = strtok_r(copy, " ", &save); tok; tok = strtok_r(NULL, " ", &save)) {
        long bit = strtol(tok, NULL, 0);
        if (bit >= 0 && (size_t)(bit / 8) < len) buf[bit / 8] |= 1u << (bit % 8);
    }
    free(copy);
}

static int evdev_ioctl(int fd, unsigned long req, void *arg) {
    int node = nodes[fd];
    unsigned nr = req & 0xff, size = (req >> 16) & 0x3fff;
    char value[8192];
    switch (nr) {
    case 0x01: *(int *)arg = 0x010001; return 0;                 // EVIOCGVERSION
    case 0x02: {                                                   // EVIOCGID
        uint16_t id[4] = {0x03, 0x1234, (uint16_t)(0x100 + node), 1};
        memcpy(arg, id, sizeof id);
        return 0;
    }
    case 0x06:                                                     // EVIOCGNAME
        if (conf_line(node, "conf", "name", value, sizeof value) < 0) strcpy(value, "fake input");
        snprintf(arg, size, "%s", value);
        return (int)(strlen(arg) + 1);
    case 0x90: {                                                   // EVIOCGRAB
        // Tell the broker so it stops (or resumes) delivering the device's
        // events to the desktop itself. Type 0xffff is never a real event.
        struct { int64_t sec, usec; uint16_t type, code; int32_t value; } ev = {
            0, 0, 0xffff, 0x90, (int32_t)(intptr_t)arg};
        return send(fd, &ev, sizeof ev, MSG_NOSIGNAL) == sizeof ev ? 0 : -1;
    }
    }
    if ((req >> 30) & 2) {                                         // any other read: zeroed buffer
        memset(arg, 0, size);
        if (nr >= 0x20 && nr < 0x40) {                             // EVIOCGBIT(ev)
            static const char *keys[] = {"ev", "key", "rel"};
            unsigned ev = nr - 0x20;
            if (ev < 3 && conf_line(node, "conf", keys[ev], value, sizeof value) == 0)
                set_bits(arg, size, value);
            return (int)size;
        }
        if (nr == 0x18 && conf_line(node, "held", "", value, sizeof value) == 0)  // EVIOCGKEY
            set_bits(arg, size, value);
        return nr >= 0x40 && nr < 0x80 ? 0 : (int)size;
    }
    return 0;
}

static int uinput_ioctl(int fd, unsigned long req, void *arg) {
    unsigned nr = req & 0xff, size = (req >> 16) & 0x3fff;
    switch (nr) {
    case 3: {                                                      // UI_DEV_SETUP: pass the name on
        char msg[96] = "SETUP ";
        snprintf(msg + 6, sizeof msg - 6, "%s", (const char *)arg + 8);
        return send(fd, msg, strlen(msg), MSG_NOSIGNAL) < 0 ? -1 : 0;
    }
    case 44:                                                       // UI_GET_SYSNAME
        snprintf(arg, size, "input99");
        return 8;
    }
    return 0;
}

int ioctl(int fd, unsigned long req, ...) {
    va_list ap;
    va_start(ap, req);
    void *arg = va_arg(ap, void *);
    va_end(ap);
    if (fd >= 0 && fd < MAX_FDS && kinds[fd] != NONE) {
        unsigned type = (req >> 8) & 0xff;
        if (kinds[fd] == EVDEV && type == 'E') return evdev_ioctl(fd, req, arg);
        if (kinds[fd] == UINPUT && type == 'U') return uinput_ioctl(fd, req, arg);
        errno = ENOTTY;
        return -1;
    }
    return REAL(ioctl)(fd, req, arg);
}
