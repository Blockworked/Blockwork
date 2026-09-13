<script setup lang="ts">
// The element picker's own window (a second native window, see
// `start_element_picker` in src-tauri and `main.ts`'s `#/element-picker`
// branch) — transparent, borderless, spanning every monitor, and NOT
// click-through: it deliberately receives every mouse event itself so the
// app being inspected never sees the confirming click, same interaction
// shape as a color-picker eyedropper.
import { onMounted, onUnmounted, ref } from 'vue';
import { invoke } from './bridge';
import { emit } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ElementPickerGeometryDto, PickedElementDto, WindowAtPointDto } from './types';

const highlight = ref<{ left: number; top: number; width: number; height: number } | null>(null);
const label = ref('');
const labelPos = ref({ left: 0, top: 0 });

let lastHit: PickedElementDto | null = null;
let lastWindow: WindowAtPointDto | null = null;
let settled = false;
let throttled = false;
let hitTestInFlight = false;
const THROTTLE_MS = 30;

// The logical-space origin `start_element_picker` opened this window at
// (fetched on mount — see `element_picker_geometry`), needed because
// `outerPosition()` is never reported on Wayland. No per-monitor scale
// correction is needed on top of this: a compositor composites a client's
// single surface into the shared *logical* desktop coordinate space
// uniformly (that's the entire point of per-monitor scale factors — to
// make one logical pixel the same real-world size everywhere), so CSS
// pixels (`clientX`/`clientY`) already line up 1:1 with the logical
// coordinates every other window/AT-SPI query here uses, on every monitor,
// regardless of that monitor's own native scale. An earlier version of this
// file scaled by `windowScale / monitorScale` per point, which was based on
// the opposite (incorrect) assumption and produced exactly the "highlight
// box is the wrong size and in the wrong place" bug this replaces.
let geometry: ElementPickerGeometryDto | null = null;
let windowScale = 1;
let originX = 0;
let originY = 0;

// Cursor (CSS px, top-left relative) → global logical px for hit-testing.
function toScreen(clientX: number, clientY: number): { x: number; y: number } {
  return { x: Math.round(originX + clientX), y: Math.round(originY + clientY) };
}

// Global logical px → CSS px, for drawing the highlight box over a hit
// (the inverse of `toScreen`).
function toCss(gx: number, gy: number): { x: number; y: number } {
  return { x: gx - originX, y: gy - originY };
}

async function settle(picked: PickedElementDto | null) {
  if (settled) return;
  settled = true;
  try {
    await emit(picked ? 'element-picked' : 'element-pick-cancelled', picked ?? null);
  } finally {
    await invoke<void>('close_element_picker').catch(err =>
      console.error('element picker: failed to close overlay window', err),
    );
  }
}

async function onMouseMove(e: MouseEvent) {
  if (throttled || hitTestInFlight) return;
  throttled = true;
  setTimeout(() => (throttled = false), THROTTLE_MS);
  hitTestInFlight = true;

  const point = toScreen(e.clientX, e.clientY);
  const hit = await invoke<PickedElementDto | null>('hit_test_element_at', { x: point.x, y: point.y, knownWindow: lastWindow }).catch(() => null);
  hitTestInFlight = false;
  lastHit = hit;
  if (hit) {
    lastWindow = hit.window;
    const topLeft = toCss(hit.x, hit.y);
    const bottomRight = toCss(hit.x + hit.width, hit.y + hit.height);
    highlight.value = { left: topLeft.x, top: topLeft.y, width: bottomRight.x - topLeft.x, height: bottomRight.y - topLeft.y };
    // Window-level picks (no AT-SPI element under the cursor — e.g. apps
    // without an accessibility bridge) carry empty name/role with the
    // window's own bounds; label them as such so a full-window highlight
    // reads as intentional rather than broken.
    label.value = hit.name || hit.role
      ? `${hit.app} · ${hit.window_title} · ${hit.role} "${hit.name}"`
      : `${hit.app} · ${hit.window_title} (window)`;
    labelPos.value = { left: e.clientX + 16, top: e.clientY + 16 };
  } else {
    highlight.value = null;
    label.value = '';
  }
}

function onClick() {
  settle(lastHit);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') settle(null);
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
}

let unlistenClose: (() => void) | undefined;

onMounted(async () => {
  // Registered FIRST and synchronously, before anything that could fail or
  // hang — clicking anywhere (this listener) or pressing Escape must always
  // be able to close the picker, independent of whether the position lookup
  // below succeeds.
  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('click', onClick);
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('contextmenu', onContextMenu);

  try {
    // Chromium's page zoom is a per-origin preference, not per-window: this
    // freshly-created webview otherwise silently inherits whatever zoom the
    // user last left the main window at, which throws off every CSS-px ↔
    // logical-px assumption below (see `reset_element_picker_zoom`'s doc
    // comment). Awaited before anything reads a size/position, and done
    // from here — once this component has actually mounted — rather than
    // right after the window is built on the Rust side, since that's early
    // enough to race Chromium's own zoom-level restoration and lose.
    await invoke<void>('reset_element_picker_zoom').catch(() => {});

    const win = getCurrentWindow();
    try {
      windowScale = await win.scaleFactor();
    } catch {
      windowScale = window.devicePixelRatio || 1;
    }
    if (!(windowScale > 0)) windowScale = 1;
    // Authoritative origin + monitor scales from the backend (see
    // `ElementPickerGeometryDto`). Falls back to the old `outerPosition()`
    // lookup if the command is ever unavailable — better a possibly-wrong
    // highlight position than a stuck window (the escape hatches above are
    // already wired and don't depend on any of this).
    try {
      geometry = await invoke<ElementPickerGeometryDto | null>('element_picker_geometry');
    } catch {
      geometry = null;
    }
    if (geometry) {
      originX = geometry.origin_x;
      originY = geometry.origin_y;
    } else {
      const physicalOrigin = await win.outerPosition();
      const logicalOrigin = physicalOrigin.toLogical(windowScale);
      originX = logicalOrigin.x;
      originY = logicalOrigin.y;
    }

    // Covers every other way the overlay could go away (Alt+F4, the window
    // manager, etc.) so a caller's `pickElement()` promise never hangs.
    unlistenClose = await win.onCloseRequested(() => {
      if (!settled) {
        settled = true;
        emit('element-pick-cancelled', null).catch(() => {});
      }
    });
  } catch (err) {
    console.error('element picker: failed to read window position, hit-testing may be misaligned', err);
  }
});

onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove);
  window.removeEventListener('click', onClick);
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('contextmenu', onContextMenu);
  unlistenClose?.();
});
</script>

<template>
  <div class="picker-root">
    <div v-if="highlight" class="highlight" :style="{ left: highlight.left + 'px', top: highlight.top + 'px', width: highlight.width + 'px', height: highlight.height + 'px' }" />
    <div v-if="label" class="label" :style="{ left: labelPos.left + 'px', top: labelPos.top + 'px' }">{{ label }}</div>
    <div class="hint">Click to pick · Esc to cancel</div>
    <!-- Always clickable regardless of hit-testing state — a redundant,
         independent way out if anything else about the picker misbehaves. -->
    <button type="button" class="cancel-btn" @click.stop="settle(null)">✕ Cancel</button>
  </div>
</template>

<style>
/* Global (unscoped, deliberately): a `<style scoped>` block can't reach
   `html`/`body`, and the default UA/page background is opaque white, which
   would otherwise paint over the whole desktop instead of showing it
   through this window's transparent surface. Safe to be global — this
   component is the only thing ever mounted on the `#/element-picker` route
   (see `main.ts`), so it never shares a page with the main app's own CSS. */
html, body {
  background: transparent !important;
  margin: 0;
  overflow: hidden;
}
</style>

<style scoped>
.picker-root {
  position: fixed;
  inset: 0;
  background: transparent;
  cursor: crosshair;
  overflow: hidden;
}
.highlight {
  position: absolute;
  border: 2px solid #4c97ff;
  background: rgba(76, 151, 255, 0.15);
  pointer-events: none;
  box-sizing: border-box;
}
.label {
  position: absolute;
  background: #1e1e1e;
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
  font: 12px sans-serif;
  white-space: nowrap;
  pointer-events: none;
}
.hint {
  position: fixed;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  background: #1e1e1e;
  color: #fff;
  padding: 6px 14px;
  border-radius: 6px;
  font: 13px sans-serif;
  pointer-events: none;
}
.cancel-btn {
  position: fixed;
  top: 16px;
  right: 16px;
  background: #1e1e1e;
  color: #fff;
  border: 1px solid #444;
  padding: 6px 14px;
  border-radius: 6px;
  font: 13px sans-serif;
  cursor: pointer;
}
.cancel-btn:hover {
  background: #333;
}
</style>
