<script setup lang="ts">
import { computed, ref } from 'vue';
import { state } from '../store';
import { newMacro, openSettings, selectMacro } from '../tauri';
import { AppDropdown } from 'blockstitch';
import { AppButton } from 'blockstitch';
import RemoveMacroDialog from './RemoveMacroDialog.vue';
import MacroSettingsDialog from './MacroSettingsDialog.vue';

const macroOptions = computed(() =>
  state.macro_names.map((name, idx) => ({ value: String(idx), label: name })),
);
const selectedValue = computed(() => (state.macro_selected != null ? String(state.macro_selected) : ''));

function onSelect(value: string) {
  if (value === '') return;
  const idx = parseInt(value, 10);
  const cached = state.macros_data[idx];
  if (cached) {
    state.macro_selected = idx;
    state.current_macro = cached;
    state.can_undo = false;
    state.can_redo = false;
    state.invalid_field_buffers = [];
    state.key_capture = null;
  }
  selectMacro(idx);
}

const showRemoveDialog = ref(false);
const showMacroSettings = ref(false);
</script>

<template>
  <div class="toolbar-row" id="selector-row">
    <div class="selector-left">
      <label id="macro-dropdown-label">Select macro</label>
      <div class="macro-dropdown-row">
        <AppDropdown
          :options="macroOptions"
          :model-value="selectedValue"
          placeholder="- no macro selected -"
          class-name="macro-select-trigger"
          aria-label="Select macro"
          aria-labelledby="macro-dropdown-label"
          @update:model-value="onSelect"
        />
        <AppButton
          icon="sliders-horizontal"
          :disabled="state.macro_selected == null"
          title="Macro Settings"
          aria-label="Macro Settings"
          @click="showMacroSettings = true"
        />
      </div>
    </div>
    <div class="selector-right">
      <AppButton icon="plus" label="New macro" title="Add a new macro" @click="newMacro()" />
      <AppButton
        id="remove-macro-btn"
        class="btn-danger"
        :disabled="state.macro_selected == null"
        title="Delete selected macro"
        icon="trash"
        label="Delete"
        @click="showRemoveDialog = true"
      />
      <AppButton icon="settings" label="Settings" title="Open Settings" @click="openSettings()" />
    </div>
    <RemoveMacroDialog v-if="showRemoveDialog" @close="showRemoveDialog = false" />
    <MacroSettingsDialog v-if="showMacroSettings" @close="showMacroSettings = false" />
  </div>
</template>
