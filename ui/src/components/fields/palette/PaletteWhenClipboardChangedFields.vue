<script setup lang="ts">
import { ClipboardCheck } from 'lucide-vue-next';
import type { InstructionDto } from '../../../types';

// No fields to edit, matches WhenClipboardChangedFields.vue.
defineProps<{ instruction: Extract<InstructionDto, { type: 'WhenClipboardChanged' }> }>();
</script>

<template>
  <ClipboardCheck />
  <span class="instruction-label when-ran-label">When Clipboard Changes</span>
</template>
