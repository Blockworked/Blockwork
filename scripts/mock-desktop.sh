#!/usr/bin/env bash
# Headless desktops for testing Blockwork without a real session: a virtual
# X11 or Wayland desktop with a stand-in Minecraft window, for the live
# screen/window tests and for launching the app. Made for Ubuntu 24.04 cloud
# containers and CI (no GPU, no /dev/uinput); see docs/mock-desktop.md.
#
#   scripts/mock-desktop.sh setup              install everything (root)
#   scripts/mock-desktop.sh run <desktop> [cmd...]
#
# <desktop> is x11 (Xvfb + openbox), sway (headless wlroots + grim),
# kwin5 (Plasma 5.27 KWin, has ScreenShot2) or kwin6 (Plasma 6 KWin in an
# Ubuntu 25.10 root, has kdotool and KWin scripts but no ScreenShot2).
# `cmd` runs inside the desktop; it defaults to the live screen tests. A
# screenshot of the desktop is saved to $MOCK_SHOT (default
# target/mock-desktop-<desktop>.png) when the desktop can take one.
#
# Fake input devices are on unless MOCK_INPUT=0: a keyboard, a mouse and
# /dev/uinput served by scripts/mock-desktop/fake-input.py through an
# LD_PRELOAD shim, with what is written to uinput injected into X11. Drive them
# from `cmd` with `mock-input` (see `mock-input --help`).
set -euo pipefail

QT_PREFIX=${QT_PREFIX:-/opt/qt6}
PLASMA6_ROOT=${PLASMA6_ROOT:-/opt/plasma6}
REPO=$(cd "$(dirname "$0")/.." && pwd)
GAME_QML="$REPO/scripts/mock-desktop/fake-minecraft.qml"
INPUT_DIR="$REPO/scripts/mock-desktop"
WIDTH=1920
HEIGHT=1080

die() { echo "mock-desktop: $*" >&2; exit 1; }

setup() {
    [[ $EUID -eq 0 ]] || die "setup needs root"
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    # Build deps, the desktops, and what the live tests shell out to.
    apt-get install -y -qq \
        pkg-config clang libgtk-3-dev libxdo-dev libssl-dev libdbus-1-dev libudev-dev \
        libwayland-dev libxkbcommon-dev libgl1-mesa-dev libegl1 libfontconfig1-dev \
        libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 libxcb-render-util0 libxcb-xkb1 \
        xvfb openbox xdotool imagemagick dbus-x11 sway grim kwin-wayland tesseract-ocr debootstrap \
        gcc python3-pip
    # The fake input broker injects into X11 with XTest.
    python3 -c 'import Xlib' 2>/dev/null || pip install -q --break-system-packages python-xlib \
        || apt-get install -y -qq python3-xlib
    # Blockwork needs Qt 6.10+. download.qt.io (aqt, CXX-Qt's qt_minimal) is
    # often blocked in sandboxes; conda-forge is not.
    if [[ ! -x $QT_PREFIX/bin/qmake6 ]]; then
        if ! command -v micromamba >/dev/null; then
            curl -fsSL https://github.com/mamba-org/micromamba-releases/releases/latest/download/micromamba-linux-64 \
                -o /usr/local/bin/micromamba
            chmod +x /usr/local/bin/micromamba
        fi
        MAMBA_ROOT_PREFIX=${MAMBA_ROOT_PREFIX:-/opt/mamba} micromamba create -y -q -p "$QT_PREFIX" -c conda-forge "qt6-main>=6.10,<6.11"
    fi
    command -v kdotool >/dev/null || cargo install --locked kdotool
    # File capabilities can't be raised in a container, so exec of the
    # packaged kwin_wayland fails; a plain copy runs. It has to stay named
    # kwin_wayland or KWin refuses its own Qt platform plugin.
    install -Dm0755 /usr/bin/kwin_wayland /opt/kwin5/kwin_wayland
    setup_plasma6
    echo "mock-desktop: ready. Build with QMAKE=$QT_PREFIX/bin/qmake6"
}

# Plasma 6 KWin from Ubuntu 25.10, in a chroot. Ubuntu 24.04 only has 5.27,
# which kdotool and Blockwork's KWin scripts don't support.
setup_plasma6() {
    [[ -x $PLASMA6_ROOT/opt/kwin/kwin_wayland ]] && return
    local scripts=/usr/share/debootstrap/scripts
    [[ -e $scripts/questing ]] || ln -s gutsy "$scripts/questing"
    # debootstrap doesn't pick a logind for polkit-qt; apt fixes that after.
    debootstrap --variant=minbase --components=main,universe \
        --include=kwin-wayland,breeze,breeze-cursor-theme,dbus,ca-certificates \
        questing "$PLASMA6_ROOT" http://archive.ubuntu.com/ubuntu/ || true
    echo "deb http://archive.ubuntu.com/ubuntu questing main universe" > "$PLASMA6_ROOT/etc/apt/sources.list"
    cp /etc/resolv.conf "$PLASMA6_ROOT/etc/"
    with_plasma6_mounts chroot "$PLASMA6_ROOT" sh -c \
        'export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get --fix-broken install -y -qq'
    install -Dm0755 "$PLASMA6_ROOT/usr/bin/kwin_wayland" "$PLASMA6_ROOT/opt/kwin/kwin_wayland"
}

PLASMA6_MOUNTS=()
mount_plasma6() {
    # /tmp holds the runtime dir and kdotool's scripts; $HOME holds
    # Blockwork's (~/.cache/blockwork). KWin has to read both.
    local dir
    for dir in /proc /dev /tmp "$HOME"; do
        mkdir -p "$PLASMA6_ROOT$dir"
        mountpoint -q "$PLASMA6_ROOT$dir" && continue
        if [[ $dir == /proc ]]; then mount -t proc proc "$PLASMA6_ROOT/proc"; else mount --bind "$dir" "$PLASMA6_ROOT$dir"; fi
        PLASMA6_MOUNTS+=("$PLASMA6_ROOT$dir")
    done
}
umount_plasma6() {
    local i
    for ((i = ${#PLASMA6_MOUNTS[@]} - 1; i >= 0; i--)); do umount -l "${PLASMA6_MOUNTS[i]}" || true; done
    PLASMA6_MOUNTS=()
}
with_plasma6_mounts() {
    mount_plasma6
    local status=0
    "$@" || status=$?
    umount_plasma6
    return $status
}

PIDS=()
STATE=
cleanup() {
    local pid
    for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    sleep 0.5
    umount_plasma6
    [[ -n $STATE ]] && rm -r -- "$STATE"
}

spawn() {
    setsid "$@" >>"$STATE/log" 2>&1 &
    PIDS+=($!)
}

wait_for() {
    local i
    for ((i = 0; i < 40; i++)); do
        eval "$1" >/dev/null 2>&1 && return
        sleep 0.25
    done
    die "timed out waiting for: $1 (log: $(tail -5 "$STATE/log"))"
}

session_bus() {
    eval "$(dbus-launch --sh-syntax)"
    PIDS+=("$DBUS_SESSION_BUS_PID")
}

start_x11() {
    local n=99
    while [[ -e /tmp/.X11-unix/X$n ]]; do n=$((n + 1)); done
    export DISPLAY=:$n
    unset WAYLAND_DISPLAY
    spawn Xvfb "$DISPLAY" -screen 0 ${WIDTH}x${HEIGHT}x24 -nolisten tcp
    wait_for "xdpyinfo -display $DISPLAY || xdotool getdisplaygeometry"
    session_bus
    spawn openbox
}

start_sway() {
    export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1
    printf 'output HEADLESS-1 resolution %sx%s\nxwayland disable\n' $WIDTH $HEIGHT >"$STATE/sway.conf"
    session_bus
    spawn sway -c "$STATE/sway.conf"
    wait_for 'ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock'
    SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock)
    local socket
    for socket in "$XDG_RUNTIME_DIR"/wayland-[0-9]*; do [[ -S $socket ]] && WAYLAND_DISPLAY=${socket##*/}; done
    export SWAYSOCK WAYLAND_DISPLAY
}

start_kwin5() {
    [[ -x /opt/kwin5/kwin_wayland ]] || die "run setup first"
    session_bus
    export WAYLAND_DISPLAY=wayland-mock XDG_CURRENT_DESKTOP=KDE KDE_SESSION_VERSION=5 XDG_SESSION_TYPE=wayland
    # Without the variable, ScreenShot2 only answers callers with a desktop
    # file that allows it.
    KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1 spawn /opt/kwin5/kwin_wayland --virtual --no-lockscreen \
        --width $WIDTH --height $HEIGHT --socket "$WAYLAND_DISPLAY"
    wait_for '[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]'
}

start_kwin6() {
    [[ -x $PLASMA6_ROOT/opt/kwin/kwin_wayland ]] || die "run setup first"
    mount_plasma6
    export WAYLAND_DISPLAY=wayland-mock XDG_CURRENT_DESKTOP=KDE KDE_SESSION_VERSION=6 XDG_SESSION_TYPE=wayland
    export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
    spawn chroot "$PLASMA6_ROOT" dbus-daemon --session --nofork --address="$DBUS_SESSION_BUS_ADDRESS"
    wait_for '[ -S "$XDG_RUNTIME_DIR/bus" ]'
    KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1 spawn chroot "$PLASMA6_ROOT" /opt/kwin/kwin_wayland --virtual --no-lockscreen \
        --width $WIDTH --height $HEIGHT --socket "$WAYLAND_DISPLAY"
    wait_for '[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]'
}

start_game() {
    local qml=$QT_PREFIX/bin/qml6
    [[ -x $qml ]] || qml=$(command -v qml6 || command -v qml) || die "no qml runtime; run setup"
    QML_BIN=$qml
    local platform=wayland
    [[ $DESKTOP == x11 ]] && platform=xcb
    # Server-side decorations where the desktop has them, so the window's
    # content is just the game.
    QT_QPA_PLATFORM=$platform QT_WAYLAND_DISABLE_WINDOWDECORATION=1 spawn "$qml" "$GAME_QML"
    case $DESKTOP in
        x11) wait_for 'xdotool search --name Minecraft' ;;
        sway) wait_for 'swaymsg -t get_tree | grep -q Minecraft' ;;
        kwin6) wait_for 'kdotool search Minecraft | grep -q .' ;;
        *) sleep 2 ;;
    esac
}

# Fake keyboard, mouse and /dev/uinput (see the header). The shim only
# touches those paths, so it's preloaded into everything `cmd` starts.
start_input() {
    [[ ${MOCK_INPUT:-1} == 1 ]] || return 0
    export FAKE_EVDEV_DIR=$STATE/input MOCK_INPUT_LOG=$STATE/input-target.log
    mkdir -p "$FAKE_EVDEV_DIR" "$STATE/bin"
    cc -shared -fPIC -O2 -o "$STATE/fake-evdev.so" "$INPUT_DIR/fake-evdev.c" -ldl
    spawn python3 "$INPUT_DIR/fake-input.py" serve "$FAKE_EVDEV_DIR"
    wait_for '[ -S "$FAKE_EVDEV_DIR/control.sock" ]'
    local platform=wayland
    [[ $DESKTOP == x11 ]] && platform=xcb
    # `mock-input target` opens a window that logs the input it gets to
    # $MOCK_INPUT_LOG; everything else goes to the broker.
    {
        echo '#!/bin/sh'
        echo 'if [ "$1" = target ]; then'
        echo "    QT_QPA_PLATFORM=$platform LD_PRELOAD= setsid '$QML_BIN' '$INPUT_DIR/input-target.qml' >>'$MOCK_INPUT_LOG' 2>&1 &"
        echo '    exit 0'
        echo 'fi'
        echo "exec python3 '$INPUT_DIR/fake-input.py' \"\$@\""
    } >"$STATE/bin/mock-input"
    chmod +x "$STATE/bin/mock-input"
    export PATH=$STATE/bin:$PATH
    export LD_PRELOAD=$STATE/fake-evdev.so${LD_PRELOAD:+:$LD_PRELOAD}
}

screenshot() {
    local out=${MOCK_SHOT:-$REPO/target/mock-desktop-$DESKTOP.png}
    mkdir -p "$(dirname "$out")"
    case $DESKTOP in
        x11) import -window root "$out" ;;
        sway) grim "$out" ;;
        # KWin's screenshots go through ScreenShot2, which the live tests use;
        # there's no CLI for it here, and Plasma 6 has none without a GPU.
        *) return 0 ;;
    esac
    echo "mock-desktop: screenshot saved to $out"
}

run() {
    DESKTOP=${1:-}
    shift || true
    [[ $DESKTOP =~ ^(x11|sway|kwin5|kwin6)$ ]] || die "usage: $0 run <x11|sway|kwin5|kwin6> [cmd...]"
    STATE=$(mktemp -d /tmp/mock-desktop.XXXXXX)
    trap cleanup EXIT
    mkdir -m 700 "$STATE/xdg"
    export XDG_RUNTIME_DIR=$STATE/xdg LC_ALL=C.UTF-8 BW_LIVE_WINDOW=Minecraft
    unset DISPLAY WAYLAND_DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE
    [[ -x $QT_PREFIX/bin/qmake6 ]] && export QMAKE=$QT_PREFIX/bin/qmake6
    "start_$DESKTOP"
    start_game
    start_input
    # The conda-forge Qt isn't on the loader path for the built app.
    [[ -d $QT_PREFIX/lib ]] && export LD_LIBRARY_PATH=$QT_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
    local status=0
    if [[ $# -eq 0 ]]; then
        (cd "$REPO" && cargo test -p blockwork-core -- --ignored --nocapture screen::) || status=$?
    else
        "$@" || status=$?
    fi
    screenshot || true
    return $status
}

case ${1:-} in
    setup) setup ;;
    run) shift; run "$@" ;;
    *) die "usage: $0 setup | run <x11|sway|kwin5|kwin6> [cmd...]" ;;
esac
