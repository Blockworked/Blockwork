<script setup lang="ts">
// Same scope-toggle-and-pick block as WindowActionFields.vue/
// ClickElementFields.vue, plus a single ValueBlock for the literal text.
import { editInstruction } from '../../tauri';
import { ValueBlock, AppDropdown } from 'blockstitch';
import { Pipette } from 'lucide-vue-next';
import { fieldLocation, textValue } from '../../types';
import { pickElement } from '../../elementPicker';
import type { ElementScope, InstrPath, InstructionDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'TypeTextInWindow' }> }>();

function onScopeChange(v: string) {
  editInstruction(props.strandId, props.path, { ...props.instruction, scope: v as ElementScope });
}

async function onPick() {
  const picked = await pickElement();
  if (!picked) return;
  editInstruction(props.strandId, props.path, { ...props.instruction, scope: 'Specific Window', app: textValue(picked.app), title_contains: textValue(picked.window_title) });
}
</script>

<template>
  <span class="instruction-label">type text in window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <button type="button" class="btn-chip" title="Pick a window from the screen" @click="onPick"><Pipette :size="14" /></button>
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'TypeTextInWindowApp')" :value="instruction.app" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'TypeTextInWindowTitle')" :value="instruction.title_contains" />
  </template>
  <span class="instruction-label">text</span>
  <ValueBlock :location="fieldLocation(strandId, path, 'TypeTextInWindowText')" :value="instruction.text" />
</template>
