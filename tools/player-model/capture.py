#!/usr/bin/env python3
"""Captures an auto-labeled Minecraft player dataset. See README.md.

Drives a local Fabric + Carpet server over RCON: spawns fake players, puts
the camera (your spectator player) somewhere looking at them, freezes the
world, and takes two F2 screenshots, with and without the players. label.py
turns each pair into YOLO boxes.
"""

import argparse
import math
import random
import re
import socket
import struct
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image

import label

ARMOR = ["leather", "chainmail", "iron", "golden", "diamond", "netherite"]
SLOTS = [("head", "helmet"), ("chest", "chestplate"), ("legs", "leggings"), ("feet", "boots")]
HELD = ["air", "air", "diamond_sword", "iron_sword", "bow", "crossbow", "shield", "iron_pickaxe",
        "cobblestone", "oak_planks", "torch", "golden_apple", "ender_pearl", "trident", "fishing_rod"]
TIMES = [1000, 6000, 6000, 10000, 12500, 13500, 18000, 23000]


class Rcon:
    """Minimal Source RCON client, as Minecraft speaks it."""

    def __init__(self, host, port, password):
        self.sock = socket.create_connection((host, port), timeout=10)
        self.next_id = 0
        if self._send(3, password)[0] == -1:
            sys.exit("RCON refused the password")

    def _send(self, kind, body):
        self.next_id += 1
        data = struct.pack("<ii", self.next_id, kind) + body.encode() + b"\0\0"
        self.sock.sendall(struct.pack("<i", len(data)) + data)
        size = struct.unpack("<i", self._read(4))[0]
        rid, _ = struct.unpack("<ii", self._read(8))
        return rid, self._read(size - 8)[:-2].decode(errors="replace")

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                sys.exit("RCON connection closed")
            buf += chunk
        return buf

    def __call__(self, command):
        return self._send(2, command)[1]


class Keys:
    """Presses F1/F2 through a virtual keyboard; the game must be focused."""

    def __init__(self):
        from evdev import UInput, ecodes

        self.e = ecodes
        self.ui = UInput({ecodes.EV_KEY: [ecodes.KEY_F1, ecodes.KEY_F2]}, name="blockwork-dataset-keys")
        time.sleep(1)  # let the compositor pick the device up

    def tap(self, key):
        code = getattr(self.e, key)
        self.ui.write(self.e.EV_KEY, code, 1)
        self.ui.syn()
        time.sleep(0.05)
        self.ui.write(self.e.EV_KEY, code, 0)
        self.ui.syn()


def numbers(text):
    return [float(v) for v in re.findall(r"-?\d+(?:\.\d+)?(?:E-?\d+)?", text.split("data:")[-1])]


def screenshot(keys, folder, timeout=10):
    """Presses F2 and returns the new screenshot as an RGBA array."""
    before = set(folder.glob("*.png"))
    keys.tap("KEY_F2")
    deadline = time.time() + timeout
    while time.time() < deadline:
        new = set(folder.glob("*.png")) - before
        if new:
            path = new.pop()
            size = -1
            while size != path.stat().st_size:  # wait until fully written
                size = path.stat().st_size
                time.sleep(0.15)
            img = np.asarray(Image.open(path).convert("RGBA"))
            path.unlink()
            return img
        time.sleep(0.05)
    sys.exit("no screenshot appeared; is the game focused, and is --mc-dir right?")


def clear_eye(rcon, eye, tries=6):
    """The eye raised until it's in open air (a spectator inside terrain
    sees through it), or None if that doesn't happen within a few blocks."""
    x, y, z = eye
    for _ in range(tries):
        if "passed" in rcon(f"execute if block {math.floor(x)} {math.floor(y)} {math.floor(z)} minecraft:air"):
            return x, y, z
        y += 1
    return None


def read_positions(rcon, players):
    return [tuple(numbers(rcon(f"data get entity {p['name']} Pos"))[:3]) for p in players]


def read_fov(mc_dir):
    """Vertical FOV from options.txt (stored as (fov - 70) / 40)."""
    for line in (mc_dir / "options.txt").read_text().splitlines():
        if line.startswith("fov:"):
            return 70 + float(line.split(":", 1)[1]) * 40
    return 70.0


def look_at(eye, target):
    dx, dy, dz = (t - e for t, e in zip(target, eye))
    yaw = math.degrees(math.atan2(-dx, dz))
    pitch = -math.degrees(math.atan2(dy, math.hypot(dx, dz)))
    return yaw, pitch


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rcon", default="127.0.0.1:25575")
    ap.add_argument("--password", required=True)
    ap.add_argument("--mc-dir", type=Path, required=True, help="the game folder (has options.txt and screenshots/)")
    ap.add_argument("--camera", required=True, help="your player name; it becomes a spectator camera")
    ap.add_argument("--out", type=Path, default=Path("dataset"))
    ap.add_argument("--samples", type=int, default=1500)
    ap.add_argument("--bots", type=int, default=6, help="most fake players in one frame")
    ap.add_argument("--names", type=Path, help="file of fake player names, one per line (skins come from these)")
    ap.add_argument("--range", type=int, default=20000, help="how far from 0,0 to roam, in blocks")
    ap.add_argument("--width", type=int, default=640, help="saved image width")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--hud-hidden", action="store_true", help="the HUD is already hidden; don't press F1")
    ap.add_argument("--debug", action="store_true", help="also save the empty frame and projected boxes")
    args = ap.parse_args()
    rng = random.Random(args.seed)

    host, port = args.rcon.rsplit(":", 1)
    rcon = Rcon(host, int(port), args.password)
    fov = read_fov(args.mc_dir)
    shots = args.mc_dir / "screenshots"
    shots.mkdir(exist_ok=True)
    (args.out / "images").mkdir(parents=True, exist_ok=True)
    (args.out / "labels").mkdir(parents=True, exist_ok=True)
    start = len(list((args.out / "images").glob("*.png")))

    names = [n.strip() for n in args.names.read_text().splitlines() if n.strip()] if args.names else []
    names = names or [f"bwbot{i}" for i in range(args.bots)]

    cam = args.camera
    # Game rule names changed in newer versions; try the new one, then the old.
    rules = [("advance_time", "doDaylightCycle", "false"), ("advance_weather", "doWeatherCycle", "false"),
             ("spawn_mobs", "doMobSpawning", "false"), ("immediate_respawn", "doImmediateRespawn", "true"),
             ("show_death_messages", "showDeathMessages", "false"), ("send_command_feedback", "sendCommandFeedback", "false"),
             ("log_admin_commands", "logAdminCommands", "false")]
    for new, old, value in rules:
        if "Incorrect" in rcon(f"gamerule {new} {value}") and "Incorrect" in rcon(f"gamerule {old} {value}"):
            sys.exit(f"the server rejected the game rule {new}")
    rcon("team add bwbots")
    for cmd in ["tick unfreeze", "team modify bwbots nametagVisibility never", "team modify bwbots collisionRule never",
                f"gamemode spectator {cam}"]:
        reply = rcon(cmd)
        if "Incorrect" in reply or "Unknown" in reply:
            sys.exit(f"the server rejected '{cmd}': {reply}")
    print(f"FOV {fov:.0f}. Click into the game window now; starting in 5 s.")
    time.sleep(5)
    keys = Keys()
    if not args.hud_hidden:
        keys.tap("KEY_F1")  # hide the HUD

    spawned = []
    made, boxes = 0, 0
    base = None
    while made < args.samples:
        rcon("tick unfreeze")
        # A new place, time and weather every 15 samples.
        if base is None or made % 15 == 0:
            base = (rng.randint(-args.range, args.range), rng.randint(-args.range, args.range))
            rcon(f"tp {cam} {base[0]} 200 {base[1]}")
            rcon(f"time set {rng.choice(TIMES)}")
            rcon(f"weather {'rain' if rng.random() < 0.2 else 'clear'}")
            time.sleep(4)  # chunks load
        # Fresh names now and then, for skin variety.
        if not spawned or made % 60 == 0:
            for name in spawned:
                rcon(f"player {name} kill")
            spawned = rng.sample(names, min(args.bots, len(names)))
            for name in spawned:
                rcon(f"player {name} spawn at {base[0]} 200 {base[1]}")
                rcon(f"gamemode creative {name}")
                rcon(f"team join bwbots {name}")
            time.sleep(2)

        k = 0 if rng.random() < 0.1 else rng.randint(1, len(spawned))
        active = rng.sample(spawned, k)
        center = (base[0] + rng.randint(-40, 40), base[1] + rng.randint(-40, 40))
        rcon("tag @a remove bwactive")
        for name in spawned:
            if name in active:
                rcon(f"tag {name} add bwactive")
            else:
                rcon(f"tp {name} {base[0] + 3000} 250 {base[1]}")
        players = []
        if active:
            spread = rcon(f"spreadplayers {center[0]} {center[1]} 1 {rng.randint(2, 8)} false @a[tag=bwactive]")
            # Fails over water or with no safe ground: try somewhere else.
            if not spread.startswith("Spread"):
                base = None
                continue
            for name in active:
                for slot, piece in SLOTS:
                    item = f"{rng.choice(ARMOR)}_{piece}" if rng.random() < 0.4 else "air"
                    rcon(f"item replace entity {name} armor.{slot} with {item}")
                rcon(f"item replace entity {name} weapon.mainhand with {rng.choice(HELD)}")
                sneaking = rng.random() < 0.15
                rcon(f"player {name} {'sneak' if sneaking else 'unsneak'}")
                rcon(f"player {name} look {rng.uniform(-180, 180):.1f} {rng.uniform(-30, 30):.1f}")
                players.append({"name": name, "sneaking": sneaking})
            time.sleep(0.6)
            positions = read_positions(rcon, players)
            for p, pos in zip(players, positions):
                p["pos"] = pos

        # Camera: a few to 40 blocks from one player, aimed near (not always at) them.
        if players:
            tx, ty, tz = rng.choice(players)["pos"]
            target = (tx, ty + 1.0, tz)
        else:
            target = (center[0], 70, center[1])
        dist = math.exp(rng.uniform(math.log(3), math.log(40)))
        ang = rng.uniform(0, 2 * math.pi)
        eye = (target[0] + dist * math.sin(ang), target[1] + rng.uniform(-0.5, 4), target[2] + dist * math.cos(ang))
        eye = clear_eye(rcon, eye)
        # A player at the lens fills the view with one blurry body.
        if eye is None or any(math.dist(eye, (x, y + 1, z)) < 2 for x, y, z in (p["pos"] for p in players)):
            continue
        yaw, pitch = look_at(eye, target)
        yaw += rng.uniform(-fov / 3, fov / 3)
        pitch = max(-60, min(60, pitch + rng.uniform(-fov / 5, fov / 5)))
        rcon(f"tp {cam} {eye[0]:.2f} {eye[1] - label.EYE:.2f} {eye[2]:.2f} {yaw:.2f} {pitch:.2f}")

        rcon("tick freeze")
        time.sleep(0.7)  # client catches up with the frozen positions
        with_players = screenshot(keys, shots)
        # Players aren't frozen by /tick: skip the sample if anyone moved.
        if players:
            now = read_positions(rcon, players)
            if max(math.dist(a, b) for a, b in zip(positions, now)) > 0.05:
                print("  skipped: a player was still moving", flush=True)
                continue
            for p, pos in zip(players, now):
                p["pos"] = pos
        for p in players:
            rcon(f"tp {p['name']} {base[0] + 3000} 250 {base[1]}")
        time.sleep(0.5)
        without = screenshot(keys, shots)
        pos = numbers(rcon(f"data get entity {cam} Pos"))[:3]
        rot = numbers(rcon(f"data get entity {cam} Rotation"))[:2]
        camera = (pos[0], pos[1], pos[2], rot[0], rot[1])

        found = label.labels(with_players, without, players, camera, fov) if players else []
        idx = start + made
        img = Image.fromarray(with_players).convert("RGB")
        img = img.resize((args.width, round(img.height * args.width / img.width)), Image.BILINEAR)
        img.save(args.out / "images" / f"{idx:06d}.png")
        (args.out / "labels" / f"{idx:06d}.txt").write_text("".join(f"0 {cx:.6f} {cy:.6f} {w:.6f} {h:.6f}\n" for cx, cy, w, h in found))
        if args.debug:
            import json
            dbg = args.out / "debug"
            dbg.mkdir(exist_ok=True)
            Image.fromarray(without).convert("RGB").save(dbg / f"{idx:06d}-empty.png")
            Image.fromarray(with_players).convert("RGB").save(dbg / f"{idx:06d}-full.png")
            size = (with_players.shape[1], with_players.shape[0])
            meta = {"camera": camera, "fov": fov, "found": found,
                    "players": [{**p, "box": label.player_box(p, camera, size, fov)} for p in players]}
            (dbg / f"{idx:06d}.json").write_text(json.dumps(meta, indent=1))
        made += 1
        boxes += len(found)
        print(f"{made}/{args.samples}: {len(found)} of {len(players)} players visible ({boxes} boxes so far)", flush=True)

    rcon("tick unfreeze")
    for name in spawned:
        rcon(f"player {name} kill")
    if not args.hud_hidden:
        keys.tap("KEY_F1")  # show the HUD again, since this run hid it
    print(f"done: {made} images, {boxes} boxes in {args.out}")


if __name__ == "__main__":
    main()
