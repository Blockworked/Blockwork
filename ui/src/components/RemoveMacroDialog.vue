<script setup lang="ts">
// Delete-macro confirmation popup - replaces the old "click again within a
// countdown" pattern on the toolbar's Delete button with a plain modal,
// mirroring ImportCommandWarningDialog.vue's shape. Teleported to <body>
// like the other dialogs (MakeVariableDialog.vue, etc.).
import { Trash2 } from 'lucide-vue-next';
import { ref } from 'vue';
import { state } from '../store';
import { removeMacro } from '../tauri';

const emit = defineEmits<{ close: [] }>();

const submitting = ref(false);

async function onDelete() {
  if (submitting.value) return;
  submitting.value = true;
  try {
    await removeMacro();
  } finally {
    submitting.value = false;
    emit('close');
  }
}

function onCancel() {
  emit('close');
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="onCancel">
      <div class="modal-panel warning-panel">
        <h2 class="modal-title"><Trash2 class="danger-icon" />Delete Macro</h2>
        <p>
          Delete <strong>{{ state.current_macro?.name }}</strong>? This can't be undone.
        </p>
        <div class="modal-actions">
          <button type="button" @click="onCancel">Cancel</button>
          <button type="button" class="btn-danger" :disabled="submitting" @click="onDelete">Delete</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
