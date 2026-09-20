<script setup lang="ts">
import { ref } from 'vue';
import { editInstruction } from '../../tauri';
import { requestAbsoluteMouseSupport } from '../../tauri';
import { ValueBlock } from 'blockstitch';
import { AppDropdown } from 'blockstitch';
import { fieldLocation } from '../../types';
import type { Coordinate, InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();
const error = ref<string | null>(null);

async function onCoordinateChange(v: string) {
  error.value = null;
  try {
    if (v === 'Absolute') await requestAbsoluteMouseSupport();
    await editInstruction(props.strandId, props.path, {
      id: props.instruction.id, type: 'MoveMouse', x: props.instruction.x, y: props.instruction.y, coordinate: v as Coordinate,
    });
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'MoveMouseX')" :value="instruction.x" placeholder="X" />
  <ValueBlock :location="fieldLocation(strandId, path, 'MoveMouseY')" :value="instruction.y" placeholder="Y" />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
  <span v-if="error" class="instruction-error">{{ error }}</span>
</template>
