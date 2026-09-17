<script setup lang="ts">
import { computed, watch } from 'vue';
import { state } from '../../../store';
import { clearStandaloneKeyCapture, startStandaloneKeyCapture } from '../../../tauri';
import { AppDropdown } from 'blockstitch';
import type { InstructionDto, KeyDirection } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Key' }> }>();

// This prefab has no real strand/index for the backend's normal capture flow
// to write into, so it uses the "standalone" capture variant instead: the
// backend parks the captured key in state.standalone_key rather than writing
// it into a strand, and we copy it into the local instruction ourselves.
const isCapturing = computed(() => state.key_capture?.kind === 'Standalone');

// By the time this fires, the backend has already cleared key_capture back
// to null in the same state snapshot - isCapturing is stale by then, so this
// can't gate on it. Standalone capture only ever has one consumer (this
// prefab), so applying unconditionally is safe.
watch(() => state.standalone_key, key => {
  if (key == null) return;
  props.instruction.key = key;
  clearStandaloneKeyCapture();
});

function onDirectionChange(dir: string) {
  props.instruction.direction = dir as KeyDirection;
}
</script>

<template>
  <span class="instruction-label">Key:</span>
  <button
    class="btn-chip key-capture-btn"
    :class="{ capturing: isCapturing }"
    @click="startStandaloneKeyCapture()"
  >{{ isCapturing ? 'Press any key…' : instruction.key }}</button>
  <AppDropdown
    :options="['Click', 'Press', 'Release']"
    :model-value="instruction.direction"
    class-name="dd-compact"
    @update:model-value="onDirectionChange"
  />
</template>
