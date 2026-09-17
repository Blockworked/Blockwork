<script setup lang="ts">
import { computed } from 'vue';
import { TriangleAlert } from 'lucide-vue-next';
import { state } from '../store';

// Vue only creates each banner's DOM node once when it flips from hidden to
// shown (v-if), so applying the entrance animation class unconditionally
// still only plays it once per appearance - no manual "already shown"
// bookkeeping needed here (unlike the original's full-rebuild-per-render
// vanilla version, which had to track this by hand).
const grabMissing = computed(() => !state.grab_available);
const emulatorMissing = computed(() => !state.emulator_available);
const razerPermissionMissing = computed(() => state.razer_permission_warning);
</script>

<template>
  <div id="warnings-container">
    <div v-if="grabMissing" class="warning-banner banner-enter">
      <TriangleAlert />
      <span>Global hotkeys unavailable.<br>Check system permissions (Accessibility / input group).</span>
    </div>
    <div v-if="emulatorMissing" class="warning-banner banner-enter">
      <TriangleAlert />
      <span>Input emulation unavailable.<br>Check system permissions.</span>
    </div>
    <div v-if="razerPermissionMissing" class="warning-banner banner-enter">
      <TriangleAlert />
      <span>Razer device access unavailable.<br>Check system permissions (openrazer / plugdev group).</span>
    </div>
  </div>
</template>
