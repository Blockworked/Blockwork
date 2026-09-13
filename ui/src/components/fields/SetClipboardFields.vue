<script setup lang="ts">
import { ValueBlock } from 'blockstitch';
import { fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'SetClipboard' }> }>();
</script>

<template>
  <span class="instruction-label">set clipboard to</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'SetClipboardValue')" :value="instruction.value" />
</template>
