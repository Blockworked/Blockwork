<script setup lang="ts">
// Static preview only - see PaletteIfFields.vue's header comment. Mirrors
// IfElseFields.vue's own 'head'/'body' split (then-mouth, "else" mid bar,
// else-mouth).
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'IfElse' }>; part: 'head' | 'body' }>();
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <span class="value-block value-hex-blank"><span class="value-op value-hex-blank-spacer">&nbsp;</span></span>
    <span class="instruction-label">then</span>
  </template>
  <template v-else>
    <div class="wrap-mouth" />
    <div class="wrap-mid-bar"><span class="instruction-label">else</span></div>
    <div class="wrap-mouth" />
  </template>
</template>
