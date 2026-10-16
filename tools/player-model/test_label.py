"""Checks the labeling math on synthetic frames: python3 -m unittest test_label"""

import unittest

import numpy as np

import label


def frames(w=320, h=240):
    rng = np.random.default_rng(1)
    scene = rng.integers(0, 255, (h, w, 4), dtype=np.uint8)
    return scene, scene.copy()


def paint(img, box, color):
    x0, y0, x1, y1 = (int(round(v)) for v in box[:4])
    img[max(0, y0):y1, max(0, x0):x1, :3] = color


class Projection(unittest.TestCase):
    def test_straight_ahead_lands_in_the_middle(self):
        # Camera at the origin facing south (+Z), level.
        (x, y, depth), = label.project([(0, label.EYE, 10)], (0, 0, 0, 0, 0), (320, 240), 70)
        self.assertAlmostEqual(x, 160)
        self.assertAlmostEqual(y, 120)
        self.assertAlmostEqual(depth, 10)

    def test_axes_follow_minecraft_conventions(self):
        cam = (0, 0, 0, 0, 0)
        # Facing south, east (+X) is on the left of the screen; up is up.
        (east_x, _, _), = label.project([(1, label.EYE, 10)], cam, (320, 240), 70)
        (_, up_y, _), = label.project([(0, label.EYE + 1, 10)], cam, (320, 240), 70)
        self.assertLess(east_x, 160)
        self.assertLess(up_y, 120)
        # Yaw 90 faces west (-X); behind the camera projects to None.
        (x, y, _), = label.project([(-10, label.EYE, 0)], (0, 0, 0, 90, 0), (320, 240), 70)
        self.assertAlmostEqual(x, 160)
        self.assertEqual(label.project([(10, label.EYE, 0)], (0, 0, 0, 90, 0), (320, 240), 70), [None])
        # Looking down 30 degrees, a point at eye height rises above center,
        # and one 30 degrees below eye level lands on it.
        (_, y, _), = label.project([(0, label.EYE, 10)], (0, 0, 0, 0, 30), (320, 240), 70)
        self.assertLess(y, 120)
        drop = 10 * np.tan(np.radians(30))
        (_, y, _), = label.project([(0, label.EYE - drop, 10)], (0, 0, 0, 0, 30), (320, 240), 70)
        self.assertAlmostEqual(y, 120)


class Labels(unittest.TestCase):
    def test_a_visible_player_gets_its_painted_box(self):
        cam = (0, 0, 0, 0, 0)
        player = {"pos": (0, 0, 8)}
        box = label.player_box(player, cam, (320, 240), 70)
        with_p, without = frames()
        # Paint a figure a bit inside the hitbox, like a real skin.
        inner = (box[0] + 4, box[1] + 2, box[2] - 4, box[3])
        paint(with_p, inner, (250, 10, 10))
        (cx, cy, w, h), = label.labels(with_p, without, [player], cam, 70)
        x0, y0, x1, y1 = (round(v) for v in inner)
        self.assertAlmostEqual(cx * 320, (x0 + x1) / 2, delta=1)
        self.assertAlmostEqual(w * 320, x1 - x0, delta=1)
        self.assertAlmostEqual(h * 240, y1 - y0, delta=1)

    def test_hidden_and_offscreen_players_get_no_box(self):
        cam = (0, 0, 0, 0, 0)
        with_p, without = frames()
        # Nothing changed on screen: fully occluded. The second is behind the camera.
        players = [{"pos": (0, 0, 8)}, {"pos": (0, 0, -8)}]
        self.assertEqual(label.labels(with_p, without, players, cam, 70), [])

    def test_overlapping_players_split_by_depth(self):
        cam = (0, 0, 0, 0, 0)
        near, far = {"pos": (0.4, 0, 6)}, {"pos": (-0.6, 0, 12)}
        nb, fb = (label.player_box(p, cam, (320, 240), 70) for p in (near, far))
        with_p, without = frames()
        paint(with_p, fb, (10, 250, 10))
        paint(with_p, nb, (250, 10, 10))  # in front, covering part of the far one
        found = sorted(label.labels(with_p, without, [far, near], cam, 70), key=lambda b: -b[3])
        self.assertEqual(len(found), 2)
        self.assertAlmostEqual(found[0][3] * 240, nb[3] - nb[1], delta=2)
        # The far player keeps only what sticks out, never more than its own box.
        self.assertLessEqual(found[1][2] * 320, fb[2] - fb[0] + 1)

    def test_a_player_at_the_lens_doesnt_hide_the_others(self):
        cam = (0, 0, 0, 0, 0)
        visible, at_lens = {"pos": (0, 0, 8)}, {"pos": (0.2, label.EYE - 1.0, 0.45)}
        self.assertIsNone(label.player_box(at_lens, cam, (320, 240), 70))
        box = label.player_box(visible, cam, (320, 240), 70)
        with_p, without = frames()
        paint(with_p, box, (250, 10, 10))
        self.assertEqual(len(label.labels(with_p, without, [at_lens, visible], cam, 70)), 1)

    def test_specks_are_ignored(self):
        with_p, without = frames()
        with_p[50, 50, :3] = with_p[50, 50, :3] ^ 255
        self.assertFalse(label.changed_mask(with_p, without).any())


if __name__ == "__main__":
    unittest.main()
