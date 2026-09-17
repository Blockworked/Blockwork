<script setup lang="ts">
// Read-only popup shown by a block's right-click "Details" item - explains
// what the block does and shows its wire identifier. Mounted once alongside
// ContextMenu.vue; content comes from blockDetails.ts via detailsDialog.ts.
import { Info } from 'lucide-vue-next';
import { detailsDialog, closeDetailsDialog } from '../detailsDialog';
</script>

<template>
  <Teleport to="body">
    <div v-if="detailsDialog.open && detailsDialog.details" class="modal-overlay" @pointerdown.self="closeDetailsDialog">
      <div class="modal-panel details-panel">
        <h2 class="modal-title"><Info />{{ detailsDialog.details.name }}</h2>
        <code class="details-identifier">{{ detailsDialog.details.identifier }}</code>
        <p class="details-explainer">{{ detailsDialog.details.explainer }}</p>
        <div class="modal-actions">
          <button type="button" @click="closeDetailsDialog">Close</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
