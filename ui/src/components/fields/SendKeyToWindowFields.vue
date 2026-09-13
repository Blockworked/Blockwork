<script setup lang="ts">
// Same scope-toggle-and-pick block as WindowActionFields.vue/
// ClickElementFields.vue, plus a live key-capture button (same widget as
// KeyFields.vue) and four modifier checkboxes. `modifiers` is a plain
// Ctrl(1)/Shift(2)/Alt(4)/Meta(8) bitmask — mirrors
// blockwork_core::hotkey_types::MOD_* — not a `Value`, since which
// modifiers are held is a fixed choice, not something a macro variable
// should drive (same reasoning as `scope`/`direction` elsewhere).
import { computed } from 'vue';
import { editInstruction, startKeyCapture } from '../../tauri';
import { state } from '../../store';
import { ValueBlock, AppDropdown } from 'blockstitch';
import { Pipette } from 'lucide-vue-next';
import { fieldLocation, pathsEqual, textValue } from '../../types';
import { pickElement } from '../../elementPicker';
import type { ElementScope, InstrPath, InstructionDto } from '../../types';

const MOD_CTRL = 1;
const MOD_SHIFT = 2;
const MOD_ALT = 4;
const MOD_META = 8;

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'SendKeyToWindow' }> }>();

const isCapturing = computed(() =>
  state.key_capture?.kind === 'Strand' && state.key_capture.strand_id === props.strandId && pathsEqual(state.key_capture.index, props.path),
);

function onScopeChange(v: string) {
  editInstruction(props.strandId, props.path, { ...props.instruction, scope: v as ElementScope });
}

async function onPick() {
  const picked = await pickElement();
  if (!picked) return;
  editInstruction(props.strandId, props.path, { ...props.instruction, scope: 'Specific Window', app: textValue(picked.app), title_contains: textValue(picked.window_title) });
}

function toggleModifier(bit: number, checked: boolean) {
  const modifiers = checked ? props.instruction.modifiers | bit : props.instruction.modifiers & ~bit;
  editInstruction(props.strandId, props.path, { ...props.instruction, modifiers });
}
</script>

<template>
  <span class="instruction-label">send key to window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <button type="button" class="btn-chip" title="Pick a window from the screen" @click="onPick"><Pipette :size="14" /></button>
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'SendKeyToWindowApp')" :value="instruction.app" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'SendKeyToWindowTitle')" :value="instruction.title_contains" />
  </template>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_CTRL) !== 0" @change="toggleModifier(MOD_CTRL, ($event.target as HTMLInputElement).checked)" />Ctrl</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_SHIFT) !== 0" @change="toggleModifier(MOD_SHIFT, ($event.target as HTMLInputElement).checked)" />Shift</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_ALT) !== 0" @change="toggleModifier(MOD_ALT, ($event.target as HTMLInputElement).checked)" />Alt</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_META) !== 0" @change="toggleModifier(MOD_META, ($event.target as HTMLInputElement).checked)" />Meta</label>
  <button
    class="btn-chip key-capture-btn"
    :class="{ capturing: isCapturing }"
    @click="startKeyCapture(strandId, path)"
  >{{ isCapturing ? 'Press any key…' : instruction.key }}</button>
</template>

<style scoped>
.modifier-toggle {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: inherit;
  white-space: nowrap;
}
.modifier-toggle input {
  margin: 0;
}
</style>
