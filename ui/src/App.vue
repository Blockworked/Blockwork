<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { state, initState } from './store';
import { cancelComboCapture, comboCaptureEvent, keyCaptureEvent, resetZoom } from './tauri';
import { NO_COMBO_ACTIONS, type NamedActionType } from './constants';
import MainPage from './components/MainPage.vue';
import SettingsPage from './components/SettingsPage.vue';

onMounted(() => {
  initState();
  document.addEventListener('keydown', onKeydown);
});
onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown);
});

async function onKeydown(e: KeyboardEvent) {
  if (state.key_capture != null) {
    e.preventDefault();
    await keyCaptureEvent(e.code, e.key);
    return;
  }
  if (state.combo_capture != null) {
    e.preventDefault();
    if (e.key === 'Escape') {
      await cancelComboCapture();
      return;
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;
    const capturingAction = state.combo_capture.kind === 'Named' ? state.combo_capture.action?.type as NamedActionType : null;
    const noCombo = capturingAction != null && NO_COMBO_ACTIONS.has(capturingAction);
    const modifiers = noCombo ? 0 : (e.ctrlKey ? 1 : 0) | (e.shiftKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0);
    await comboCaptureEvent(e.code, modifiers);
    return;
  }
  // Reset Chromium page zoom to 100%. Handled here rather than by the
  // browser's own Ctrl+0 accelerator, which this CEF runtime can't be relied
  // on to deliver (opt-in per webview; absent entirely on Alloy-style
  // webviews). preventDefault() keeps the browser from also applying its own
  // zoom step where the accelerator does exist, so the reset happens once.
  // Key/combo capture above takes precedence while active.
  if (!e.repeat && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey &&
      (e.code === 'Digit0' || e.code === 'Numpad0')) {
    e.preventDefault();
    await resetZoom();
  }
}
</script>

<template>
  <MainPage :class="{ 'page-hidden': state.page === 'Settings' }" />
  <SettingsPage :class="{ 'page-hidden': state.page !== 'Settings' }" />
</template>
