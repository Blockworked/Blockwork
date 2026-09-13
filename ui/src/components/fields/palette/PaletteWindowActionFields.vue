<script setup lang="ts">
// Sidebar prefab preview — literal-only fields, same spirit as
// PaletteTextFields.vue/PaletteListCommandFields.vue (the sidebar refuses
// value drops outright, so `app`/`title_contains` are always plain text and
// MoveWindow's `x`/`y` are always plain numbers here). App/title only show up
// once their scope is picked, matching WindowActionFields.vue.
import { AutosizeInput, PaletteNumberField, AppDropdown } from 'blockstitch';
import { textValue } from '../../../types';
import type { ElementScope, InstructionDto } from '../../../types';

type WindowAction = Extract<InstructionDto, { type: 'FocusWindow' | 'MinimizeWindow' | 'MaximizeWindow' | 'MoveWindow' }>;
const props = defineProps<{ instruction: WindowAction }>();

const VERBS: Record<WindowAction['type'], string> = {
  FocusWindow: 'focus',
  MinimizeWindow: 'minimize',
  MaximizeWindow: 'maximize',
  MoveWindow: 'move',
};

function onScopeChange(v: string) {
  props.instruction.scope = v as ElementScope;
}
function setApp(v: string) {
  props.instruction.app = textValue(v);
}
function setTitle(v: string) {
  props.instruction.title_contains = textValue(v);
}
</script>

<template>
  <span class="instruction-label">{{ VERBS[props.instruction.type] }} window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <template v-if="props.instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <AutosizeInput :model-value="props.instruction.app.kind === 'Text' ? props.instruction.app.value : ''" :min-chars="4" @update:model-value="setApp" />
  </template>
  <template v-if="props.instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <AutosizeInput :model-value="props.instruction.title_contains.kind === 'Text' ? props.instruction.title_contains.value : ''" :min-chars="4" @update:model-value="setTitle" />
  </template>
  <template v-if="props.instruction.type === 'MoveWindow'">
    <span class="instruction-label">to x</span>
    <PaletteNumberField v-model="props.instruction.x" />
    <span class="instruction-label">y</span>
    <PaletteNumberField v-model="props.instruction.y" />
  </template>
</template>
