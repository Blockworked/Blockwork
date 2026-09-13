<script setup lang="ts">
import { ValueBlock } from 'blockstitch';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'WaitUntil' }> }>();
</script>

<template>
  <span class="instruction-label">wait until</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
</template>
