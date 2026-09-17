<script setup lang="ts">
// Static preview only - mirrors PaletteIfFields.vue's split (see its
// comment) and ForeverFields.vue's own real-canvas shape.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'Forever' }>; part: 'head' | 'body' }>();
</script>

<template>
  <span v-if="part === 'head'" class="instruction-label">forever</span>
  <div v-else class="wrap-mouth" />
</template>
