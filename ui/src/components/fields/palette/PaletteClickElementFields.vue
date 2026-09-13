<script setup lang="ts">
// Sidebar prefab preview — literal-only fields, same spirit as
// PaletteMoveMouseFields.vue's Absolute/Relative toggle. App/title only
// show up once their scope is picked, matching ClickElementFields.vue.
import { AutosizeInput, AppDropdown } from 'blockstitch';
import { textValue } from '../../../types';
import type { ElementScope, InstructionDto } from '../../../types';

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'ClickElement' }> }>();

function onScopeChange(v: string) {
  props.instruction.scope = v as ElementScope;
}
function setApp(v: string) {
  props.instruction.app = textValue(v);
}
function setTitle(v: string) {
  props.instruction.title_contains = textValue(v);
}
function setName(v: string) {
  props.instruction.name = textValue(v);
}
function setRole(v: string) {
  props.instruction.role = textValue(v);
}
</script>

<template>
  <span class="instruction-label">click element in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <AutosizeInput :model-value="props.instruction.app.kind === 'Text' ? props.instruction.app.value : ''" :min-chars="4" @update:model-value="setApp" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <AutosizeInput :model-value="props.instruction.title_contains.kind === 'Text' ? props.instruction.title_contains.value : ''" :min-chars="4" @update:model-value="setTitle" />
  </template>
  <span class="instruction-label">named</span>
  <AutosizeInput :model-value="props.instruction.name.kind === 'Text' ? props.instruction.name.value : ''" :min-chars="4" @update:model-value="setName" />
  <span class="instruction-label">with role</span>
  <AutosizeInput :model-value="props.instruction.role.kind === 'Text' ? props.instruction.role.value : ''" :min-chars="4" @update:model-value="setRole" />
</template>
