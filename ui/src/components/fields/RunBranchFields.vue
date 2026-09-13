<script setup lang="ts">
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'RunBranch' }> }>();
</script>

<template>
  <span class="instruction-label">{{ instruction.name }}</span>
</template>
