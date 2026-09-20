<script setup lang="ts">
import { ref } from 'vue';
import { PaletteNumberField } from 'blockstitch';
import { AppDropdown } from 'blockstitch';
import { requestAbsoluteMouseSupport } from '../../../tauri';
import type { Coordinate, InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'MoveMouse' }> }>();
const error = ref<string | null>(null);

async function onCoordinateChange(v: string) {
  error.value = null;
  try {
    if (v === 'Absolute') await requestAbsoluteMouseSupport();
    props.instruction.coordinate = v as Coordinate;
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <span class="instruction-label">Move mouse:</span>
  <PaletteNumberField v-model="props.instruction.x" placeholder="X" />
  <PaletteNumberField v-model="props.instruction.y" placeholder="Y" />
  <AppDropdown
    :options="['Absolute', 'Relative']"
    :model-value="instruction.coordinate"
    class-name="dd-compact"
    @update:model-value="onCoordinateChange"
  />
  <span v-if="error" class="instruction-error">{{ error }}</span>
</template>
