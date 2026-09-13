<script setup lang="ts">
// Same Focused-Window/Specific-App/Specific-Window toggle as ClickElementFields.vue.
import { editInstruction } from '../../tauri';
import { ValueBlock, AppDropdown } from 'blockstitch';
import { Pipette } from 'lucide-vue-next';
import { fieldLocation, textValue } from '../../types';
import { pickElement } from '../../elementPicker';
import type { ElementScope, InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'SetElementValue' }> }>();

function onScopeChange(v: string) {
  editInstruction(props.strandId, props.path, {
    id: props.instruction.id,
    type: 'SetElementValue',
    scope: v as ElementScope,
    app: props.instruction.app,
    title_contains: props.instruction.title_contains,
    name: props.instruction.name,
    role: props.instruction.role,
    value: props.instruction.value,
  });
}

async function onPick() {
  const picked = await pickElement();
  if (!picked) return;
  editInstruction(props.strandId, props.path, {
    id: props.instruction.id,
    type: 'SetElementValue',
    scope: 'Specific Window',
    app: textValue(picked.app),
    title_contains: textValue(picked.window_title),
    name: textValue(picked.name),
    role: textValue(picked.role),
    value: props.instruction.value,
  });
}
</script>

<template>
  <span class="instruction-label">set value of element in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <button type="button" class="btn-chip" title="Pick an element from the screen" @click="onPick"><Pipette :size="14" /></button>
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'SetElementValueApp')" :value="instruction.app" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'SetElementValueTitle')" :value="instruction.title_contains" />
  </template>
  <span class="instruction-label">named</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'SetElementValueName')" :value="instruction.name" />
  <span class="instruction-label">with role</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'SetElementValueRole')" :value="instruction.role" />
  <span class="instruction-label">to</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'SetElementValueValue')" :value="instruction.value" />
</template>
