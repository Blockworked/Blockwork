<script setup lang="ts">
// ImportReviewDialog.vue - review popup shown before finishing an import
// that needs confirmation -
// either the macro contains a Command instruction (runs arbitrary shell
// commands, so a malicious macro maker could use one to do real damage)
// and/or it requests non-default Macro Settings (see MacroSettingsDialog.vue)
// that should be confirmed rather than silently applied. Teleported to
// <body> like MakeVariableDialog.vue. The backend has already staged the
// parsed macro in `pending_import` (see commands.rs's import_macro); this
// dialog just decides whether confirmImportMacro or cancelImportMacro
// resolves it, and which of the offered custom settings to keep.
import { TriangleAlert } from 'lucide-vue-next';
import { reactive, ref } from 'vue';
import { cancelImportMacro, confirmImportMacro } from '../tauri';
import type { ImportPromptDto } from '../types';
import { SwitchControl } from 'blockstitch';

const props = defineProps<{ prompt: ImportPromptDto }>();
const emit = defineEmits<{ close: [] }>();

const submitting = ref(false);

// Defaults to keeping every offered custom setting's imported value -
// unchecking one in the popup is what resets it back to the default instead.
const keepSettings = reactive<Record<string, boolean>>(
  Object.fromEntries(props.prompt.custom_settings.map(s => [s.key, true])),
);

async function onImportAnyway() {
  if (submitting.value) return;
  submitting.value = true;
  try {
    await confirmImportMacro({ ...keepSettings });
  } finally {
    submitting.value = false;
    emit('close');
  }
}

async function onCancel() {
  if (submitting.value) return;
  await cancelImportMacro();
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="onCancel">
      <div class="modal-panel warning-panel">
        <h2 class="modal-title"><TriangleAlert class="warning-icon" />Review Imported Macro</h2>
        <p v-if="prompt.needs_command_warning">
          This macro runs a <strong>Command</strong> instruction, which can execute arbitrary
          programs on your computer. Only import macros from people and sources you trust.
        </p>
        <template v-if="prompt.custom_settings.length">
          <p>This macro asks for the following non-default settings:</p>
          <div class="settings-row" v-for="setting in prompt.custom_settings" :key="setting.key">
            <SwitchControl v-model="keepSettings[setting.key]">{{ setting.label }}</SwitchControl>
          </div>
        </template>
        <div class="modal-actions">
          <button type="button" @click="onCancel">Cancel</button>
          <button type="button" class="btn-primary" :disabled="submitting" @click="onImportAnyway">
            {{ prompt.needs_command_warning ? 'Import Anyway' : 'Import' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
