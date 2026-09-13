<script setup lang="ts">
// Sidebar prefab preview — like PaletteTextFields, always a literal (never an
// operator or dropped-in block): the sidebar refuses value drops outright
// (see canvasDrag.ts's isOverSidebar), so this only ever edits the plain
// `Text` leaf `defaultInstruction('SetClipboard')` seeds.
import { AutosizeInput } from 'blockstitch';
import { textValue } from '../../../types';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'SetClipboard' }> }>();

function onChange(v: string) {
  props.instruction.value = textValue(v);
}
</script>

<template>
  <span class="instruction-label">set clipboard to</span>
  <AutosizeInput
    :model-value="instruction.value.kind === 'Text' ? instruction.value.value : ''"
    :min-chars="6"
    @update:model-value="onChange"
  />
</template>
