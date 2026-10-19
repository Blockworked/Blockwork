# Community blocks

The block library Blockwork shows under My Blocks > Browse library. The app
reads `index.json` from this folder on the `master` branch, then downloads a
pack when someone adds it and checks it against the SHA-256 the index lists.

## Adding a pack

1. Make the blocks in a macro, then right-click one in the My Blocks palette
   and pick "Share blocks as a pack…". Blocks they call come along.
2. Put the `.json` file in `packs/`. Give it a unique `id`; keep the same id
   for later versions so they replace the old blocks instead of adding copies.
3. Run `python3 scripts/build-block-index.py` to update `index.json`.
4. Open a pull request.

Reviewers: read the whole pack. Blockwork shows people what a pack's blocks
can do before they're added, but a pack in this library should not run
commands, go online, write files or read secrets unless that's plainly its
purpose.

## Other libraries

Anyone can host a library: an `index.json` in this format plus the pack
files, over `https://`. People add its address in Settings > Block library.
