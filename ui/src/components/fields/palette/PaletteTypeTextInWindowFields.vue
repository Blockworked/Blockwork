<script setup lang="ts">
// Sidebar prefab preview — literal-only fields, same spirit as
// PaletteClickElementFields.vue.
import { AutosizeInput, AppDropdown } from 'blockstitch';
import { textValue } from '../../../types';
import type { ElementScope, InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'TypeTextInWindow' }> }>();

function onScopeChange(v: string) {
  props.instruction.scope = v as ElementScope;
}
function setApp(v: string) {
  props.instruction.app = textValue(v);
}
function setTitle(v: string) {
  props.instruction.title_contains = textValue(v);
}
function setText(v: string) {
  props.instruction.text = textValue(v);
}
</script>

<template>
  <span class="instruction-label">type text in window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <AutosizeInput :model-value="props.instruction.app.kind === 'Text' ? props.instruction.app.value : ''" :min-chars="4" @update:model-value="setApp" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <AutosizeInput :model-value="props.instruction.title_contains.kind === 'Text' ? props.instruction.title_contains.value : ''" :min-chars="4" @update:model-value="setTitle" />
  </template>
  <span class="instruction-label">text</span>
  <AutosizeInput :model-value="props.instruction.text.kind === 'Text' ? props.instruction.text.value : ''" :min-chars="4" @update:model-value="setText" />
</template>
