<script setup lang="ts">
// Variable name popup - a name textbox plus Cancel/OK, teleported to <body>
// so it sits above the sidebar/canvas regardless of where the triggering
// button lives. Doubles as both "Make a Variable" (no `renameTarget`) and
// "Rename variable" (`renameTarget` set, from the per-variable context
// menu - see variableDialogs.ts, which coordinates both triggers since they
// live in different components). Duplicate/empty names are rejected by the
// backend (`create_variable_in`/`rename_variable_in` - see commands.rs); the
// error is shown inline and the dialog stays open so the user can fix it.
import { computed, nextTick, ref, watch } from 'vue';
import { createVariable, renameVariable } from '../tauri';

const props = defineProps<{ renameTarget?: string | null }>();
const emit = defineEmits<{ close: [] }>();

const isRename = computed(() => !!props.renameTarget);
const name = ref(props.renameTarget ?? '');
const error = ref<string | null>(null);
const submitting = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);

watch(inputEl, async el => {
  if (el) {
    await nextTick();
    el.focus();
    el.select();
  }
});

async function onOk() {
  if (submitting.value) return;
  submitting.value = true;
  try {
    if (props.renameTarget) {
      await renameVariable(props.renameTarget, name.value);
    } else {
      await createVariable(name.value);
    }
    emit('close');
  } catch (e) {
    error.value = String(e);
  } finally {
    submitting.value = false;
  }
}

function onCancel() {
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="onCancel">
      <div class="modal-panel">
        <h2 class="modal-title">{{ isRename ? 'Rename Variable' : 'Make a Variable' }}</h2>
        <input
          ref="inputEl"
          type="text"
          class="modal-input"
          :class="{ invalid: error }"
          placeholder="Variable name"
          v-model="name"
          @keydown.enter="onOk"
          @keydown.esc="onCancel"
        />
        <span v-if="error" class="invalid-hint">{{ error }}</span>
        <div class="modal-actions">
          <button type="button" @click="onCancel">Cancel</button>
          <button type="button" class="btn-primary" :disabled="submitting" @click="onOk">OK</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
