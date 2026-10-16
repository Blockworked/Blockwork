# Minecraft player model

Builds `mc-player-320-v1`, the built-in model behind Blockwork's detection
blocks: YOLOv8n fine-tuned to find Minecraft players, 320 px input.

For automation where it's allowed (single-player, your own servers, testing).
Don't use it against other players on servers that forbid it.

## How labeling works

`capture.py` drives a local Fabric server with the Carpet mod over RCON. For
each sample it spawns fake players, places your spectator player as the
camera, freezes the world with `/tick freeze`, and takes two F2 screenshots:
with the players, then with them moved away. Pixels that changed are visible
player pixels; projecting each player's known position splits them per
player (`label.py`). Nothing outside the game window is captured.

## 1. Set up Minecraft (you do this once)

1. Install Minecraft Java Edition and pick a version Carpet supports, 1.20.3
   or newer (for `/tick`).
2. Make a server folder. Get the Fabric server launcher for that version from
   fabricmc.net, and the matching Carpet jar from Modrinth or
   github.com/gnembon/fabric-carpet into its `mods/` folder.
3. Start the server once, read the EULA it points to, and set `eula=true` in
   `eula.txt` if you accept it.
4. In `server.properties` set:
   ```
   enable-rcon=true
   rcon.port=25575
   rcon.password=<something>
   online-mode=true
   ```
   then start the server again and run `op <your name>` in its console.
5. Join `localhost` with a vanilla client of the same version. In Options:
   FOV Normal (70), Clouds Off, Particles Minimal, Entity Shadows Off, View
   Bobbing Off, FOV Effects 0%. Keep the window size fixed while capturing.

## 2. Capture

```bash
python3 -m venv venv && venv/bin/pip install numpy pillow evdev
venv/bin/python capture.py --password <rcon password> --mc-dir ~/.minecraft --camera <your name> --samples 1500
```

Click into the game window within 5 seconds, then leave the keyboard and
mouse alone (it presses F1 and F2 through a virtual keyboard, which needs
access to `/dev/uinput`). 1500 samples take roughly an hour. It roams far
from spawn, so expect new chunks to generate.

Skins come from the fake players' names: in online mode Carpet uses the
skin of the account with that name, and unknown names get default skins. For
more variety, pass `--names names.txt` with one name per line.

Check the labels before training:

```bash
venv/bin/python preview.py dataset 30   # boxes drawn into dataset/preview/
```

## 3. Train

Needs an NVIDIA GPU (CPU works, slowly).

```bash
venv/bin/pip install torch torchvision --index-url https://download.pytorch.org/whl/cu128
venv/bin/pip install ultralytics onnx onnxslim
venv/bin/python train.py --data dataset
```

It prints validation scores, exports `dataset/mc-player-320-v1.onnx`, copies
it into Blockwork's models folder, and prints its SHA-256. Once the file is
hosted, put its URL, hash and size in `KNOWN` in
`blockwork-core/src/detect/models.rs` so Blockwork can download it.

The starting weights are Ultralytics YOLOv8n, which are AGPL-3.0, so the
fine-tuned model is too.

## Tests

```bash
python3 -m unittest test_label test_capture
```
