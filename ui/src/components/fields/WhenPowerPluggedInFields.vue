<script setup lang="ts">
import { PlugZap } from 'lucide-vue-next';
import type { InstrPath, InstructionDto } from '../../types';

// No fields to edit - strandId/path/instruction are accepted (like every
// other *Fields component) purely so Vue treats them as declared props
// rather than falling them through as DOM attrs onto the two root nodes
// below.
defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'WhenPowerPluggedIn' }> }>();
</script>

<template>
  <PlugZap />
  <span class="instruction-label when-ran-label">When Power Plugged In</span>
</template>
