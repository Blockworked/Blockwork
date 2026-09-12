<script setup lang="ts">
import { computed } from 'vue';
import { state } from '../store';
import { setTitle, setMacroSpeedMultiplier } from '../tauri';
import InstructionSidebar from './InstructionSidebar.vue';
import { Canvas } from 'blockstitch';
import EditorToolbar from './EditorToolbar.vue';
import ContextMenu from './ContextMenu.vue';
import DetailsDialog from './DetailsDialog.vue';
import CustomBlockCanvasColorSync from './CustomBlockCanvasColorSync.vue';

const isRecording = computed(() => state.recording_phase.phase === 'Active');
const speedMultiplier = computed(() => state.current_macro?.speed_multiplier ?? 1);
const speedFillPercent = computed(() => {
  const min = 0.1;
  const max = 10;
  const pct = ((speedMultiplier.value - min) / (max - min)) * 100;
  return `${Math.min(100, Math.max(0, pct))}%`;
});

function onTitleInput(e: Event) {
  setTitle((e.target as HTMLInputElement).value);
}

function onSpeedRangeInput(e: Event) {
  setMacroSpeedMultiplier(Number((e.target as HTMLInputElement).value));
}

function onSpeedNumberChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const value = parseFloat(input.value);
  if (!Number.isNaN(value)) {
    setMacroSpeedMultiplier(value);
  } else {
    input.value = String(speedMultiplier.value);
  }
}
</script>

<template>
  <div id="macro-editor" :class="{ recording: isRecording }">
    <div class="editor-title-row">
      <label for="macro-title">Title</label>
      <input type="text" id="macro-title" placeholder="Macro name" :value="state.current_macro?.name" @input="onTitleInput">
      <div class="speed-control" title="Scales every Wait instruction in this macro">
        <label for="macro-speed">Speed</label>
        <input
          type="range"
          id="macro-speed"
          min="0.1"
          max="10"
          step="any"
          :disabled="!state.current_macro"
          :value="speedMultiplier"
          :style="{ '--fill': speedFillPercent }"
          @input="onSpeedRangeInput"
        >
        <input
          type="text"
          class="speed-number"
          inputmode="decimal"
          :disabled="!state.current_macro"
          :value="String(speedMultiplier)"
          @change="onSpeedNumberChange"
        >
        <span class="speed-unit">x</span>
      </div>
    </div>

    <div class="editor-content-area">
      <InstructionSidebar />
      <Canvas>
        <template #overlay>
          <div id="recording-overlay" class="recording-overlay" v-show="isRecording">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="6" fill="currentColor"></circle></svg>
            <span>Recording…</span>
            <span class="recording-overlay-hint">Adding and removing instructions is disabled while recording.</span>
          </div>
        </template>
        <template #context-menu>
          <ContextMenu />
        </template>
      </Canvas>
    </div>

    <EditorToolbar />
    <DetailsDialog />
    <CustomBlockCanvasColorSync />
  </div>
</template>
