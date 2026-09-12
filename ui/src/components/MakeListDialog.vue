<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { createList, renameList } from '../tauri';
import { renameListEditor } from '../listEditors';

const props = defineProps<{ renameTarget?: string | null }>();
const emit = defineEmits<{ close: [] }>();
const isRename = computed(() => !!props.renameTarget);
const name = ref(props.renameTarget ?? '');
const error = ref<string | null>(null);
const submitting = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);

watch(inputEl, async el => {
  if (!el) return;
  await nextTick();
  el.focus();
});

async function onOk() {
  if (submitting.value) return;
  submitting.value = true;
  try {
    if (props.renameTarget) {
      await renameList(props.renameTarget, name.value);
      renameListEditor(props.renameTarget, name.value);
    } else {
      await createList(name.value);
    }
    emit('close');
  } catch (e) {
    error.value = String(e);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="emit('close')">
      <div class="modal-panel">
        <h2 class="modal-title">{{ isRename ? 'Rename List' : 'Make a List' }}</h2>
        <input
          ref="inputEl"
          v-model="name"
          type="text"
          class="modal-input"
          :class="{ invalid: error }"
          placeholder="List name"
          @keydown.enter="onOk"
          @keydown.esc="emit('close')"
        >
        <span v-if="error" class="invalid-hint">{{ error }}</span>
        <div class="modal-actions">
          <button type="button" @click="emit('close')">Cancel</button>
          <button type="button" class="btn-primary" :disabled="submitting" @click="onOk">OK</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
