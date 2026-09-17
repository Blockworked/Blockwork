<script setup lang="ts">
import type { InstrPath, InstructionDto } from '../../types';

// No fields to edit - strandId/path/instruction accepted purely so Vue
// treats them as declared props (see WhenRanFields.vue's comment).
defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'EscapeLoop' }> }>();
</script>

<template>
  <span class="instruction-label">escape loop</span>
</template>
