"""Turns a capture pair into YOLO labels.

Each sample is two frames of a frozen world: one with the fake players, one
with them moved away. Pixels that differ are visible player pixels. Each
player's known 3D position is projected into the frame to decide which
player a pixel belongs to, so occlusion and overlap come out right.
"""

import math

import numpy as np

# Player hitbox in blocks; sneaking lowers it.
WIDTH, HEIGHT, SNEAK_HEIGHT = 0.6, 1.8, 1.5
# Spectator eye height above the camera player's feet.
EYE = 1.62
# Max channel difference that still counts as "unchanged".
THRESHOLD = 14
# Projected boxes grow by this much before claiming pixels.
GATE = 0.25
# Closer than this (in blocks) a player can't be boxed sensibly.
NEAR = 0.3


def basis(yaw, pitch):
    """Camera forward/right/up vectors for Minecraft yaw/pitch in degrees.
    Yaw 0 faces +Z (south), 90 faces -X; positive pitch looks down."""
    y, p = math.radians(yaw), math.radians(pitch)
    forward = np.array([-math.sin(y) * math.cos(p), -math.sin(p), math.cos(y) * math.cos(p)])
    right = np.array([-math.cos(y), 0.0, -math.sin(y)])
    up = np.cross(right, forward)
    return forward, right, up


def project(points, camera, size, fov):
    """Screen pixels of world `points`, or None for a point behind the camera.
    `camera` is (x, y, z, yaw, pitch) with y at the feet; `fov` is vertical."""
    w, h = size
    eye = np.array([camera[0], camera[1] + EYE, camera[2]])
    forward, right, up = basis(camera[3], camera[4])
    f = (h / 2) / math.tan(math.radians(fov) / 2)
    out = []
    for p in points:
        v = np.asarray(p, dtype=float) - eye
        depth = v @ forward
        if depth <= NEAR:
            out.append(None)
            continue
        out.append((w / 2 + f * (v @ right) / depth, h / 2 - f * (v @ up) / depth, depth))
    return out


def player_box(player, camera, size, fov):
    """Projected (x0, y0, x1, y1, depth) of a player's hitbox, or None when
    any corner is behind the camera."""
    x, y, z = player["pos"]
    top = y + (SNEAK_HEIGHT if player.get("sneaking") else HEIGHT)
    half = WIDTH / 2
    corners = [(x + dx, yy, z + dz) for dx in (-half, half) for dz in (-half, half) for yy in (y, top)]
    pts = project(corners, camera, size, fov)
    if any(p is None for p in pts):
        return None
    xs, ys, ds = zip(*pts)
    return min(xs), min(ys), max(xs), max(ys), min(ds)


def changed_mask(with_players, without_players):
    """Pixels that differ between the frames, minus isolated specks."""
    a = with_players[..., :3].astype(np.int16)
    b = without_players[..., :3].astype(np.int16)
    mask = np.abs(a - b).max(axis=2) > THRESHOLD
    # Keep pixels with at least 3 changed neighbours.
    padded = np.pad(mask, 1).astype(np.uint8)
    neighbours = sum(
        padded[1 + dy: padded.shape[0] - 1 + dy, 1 + dx: padded.shape[1] - 1 + dx]
        for dy in (-1, 0, 1) for dx in (-1, 0, 1) if (dy, dx) != (0, 0)
    )
    return mask & (neighbours >= 3)


def labels(with_players, without_players, players, camera, fov, min_pixels=40):
    """YOLO boxes (cx, cy, w, h, all 0-1) for each visible player."""
    h, w = with_players.shape[:2]
    mask = changed_mask(with_players, without_players)
    boxes = []
    for player in players:
        box = player_box(player, camera, (w, h), fov)
        if box is not None:
            boxes.append(box)
    # Nearest first: where gates overlap, the nearer player owns the pixel.
    boxes.sort(key=lambda b: b[4])
    owned = np.zeros_like(mask)
    out = []
    for x0, y0, x1, y1, _ in boxes:
        gx, gy = (x1 - x0) * GATE + 3, (y1 - y0) * GATE + 3
        c0, r0 = max(0, int(x0 - gx)), max(0, int(y0 - gy))
        c1, r1 = min(w, int(math.ceil(x1 + gx))), min(h, int(math.ceil(y1 + gy)))
        if c1 <= c0 or r1 <= r0:
            continue
        mine = mask[r0:r1, c0:c1] & ~owned[r0:r1, c0:c1]
        area = (x1 - x0) * (y1 - y0)
        if mine.sum() < max(min_pixels, 0.05 * area):
            continue
        # Only players that get a box claim pixels, so a rejected one can't
        # hide the players behind it.
        owned[r0:r1, c0:c1] |= mine
        rows, cols = np.nonzero(mine)
        bx0, bx1 = c0 + cols.min(), c0 + cols.max() + 1
        by0, by1 = r0 + rows.min(), r0 + rows.max() + 1
        out.append(((bx0 + bx1) / 2 / w, (by0 + by1) / 2 / h, (bx1 - bx0) / w, (by1 - by0) / h))
    return out
