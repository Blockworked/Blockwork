<script setup lang="ts">
import { ref } from 'vue';
import { Trash2 } from 'lucide-vue-next';
import { deleteList, deleteVariable } from '../tauri';
import { closeDeleteUsageDialog, deleteUsageDialog } from '../deleteUsageDialog';
import { forgetListEditor } from '../listEditors';

const submitting = ref(false);

async function onDelete() {
  if (submitting.value) return;
  submitting.value = true;
  try {
    if (deleteUsageDialog.kind === 'variable') await deleteVariable(deleteUsageDialog.name);
    else {
      await deleteList(deleteUsageDialog.name);
      forgetListEditor(deleteUsageDialog.name);
    }
    closeDeleteUsageDialog();
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="deleteUsageDialog.open" class="modal-overlay" @pointerdown.self="closeDeleteUsageDialog">
      <div class="modal-panel warning-panel">
        <h2 class="modal-title"><Trash2 class="danger-icon" />Delete {{ deleteUsageDialog.kind }}</h2>
        <p>
          <strong>{{ deleteUsageDialog.name }}</strong> is used by blocks on the canvas. Deleting it will leave those references unresolved.
        </p>
        <div class="modal-actions">
          <button type="button" @click="closeDeleteUsageDialog">Cancel</button>
          <button type="button" class="btn-danger" :disabled="submitting" @click="onDelete">Delete anyway</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
