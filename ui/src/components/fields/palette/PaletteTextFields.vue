<script setup lang="ts">
// Sidebar prefab preview - like PaletteNumberField, always a literal (never
// an operator or dropped-in block): the sidebar refuses value drops outright
// (see canvasDrag.ts's isOverSidebar), so this only ever edits the plain
// `Text` leaf `defaultInstruction('Text')` seeds.
import { AutosizeInput } from 'blockstitch';
import { textValue } from '../../../types';
import type { InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'Text' }> }>();

function onChange(v: string) {
  props.instruction.text = textValue(v);
}
</script>

<template>
  <span class="instruction-label">Text:</span>
  <AutosizeInput
    :model-value="instruction.text.kind === 'Text' ? instruction.text.value : ''"
    :min-chars="6"
    @update:model-value="onChange"
  />
</template>
