#pragma once

// Tells the compositor how much of each frameless top-level window is
// transparent shadow, so moves/snapping/screen clamping use the visible frame.
// A no-op on other platforms.
void blockwork_set_window_frame_margin(int margin);
