# Community pack variables

Pack variables are private by default. They are saved with the installed pack,
kept separate from your macro's variables, and shared by that pack's blocks.
Two packs can use the same variable name without sharing its value.

When sharing a pack, expand **Pack variables** to choose how each variable is used:

- **Internal** keeps working state inside the pack.
- **Output** adds a reporter block to the pack's section in Community.
- **Setting** adds an editable value under the installed pack's **Settings** button.

Use custom block inputs with defaults for options that change from call to call.
Settings are for options shared by all calls in that macro. The pack's Settings
panel also has a collapsed **Inspect variables** section showing saved values
from the last completed run.

Existing pack files still import. Existing macros migrate private variables on
load, keeping their saved values and a backup of the original file. Variables used outside the pack, including
floating reporters and names shared by multiple old packs, remain macro
variables so existing reads and writes keep working. Migration uses block
references and pack membership, rather than guessing from variable prefixes.

Pack updates preserve saved values and existing output reporters. Removing a
pack removes its private state and output blocks; legacy shared macro variables
are kept. Changes to pack settings and pack installation can be undone.

Macros saved with pack-owned state use format version 3. Older Blockwork builds
will ask you to update instead of opening a file without its private state.
