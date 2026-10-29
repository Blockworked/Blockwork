# Blockstitch QML snapshot

Source: https://github.com/Blockworked/blockstitch
Base revision: `0bfc7874bc864024e15ea43cd6aace34b9027d85`
License: GPL-3.0-or-later (as declared by the upstream crate).

The base matches master’s updated Blockstitch pin, including BooleanSlot and
the current shared block editor. This source snapshot includes the shared touchscreen changes developed in the
sibling Blockstitch checkout. Cargo and Android CMake use this same snapshot so
a clean build does not depend on unpublished Git objects or a sibling checkout.
Core/FFI remain pinned to the base revision. Once the QML changes are published
upstream, update the Git pin and remove this temporary QML path dependency.

Local changes: touchscreen block dragging and long press, two-finger canvas
zoom/pan with drag cancellation, selected-gap/free-space palette insertion,
touch-sized common controls, bounded scrolling dialogs, keyboard-aware canvas
focus, and touch regression tests.
