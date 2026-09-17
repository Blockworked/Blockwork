<script setup lang="ts">
// Static preview only - no live editable condition (the sidebar is never a
// valid value-drop target) and no real nested body - but shaped as the
// actual hollow bracket the real canvas block renders as (see
// PaletteInstructionBlock.vue and style.css's .instruction-row-wrap), not a
// flattened single-line stand-in, so the prefab reads as what it actually
// is. Mirrors IfFields.vue's own 'head'/'body' split.
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'If' }>; part: 'head' | 'body' }>();
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <span class="value-block value-hex-blank"><span class="value-op value-hex-blank-spacer">&nbsp;</span></span>
    <span class="instruction-label">then</span>
  </template>
  <div v-else class="wrap-mouth" />
</template>
