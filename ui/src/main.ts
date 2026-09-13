import { createApp } from 'vue';

// The element-picker overlay is a second native window pointed at this same
// bundle (see `start_element_picker` in src-tauri) — a minimal transparent
// page with none of the main editor's setup, so it's branched off before any
// of that runs. Wrapped in an async IIFE (rather than top-level await) so
// this doesn't depend on the build target supporting top-level await.
(async () => {
  if (location.hash === '#/element-picker') {
    const { default: ElementPickerOverlay } = await import('./ElementPickerOverlay.vue');
    createApp(ElementPickerOverlay).mount('#app');
    return;
  }

  await Promise.all([import('blockstitch/theme.css'), import('./custom-block-color.css'), import('./block-interactions.css')]);
  const [{ default: App }, { setupBlockstitch }] = await Promise.all([import('./App.vue'), import('./blockstitchSetup')]);
  setupBlockstitch();
  createApp(App).mount('#app');
})();
