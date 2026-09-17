<script setup lang="ts">
// Static preview only - mirrors PaletteIfFields.vue's split (see its
// comment) and WhileFields.vue's own real-canvas shape.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'While' }>; part: 'head' | 'body' }>();
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">while</span>
    <span class="value-block value-hex-blank"><span class="value-op value-hex-blank-spacer">&nbsp;</span></span>
  </template>
  <div v-else class="wrap-mouth" />
</template>
