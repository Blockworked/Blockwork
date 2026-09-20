<script setup lang="ts">
// "Recording Settings" popup, opened from the sliders button next to the
// Record button (RunControls.vue). Controls how mouse movement gets
// captured while recording, app-wide - unlike MacroSettingsDialog.vue's
// per-macro settings, this is a session-wide preference persisted like
// loop mode or global speed.
import { computed, ref } from 'vue';
import { TriangleAlert } from 'lucide-vue-next';
import { state } from '../store';
import { toggleRecordMouseRelative, toggleRecordMouseMovement } from '../tauri';
import { SwitchControl } from 'blockstitch';

const emit = defineEmits<{ close: [] }>();

// Whatever the backend refused the last absolute switch with (no XWayland on
// X11, no libei permission on Wayland); cleared once a toggle goes through.
const absoluteError = ref('');

async function setRelative(relative: boolean) {
  try {
    await toggleRecordMouseRelative(relative);
    absoluteError.value = '';
  } catch (e) {
    absoluteError.value = String(e);
  }
}

// Wayland has no cursor-position API, so absolute recording tracks the cursor
// and steers it through libei rather than reading it back.
const absoluteNote = computed(() => {
  if (absoluteError.value) return absoluteError.value;
  if (!state.record_mouse_movement || state.record_mouse_relative) return '';
  if (!state.absolute_mouse_position_available) {
    return 'Absolute mouse recording isn’t available in this session.';
  }
  return state.wayland_session
    ? 'Recording absolute positions on Wayland is considered experimental.'
    : '';
});
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
        <div
          class="settings-row"
          :class="{
            'row-disabled': !state.record_mouse_movement,
          }"
        >
          <SwitchControl
            :model-value="state.record_mouse_relative"
            @update:model-value="setRelative"
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
        <div v-if="absoluteNote" class="warning-banner settings-row-note">
          <TriangleAlert />
          <span>{{ absoluteNote }}</span>
        </div>
        <div class="modal-actions">
          <button type="button" class="btn-primary" @click="emit('close')">Done</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.recording-settings-panel {
  box-sizing: border-box;
  width: 440px;
  max-width: calc(100vw - 32px);
}

.row-disabled {
  opacity: 0.5;
  pointer-events: none;
}

.settings-row-note {
  margin-top: 8px;
  font-size: 12px;
  /* Readable until the blockstitch pin picks up --blockstitch-yellow-text,
     whose absence leaves the banner's yellow-on-yellow unreadable in light. */
  color: var(--blockstitch-yellow-text, var(--blockstitch-text));
}

</style>
