#!/usr/bin/env python3
"""Fake input devices for the mock desktop.

Serves the sockets behind fake-evdev.so (see that file): a fake keyboard
(/dev/input/event0), a fake mouse (/dev/input/event1) and /dev/uinput. What a
process writes to uinput is injected into the X server with XTest, the way the
kernel would hand a real virtual device to the desktop, and logged. Events
"typed" on the fake keyboard and mouse go to every process that opened the
device and, while nobody holds an EVIOCGRAB on it, to the X server too.

    fake-input.py serve <dir>        run the broker (mock-desktop.sh does this)
    fake-input.py <command...>       drive it; $FAKE_EVDEV_DIR names the broker

Commands (keys and buttons take names from linux/input-event-codes.h, with or
without the KEY_/BTN_ prefix, or numbers):
    tap KEY...            press and release each key in turn
    combo KEY...          press the keys in order, release in reverse
    down KEY / up KEY     hold or release a key or button
    type [--delay=MS] TEXT  type text on the fake keyboard (US layout), MS apart (default 50)
    click [BUTTON]        click a mouse button (default left)
    move DX DY            relative mouse motion
    wheel N               scroll N notches (negative is down)
    status                grab state of the devices and uinput clients
    log [N]               the last N (default 20) lines of injected uinput events
    wait-grab [0|1] [S]   wait up to S seconds (default 10) for the devices to be grabbed (1) or released (0)
    wait-log PATTERN [S]  wait until a uinput log line contains PATTERN

USB devices are a sysfs-like tree in $FAKE_USB_DIR that fake-evdev.so shows
as /sys/bus/usb/devices; they need no broker:
    usb list                              the fake devices, by port
    usb plug VVVV:PPPP [PRODUCT [MAKER]]  plug in a device, prints its port
    usb unplug TEXT                       unplug every device whose port, ids or name contains TEXT
"""
import os
import re
import select
import shlex
import socket
import struct
import sys
import time

EVENT = struct.Struct("qqHHi")
EV_SYN, EV_KEY, EV_REL = 0, 1, 2
REL_X, REL_Y, REL_HWHEEL, REL_WHEEL = 0, 1, 6, 8
GRAB = 0xFFFF
BUTTONS = {0x110: 1, 0x111: 3, 0x112: 2, 0x113: 8, 0x114: 9}

DEVICES = {
    0: {
        "name": "Fake Keyboard",
        "ev": [EV_SYN, EV_KEY],
        "key": list(range(1, 249)),
        "rel": [],
    },
    1: {
        "name": "Fake Mouse",
        "ev": [EV_SYN, EV_KEY, EV_REL],
        "key": list(BUTTONS),
        "rel": [REL_X, REL_Y, REL_HWHEEL, REL_WHEEL],
    },
}


def load_codes():
    codes = {}
    for header in ("/usr/include/linux/input-event-codes.h",):
        try:
            text = open(header).read()
        except OSError:
            continue
        for name, value in re.findall(r"#define\s+((?:KEY|BTN)_\w+)\s+(0x[0-9a-fA-F]+|\d+)\b", text):
            codes[name] = int(value, 0)
    return codes


CODES = load_codes()


def code_of(name):
    if re.fullmatch(r"\d+|0x[0-9a-fA-F]+", name):
        return int(name, 0)
    upper = name.upper()
    for candidate in (upper, "KEY_" + upper, "BTN_" + upper):
        if candidate in CODES:
            return CODES[candidate]
    raise SystemExit(f"fake-input: unknown key {name!r}")


# US layout: character -> (key, shift)
def char_keys():
    table = {}
    plain = "1234567890-=qwertyuiop[]asdfghjkl;'`\\zxcvbnm,./"
    shifted = '!@#$%^&*()_+QWERTYUIOP{}ASDFGHJKL:"~|ZXCVBNM<>?'
    codes = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
             30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53]
    for ch, sh, code in zip(plain, shifted, codes):
        table[ch] = (code, False)
        table[sh] = (code, True)
    table[" "] = (57, False)
    table["\n"] = (28, False)
    table["\t"] = (15, False)
    return table


class X:
    """Injects events into the X server, or just counts them without one."""

    def __init__(self):
        self.display = None
        if os.environ.get("DISPLAY"):
            try:
                from Xlib import X as XC, display
                from Xlib.ext import xtest
                self.display = display.Display()
                self.XC, self.xtest = XC, xtest
            except Exception as err:  # noqa: BLE001
                print(f"fake-input: no XTest ({err}); events are logged only", file=sys.stderr)

    def key(self, code, pressed):
        if not self.display:
            return
        if code in BUTTONS:
            kind = self.XC.ButtonPress if pressed else self.XC.ButtonRelease
            self.xtest.fake_input(self.display, kind, BUTTONS[code])
        elif code < 0x100:
            kind = self.XC.KeyPress if pressed else self.XC.KeyRelease
            self.xtest.fake_input(self.display, kind, code + 8)

    def motion(self, dx, dy):
        if self.display and (dx or dy):
            self.xtest.fake_input(self.display, self.XC.MotionNotify, True, x=dx, y=dy)

    def wheel(self, axis, value):
        if not self.display:
            return
        up, down = (4, 5) if axis == REL_WHEEL else (7, 6)
        for _ in range(abs(value)):
            button = up if value > 0 else down
            self.xtest.fake_input(self.display, self.XC.ButtonPress, button)
            self.xtest.fake_input(self.display, self.XC.ButtonRelease, button)

    def flush(self):
        if self.display:
            self.display.sync()

    def apply(self, events):
        dx = dy = 0
        for etype, code, value in events:
            if etype == EV_KEY and value in (0, 1):
                self.key(code, value == 1)
            elif etype == EV_REL and code == REL_X:
                dx += value
            elif etype == EV_REL and code == REL_Y:
                dy += value
            elif etype == EV_REL and code in (REL_WHEEL, REL_HWHEEL):
                self.wheel(code, value)
        self.motion(dx, dy)
        self.flush()


class Broker:
    def __init__(self, root):
        self.root = root
        self.x = X()
        self.listeners = {}   # listening socket -> node (-1 uinput, -2 control)
        self.clients = {}     # client socket -> state dict
        self.held = {n: set() for n in DEVICES}
        self.log = open(os.path.join(root, "uinput.log"), "a", buffering=1)
        os.makedirs(os.path.join(root, "devinput"), exist_ok=True)
        for node, dev in DEVICES.items():
            open(os.path.join(root, "devinput", f"event{node}"), "w").close()
            with open(os.path.join(root, f"event{node}.conf"), "w") as conf:
                conf.write(f"name {dev['name']}\n")
                for key in ("ev", "key", "rel"):
                    conf.write(f"{key} {' '.join(map(str, dev[key]))}\n")
            self.write_held(node)
            self.listen(f"event{node}.sock", socket.SOCK_SEQPACKET, node)
        self.listen("uinput.sock", socket.SOCK_SEQPACKET, -1)
        self.listen("control.sock", socket.SOCK_STREAM, -2)

    def listen(self, name, kind, node):
        path = os.path.join(self.root, name)
        if os.path.exists(path):
            os.unlink(path)
        sock = socket.socket(socket.AF_UNIX, kind)
        sock.bind(path)
        sock.listen(16)
        self.listeners[sock] = node

    def write_held(self, node):
        path = os.path.join(self.root, f"event{node}.held")
        with open(path + ".tmp", "w") as f:
            f.write(" ".join(map(str, sorted(self.held[node]))) + "\n")
        os.replace(path + ".tmp", path)

    def grabbed(self, node):
        return any(c["node"] == node and c["grab"] for c in self.clients.values())

    def run(self):
        while True:
            ready, _, _ = select.select(list(self.listeners) + list(self.clients), [], [])
            for sock in ready:
                if sock in self.listeners:
                    conn, _ = sock.accept()
                    self.clients[conn] = {"node": self.listeners[sock], "grab": False, "name": "", "pending": [], "buf": b""}
                else:
                    self.read(sock)

    def drop(self, sock):
        self.clients.pop(sock, None)
        sock.close()

    def read(self, sock):
        state = self.clients[sock]
        try:
            data = sock.recv(65536)
        except OSError:
            data = b""
        if not data:
            self.drop(sock)
            return
        node = state["node"]
        if node == -2:
            state["buf"] += data
            while b"\n" in state["buf"]:
                line, state["buf"] = state["buf"].split(b"\n", 1)
                reply = self.command(shlex.split(line.decode()))
                try:
                    sock.sendall((reply + "\n").encode())
                except OSError:
                    pass
        elif node == -1:
            self.uinput(state, data)
        else:
            for _, _, etype, code, value in EVENT.iter_unpack(data[: len(data) // EVENT.size * EVENT.size]):
                if etype == GRAB and code == 0x90:
                    state["grab"] = bool(value)
                    print(f"fake-input: event{node} {'grabbed' if value else 'released'}", file=sys.stderr)

    def uinput(self, state, data):
        if data.startswith(b"SETUP "):
            state["name"] = data[6:].decode(errors="replace")
            self.log.write(f"{time.time():.3f} setup {state['name']}\n")
            return
        for _, _, etype, code, value in EVENT.iter_unpack(data[: len(data) // EVENT.size * EVENT.size]):
            if etype == EV_SYN:
                events, state["pending"] = state["pending"], []
                self.x.apply(events)
                for e in events:
                    self.log.write(f"{time.time():.3f} {state['name']} {describe(*e)}\n")
            else:
                state["pending"].append((etype, code, value))

    def emit(self, node, events):
        """Events from the fake device itself, as if a person used it."""
        for etype, code, value in events:
            if etype == EV_KEY:
                (self.held[node].add if value else self.held[node].discard)(code)
        self.write_held(node)
        now = time.time()
        sec, usec = int(now), int((now % 1) * 1e6)
        packet = b"".join(EVENT.pack(sec, usec, *e) for e in events + [(EV_SYN, 0, 0)])
        for sock, state in list(self.clients.items()):
            if state["node"] == node:
                try:
                    sock.send(packet)
                except OSError:
                    self.drop(sock)
        if not self.grabbed(node):
            self.x.apply(events)

    def command(self, args):
        if not args:
            return "error: empty command"
        cmd, rest = args[0], args[1:]
        try:
            if cmd in ("down", "up"):
                code = code_of(rest[0])
                node = 1 if code in BUTTONS else 0
                self.emit(node, [(EV_KEY, code, 1 if cmd == "down" else 0)])
            elif cmd == "tap":
                for name in rest:
                    code = code_of(name)
                    node = 1 if code in BUTTONS else 0
                    self.emit(node, [(EV_KEY, code, 1)])
                    time.sleep(0.02)
                    self.emit(node, [(EV_KEY, code, 0)])
                    time.sleep(0.02)
            elif cmd == "combo":
                codes = [code_of(n) for n in rest]
                for code in codes:
                    self.emit(0, [(EV_KEY, code, 1)])
                    time.sleep(0.02)
                for code in reversed(codes):
                    self.emit(0, [(EV_KEY, code, 0)])
                    time.sleep(0.02)
            elif cmd == "type":
                table = char_keys()
                delay = 0.05
                if rest and rest[0].startswith("--delay="):
                    delay = float(rest.pop(0).split("=", 1)[1]) / 1000
                for ch in " ".join(rest).replace("\\n", "\n"):
                    code, shift = table[ch]
                    if shift:
                        self.emit(0, [(EV_KEY, 42, 1)])
                    self.emit(0, [(EV_KEY, code, 1)])
                    self.emit(0, [(EV_KEY, code, 0)])
                    if shift:
                        self.emit(0, [(EV_KEY, 42, 0)])
                    time.sleep(delay)
            elif cmd == "click":
                code = code_of(rest[0]) if rest else 0x110
                self.emit(1, [(EV_KEY, code, 1)])
                time.sleep(0.03)
                self.emit(1, [(EV_KEY, code, 0)])
            elif cmd == "move":
                self.emit(1, [(EV_REL, REL_X, int(rest[0])), (EV_REL, REL_Y, int(rest[1]))])
            elif cmd == "wheel":
                self.emit(1, [(EV_REL, REL_WHEEL, int(rest[0]))])
            elif cmd == "status":
                lines = [f"event{n} ({d['name']}): {'grabbed' if self.grabbed(n) else 'free'}, "
                         f"{sum(1 for c in self.clients.values() if c['node'] == n)} open, held={sorted(self.held[n])}"
                         for n, d in DEVICES.items()]
                names = [c["name"] or "?" for c in self.clients.values() if c["node"] == -1]
                lines.append(f"uinput: {len(names)} open {names}")
                return "\n".join(lines)
            else:
                return f"error: unknown command {cmd}"
        except (IndexError, ValueError, KeyError) as err:
            return f"error: {err!r}"
        except SystemExit as err:
            return f"error: {err}"
        return "ok"


def describe(etype, code, value):
    if etype == EV_KEY:
        names = [n for n, c in CODES.items() if c == code]
        return f"key {names[0] if names else code} {('up', 'down', 'repeat')[value] if value in (0, 1, 2) else value}"
    if etype == EV_REL:
        return f"rel {({0: 'x', 1: 'y', 6: 'hwheel', 8: 'wheel'}).get(code, code)} {value}"
    return f"ev {etype} {code} {value}"


def control(root, args):
    if args[0] == "log":
        n = int(args[1]) if len(args) > 1 else 20
        with open(os.path.join(root, "uinput.log")) as f:
            print("".join(f.readlines()[-n:]), end="")
        return 0
    if args[0] in ("wait-grab", "wait-log"):
        timeout = float(args[2]) if len(args) > 2 else 10
        deadline = time.time() + timeout
        while time.time() < deadline:
            if args[0] == "wait-grab":
                status = send(root, ["status"])
                want = (args[1] if len(args) > 1 else "1") == "1"
                states = re.findall(r"event\d \([^)]*\): (\w+)", status)
                if states and all((s == "grabbed") == want for s in states):
                    return 0
            else:
                with open(os.path.join(root, "uinput.log")) as f:
                    if any(args[1] in line for line in f):
                        return 0
            time.sleep(0.1)
        print(f"fake-input: timed out: {' '.join(args)}", file=sys.stderr)
        return 1
    reply = send(root, args)
    print(reply)
    return 1 if reply.startswith("error") else 0


def send(root, args):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(os.path.join(root, "control.sock"))
    sock.sendall((shlex.join(args) + "\n").encode())
    out = b""
    while not out.endswith(b"\n"):
        chunk = sock.recv(65536)
        if not chunk:
            break
        out += chunk
    return out.decode().rstrip("\n")


def usb(args):
    root = os.environ.get("FAKE_USB_DIR")
    if not root:
        raise SystemExit("fake-input: FAKE_USB_DIR isn't set (run inside scripts/mock-desktop.sh)")
    os.makedirs(root, exist_ok=True)

    def read(port, name):
        try:
            with open(os.path.join(root, port, name)) as f:
                return f.read().strip()
        except OSError:
            return ""

    def devices():
        for port in sorted(os.listdir(root)):
            ids = f"{read(port, 'idVendor')}:{read(port, 'idProduct')}"
            yield port, ids, " ".join(p for p in (read(port, "manufacturer"), read(port, "product")) if p)

    cmd, rest = (args[0], args[1:]) if args else ("list", [])
    if cmd == "list":
        for port, ids, name in devices():
            print(f"{port}\t{ids}\t{name}")
    elif cmd == "plug" and rest and re.fullmatch(r"[0-9a-fA-F]{4}:[0-9a-fA-F]{4}", rest[0]):
        vendor, product = rest[0].lower().split(":")
        n = 1
        while os.path.exists(os.path.join(root, f"1-{n}")):
            n += 1
        port = os.path.join(root, f"1-{n}")
        os.makedirs(port)
        files = {"idVendor": vendor, "idProduct": product, "bDeviceClass": "00",
                 "product": rest[1] if len(rest) > 1 else "", "manufacturer": rest[2] if len(rest) > 2 else ""}
        for name, value in files.items():
            if value:
                with open(os.path.join(port, name), "w") as f:
                    f.write(value + "\n")
        print(f"1-{n}")
    elif cmd == "unplug" and rest:
        text = rest[0].lower()
        gone = [port for port, ids, name in devices() if text in port.lower() or text in ids or text in name.lower()]
        if not gone:
            raise SystemExit(f"fake-input: no USB device matches {rest[0]!r}")
        for port in gone:
            for name in os.listdir(os.path.join(root, port)):
                os.remove(os.path.join(root, port, name))
            os.rmdir(os.path.join(root, port))
            print(port)
    else:
        raise SystemExit("usage: usb list | usb plug VVVV:PPPP [PRODUCT [MAKER]] | usb unplug TEXT")
    return 0


def main():
    args = sys.argv[1:]
    if not args or args[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    if args[0] == "serve":
        root = args[1]
        os.makedirs(root, exist_ok=True)
        broker = Broker(root)
        print(f"fake-input: serving {root}", file=sys.stderr, flush=True)
        broker.run()
        return 0
    if args[0] == "usb":
        return usb(args[1:])
    root = os.environ.get("FAKE_EVDEV_DIR")
    if not root:
        raise SystemExit("fake-input: FAKE_EVDEV_DIR isn't set (run inside scripts/mock-desktop.sh)")
    return control(root, args)


if __name__ == "__main__":
    sys.exit(main())
