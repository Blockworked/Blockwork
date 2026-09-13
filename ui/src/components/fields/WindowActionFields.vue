<script setup lang="ts">
// Shared by the four window-action instructions, keyed on `instruction.type`
// — same pattern as ListCommandFields.vue for the seven list commands.
// App/title only show up once their scope is picked — "Focused Window"
// (the default) acts on the currently active window directly; "Specific App"
// reveals just the app field, "Specific Window" just the title field. Same
// toggle-driven shape as ClickElementFields.vue.
import { editInstruction } from '../../tauri';
import { ValueBlock, AppDropdown } from 'blockstitch';
import { Pipette } from 'lucide-vue-next';
import { fieldLocation, textValue } from '../../types';
import { pickElement } from '../../elementPicker';
import type { ElementScope, InstrPath, InstructionDto } from '../../types';

type WindowAction = Extract<InstructionDto, { type: 'FocusWindow' | 'MinimizeWindow' | 'MaximizeWindow' | 'MoveWindow' }>;
const props = defineProps<{ strandId: string; path: InstrPath; instruction: WindowAction }>();

const VERBS: Record<WindowAction['type'], string> = {
  FocusWindow: 'focus',
  MinimizeWindow: 'minimize',
  MaximizeWindow: 'maximize',
  MoveWindow: 'move',
};

function onScopeChange(v: string) {
  const scope = v as ElementScope;
  const instruction = props.instruction;
  if (instruction.type === 'MoveWindow') {
    editInstruction(props.strandId, props.path, {
      id: instruction.id,
      type: 'MoveWindow',
      scope,
      app: instruction.app,
      title_contains: instruction.title_contains,
      x: instruction.x,
      y: instruction.y,
    });
  } else {
    editInstruction(props.strandId, props.path, {
      id: instruction.id,
      type: instruction.type,
      scope,
      app: instruction.app,
      title_contains: instruction.title_contains,
    });
  }
}

async function onPick() {
  const picked = await pickElement();
  if (!picked) return;
  const instruction = props.instruction;
  const base = { id: instruction.id, scope: 'Specific Window' as const, app: textValue(picked.app), title_contains: textValue(picked.window_title) };
  if (instruction.type === 'MoveWindow') {
    editInstruction(props.strandId, props.path, { ...base, type: 'MoveWindow', x: instruction.x, y: instruction.y });
  } else {
    editInstruction(props.strandId, props.path, { ...base, type: instruction.type });
  }
}
</script>

<template>
  <span class="instruction-label">{{ VERBS[props.instruction.type] }} window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <button type="button" class="btn-chip" title="Pick a window from the screen" @click="onPick"><Pipette :size="14" /></button>
  <template v-if="props.instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <ValueBlock :location="fieldLocation(strandId, path, `${props.instruction.type}App`)" :value="props.instruction.app" />
  </template>
  <template v-if="props.instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <ValueBlock :location="fieldLocation(strandId, path, `${props.instruction.type}Title`)" :value="props.instruction.title_contains" />
  </template>
  <template v-if="props.instruction.type === 'MoveWindow'">
    <span class="instruction-label">to x</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'MoveWindowX')" :value="props.instruction.x" />
    <span class="instruction-label">y</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'MoveWindowY')" :value="props.instruction.y" />
  </template>
</template>
