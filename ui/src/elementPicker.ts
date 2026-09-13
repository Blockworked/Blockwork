// Drives the "pick from screen" element picker's request half — opens the
// overlay window (`ElementPickerOverlay.vue`, a second native window) and
// waits for it to report back a pick or a cancellation. Goes through
// `./bridge` (not straight `@tauri-apps/api`) for the same dev-bridge-aware
// behavior every other command here has — the picker just isn't *available*
// there (see `dev_bridge.rs`), so `start_element_picker` rejects cleanly
// instead of hanging.
import { invoke, listen } from './bridge';
import type { ElementPickerGeometryDto, PickedElementDto } from './types';

/**
 * Opens the screen element picker and resolves with the user's pick, or
 * `null` if they cancelled (Escape, closed the overlay another way, or the
 * picker isn't available in this environment).
 */
export async function pickElement(): Promise<PickedElementDto | null> {
  try {
    // The geometry return value belongs to the overlay window (it fetches
    // the same via `element_picker_geometry`); the opener only needs the
    // open to have succeeded.
    await invoke<ElementPickerGeometryDto>('start_element_picker');
  } catch (e) {
    console.warn('element picker unavailable:', e);
    return null;
  }

  return new Promise<PickedElementDto | null>(resolve => {
    let settled = false;
    let unlistenPicked: (() => void) | undefined;
    let unlistenCancelled: (() => void) | undefined;

    const settle = (value: PickedElementDto | null) => {
      if (settled) return;
      settled = true;
      unlistenPicked?.();
      unlistenCancelled?.();
      resolve(value);
    };

    // The overlay window emits these as a broadcast (`emit`, not `emitTo`)
    // so it doesn't need to know the main window's current label (which
    // changes across a tray-triggered rebuild — see `tray.rs`'s
    // `rebuild_main_window`).
    listen<PickedElementDto>('element-picked', evt => settle(evt.payload)).then(u => (settled ? u() : (unlistenPicked = u)));
    listen('element-pick-cancelled', () => settle(null)).then(u => (settled ? u() : (unlistenCancelled = u)));
  });
}
