<script setup lang="ts">
// "Macro Settings" popup, opened from the gear button next to the "Select
// macro" dropdown (MacroSelector.vue). Edits settings for the currently
// selected macro only - each field here is persisted on the macro itself
// (MacroDto.settings) and travels with it on export/import, unlike the
// app-wide preferences in SettingsPage.vue. Teleported to <body> like the
// other dialogs (MakeVariableDialog.vue, RemoveMacroDialog.vue, etc.).
import { state } from '../store';
import { setMacroAlwaysListen } from '../tauri';
import { SwitchControl } from 'blockstitch';

const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="emit('close')">
      <div class="modal-panel macro-settings-panel">
        <h2 class="modal-title">Macro Settings</h2>
        <div class="settings-row">
          <SwitchControl
            :model-value="state.current_macro?.settings.always_listen ?? false"
            @update:model-value="setMacroAlwaysListen"
          >
            Always listen for events, even when a different macro is selected
          </SwitchControl>
        </div>
        <div class="modal-actions">
          <button type="button" class="btn-primary" @click="emit('close')">Done</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
