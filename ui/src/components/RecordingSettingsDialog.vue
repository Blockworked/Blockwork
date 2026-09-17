<script setup lang="ts">
// "Recording Settings" popup, opened from the sliders button next to the
// Record button (RunControls.vue). Controls how mouse movement gets
// captured while recording, app-wide - unlike MacroSettingsDialog.vue's
// per-macro settings, this is a session-wide preference persisted like
// loop mode or global speed.
import { state } from '../store';
import { toggleRecordMouseRelative, toggleRecordMouseMovement } from '../tauri';
import { SwitchControl } from 'blockstitch';

const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="emit('close')">
      <div class="modal-panel recording-settings-panel">
        <h2 class="modal-title">Recording Settings</h2>
        <div class="settings-row">
          <SwitchControl
            :model-value="state.record_mouse_movement"
            @update:model-value="toggleRecordMouseMovement"
          >
            Record mouse movement
          </SwitchControl>
        </div>
        <div class="settings-row" :class="{ 'row-disabled': !state.record_mouse_movement }">
          <SwitchControl
            :model-value="state.record_mouse_relative"
            @update:model-value="toggleRecordMouseRelative"
          >
            Record mouse movement as relative motion
          </SwitchControl>
        </div>
        <p class="settings-row-hint">
          {{ !state.record_mouse_movement
            ? 'Mouse movement isn’t recorded; only clicks, scrolls, and keys are.'
            : state.record_mouse_relative
              ? 'Movement is recorded as deltas from the cursor’s previous position.'
              : 'Movement is recorded as absolute positions on screen.' }}
        </p>
        <div class="modal-actions">
          <button type="button" class="btn-primary" @click="emit('close')">Done</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.row-disabled {
  opacity: 0.5;
  pointer-events: none;
}
</style>
