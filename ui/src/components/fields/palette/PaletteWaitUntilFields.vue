<script setup lang="ts">
// Static preview only — no live editable condition, same spirit as
// PaletteIfFields.vue's blank hexagon (the sidebar never accepts value drops).
import type { InstructionDto } from '../../../types';

defineProps<{ instruction: Extract<InstructionDto, { type: 'WaitUntil' }> }>();
</script>

<template>
  <span class="instruction-label">wait until</span>
  <span class="value-block value-hex-blank"><span class="value-op value-hex-blank-spacer">&nbsp;</span></span>
</template>
