<script setup lang="ts">
// Sidebar prefab preview — literal-only fields, same spirit as
// PaletteClickElementFields.vue, plus the standalone key-capture flow
// PaletteKeyFields.vue uses (this prefab has no real strand/path for the
// canvas's normal capture flow to write into).
import { computed, watch } from 'vue';
import { state } from '../../../store';
import { clearStandaloneKeyCapture, startStandaloneKeyCapture } from '../../../tauri';
import { AutosizeInput, AppDropdown } from 'blockstitch';
import { textValue } from '../../../types';
import type { ElementScope, InstructionDto } from '../../../types';

const MOD_CTRL = 1;
const MOD_SHIFT = 2;
const MOD_ALT = 4;
const MOD_META = 8;

const props = defineProps<{ instruction: Extract<InstructionDto, { type: 'SendKeyToWindow' }> }>();

const isCapturing = computed(() => state.key_capture?.kind === 'Standalone');

watch(() => state.standalone_key, key => {
  if (key == null) return;
  props.instruction.key = key;
  clearStandaloneKeyCapture();
});

function onScopeChange(v: string) {
  props.instruction.scope = v as ElementScope;
}
function setApp(v: string) {
  props.instruction.app = textValue(v);
}
function setTitle(v: string) {
  props.instruction.title_contains = textValue(v);
}
function toggleModifier(bit: number, checked: boolean) {
  props.instruction.modifiers = checked ? props.instruction.modifiers | bit : props.instruction.modifiers & ~bit;
}
</script>

<template>
  <span class="instruction-label">send key to window in</span>
  <AppDropdown :options="['Focused Window', 'Specific App', 'Specific Window']" :model-value="instruction.scope" class-name="dd-compact" @update:model-value="onScopeChange" />
  <template v-if="instruction.scope === 'Specific App'">
    <span class="instruction-label">app</span>
    <AutosizeInput :model-value="props.instruction.app.kind === 'Text' ? props.instruction.app.value : ''" :min-chars="4" @update:model-value="setApp" />
  </template>
  <template v-if="instruction.scope === 'Specific Window'">
    <span class="instruction-label">title contains</span>
    <AutosizeInput :model-value="props.instruction.title_contains.kind === 'Text' ? props.instruction.title_contains.value : ''" :min-chars="4" @update:model-value="setTitle" />
  </template>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_CTRL) !== 0" @change="toggleModifier(MOD_CTRL, ($event.target as HTMLInputElement).checked)" />Ctrl</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_SHIFT) !== 0" @change="toggleModifier(MOD_SHIFT, ($event.target as HTMLInputElement).checked)" />Shift</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_ALT) !== 0" @change="toggleModifier(MOD_ALT, ($event.target as HTMLInputElement).checked)" />Alt</label>
  <label class="modifier-toggle"><input type="checkbox" :checked="(instruction.modifiers & MOD_META) !== 0" @change="toggleModifier(MOD_META, ($event.target as HTMLInputElement).checked)" />Meta</label>
  <button
    class="btn-chip key-capture-btn"
    :class="{ capturing: isCapturing }"
    @click="startStandaloneKeyCapture()"
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
