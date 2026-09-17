<script setup lang="ts">
// Static preview only - mirrors PaletteIfFields.vue's split (see its
// comment) and RepeatFields.vue's own real-canvas shape.
import { PaletteNumberField } from 'blockstitch';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Repeat' }>; part: 'head' | 'body' }>();
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">repeat</span>
    <PaletteNumberField v-model="props.instruction.count" />
  </template>
  <div v-else class="wrap-mouth" />
</template>
