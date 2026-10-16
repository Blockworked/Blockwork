"""Checks the RCON client and reply parsing: python3 -m unittest test_capture"""

import socket
import struct
import threading
import unittest

import capture


def fake_server(password, replies):
    """A one-connection RCON server answering commands from `replies`."""
    srv = socket.socket()
    srv.bind(("127.0.0.1", 0))
    srv.listen(1)
    seen = []

    def serve():
        conn, _ = srv.accept()
        while True:
            head = conn.recv(4)
            if not head:
                break
            size = struct.unpack("<i", head)[0]
            data = b""
            while len(data) < size:
                data += conn.recv(size - len(data))
            rid, kind = struct.unpack("<ii", data[:8])
            body = data[8:-2].decode()
            if kind == 3:
                rid, out = (rid if body == password else -1), ""
            else:
                seen.append(body)
                out = replies.get(body, "")
            payload = struct.pack("<ii", rid, 0) + out.encode() + b"\0\0"
            conn.sendall(struct.pack("<i", len(payload)) + payload)

    threading.Thread(target=serve, daemon=True).start()
    return srv.getsockname()[1], seen


class RconTest(unittest.TestCase):
    def test_commands_round_trip(self):
        port, seen = fake_server("pw", {"data get entity bot Pos": "bot has the following entity data: [1.5d, 64.0d, -2.25d]"})
        rcon = capture.Rcon("127.0.0.1", port, "pw")
        reply = rcon("data get entity bot Pos")
        self.assertEqual(capture.numbers(reply), [1.5, 64.0, -2.25])
        self.assertEqual(seen, ["data get entity bot Pos"])

    def test_a_wrong_password_exits(self):
        port, _ = fake_server("pw", {})
        with self.assertRaises(SystemExit):
            capture.Rcon("127.0.0.1", port, "nope")

    def test_the_camera_rises_out_of_terrain(self):
        air = "execute if block 3 72 -5 minecraft:air"
        port, seen = fake_server("pw", {air: "Test passed"})
        rcon = capture.Rcon("127.0.0.1", port, "pw")
        # 70 and 71 are solid (the fake answers nothing), 72 is air.
        self.assertEqual(capture.clear_eye(rcon, (3.5, 70.2, -4.5)), (3.5, 72.2, -4.5))
        self.assertEqual(len(seen), 3)
        self.assertIsNone(capture.clear_eye(rcon, (3.5, 0.0, -4.5), tries=2))

    def test_rotations_and_look_at(self):
        self.assertEqual(capture.numbers("cam has the following entity data: [-90.5f, 12.0f]"), [-90.5, 12.0])
        # Looking from the origin toward +Z is yaw 0, toward -X yaw 90, down is positive pitch.
        self.assertAlmostEqual(capture.look_at((0, 0, 0), (0, 0, 5))[0], 0)
        self.assertAlmostEqual(capture.look_at((0, 0, 0), (-5, 0, 0))[0], 90)
        self.assertGreater(capture.look_at((0, 5, 0), (0, 0, 5))[1], 0)


if __name__ == "__main__":
    unittest.main()
