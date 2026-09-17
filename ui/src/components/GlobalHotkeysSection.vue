<script setup lang="ts">
import { Keyboard } from 'lucide-vue-next';
import { state } from '../store';
import { clearNamedHotkey, resetHotkeyToDefault, startComboCapture } from '../tauri';
import { NAMED_ACTIONS, NO_COMBO_ACTIONS, type NamedActionType } from '../constants';
import { AppButton } from 'blockstitch';

function bindingFor(type: NamedActionType) {
  return state.hotkey_bindings.find(b => b.action.type === type);
}
function isCapturing(type: NamedActionType) {
  return state.combo_capture?.kind === 'Named' && state.combo_capture.action?.type === type;
}
function defaultDisplayFor(type: NamedActionType) {
  return state.named_hotkey_defaults.find(d => d.action.type === type)?.combo_display ?? null;
}
function isAtDefault(type: NamedActionType) {
  const def = defaultDisplayFor(type);
  return def == null || bindingFor(type)?.combo_display === def;
}
</script>

<template>
  <div class="settings-section">
    <div class="settings-section-title"><Keyboard /><span>Global Hotkeys</span></div>
    <div>
      <div v-for="{ label, type } in NAMED_ACTIONS" :key="type" class="settings-row">
        <span class="settings-row-label">
          {{ label }}
          <span v-if="NO_COMBO_ACTIONS.has(type)" class="settings-row-hint" title="Single key only, no modifiers - modifier keys held while this fires would be captured as macro steps">(single key)</span>
        </span>
        <button
          class="btn-chip"
          :class="{ capturing: isCapturing(type) }"
          :disabled="false"
          @click="isCapturing(type) ? undefined : startComboCapture({ type })"
        >
          {{ isCapturing(type) ? (NO_COMBO_ACTIONS.has(type) ? 'Press a key…' : 'Press combo…') : (bindingFor(type)?.combo_display ?? 'Not set') }}
        </button>
        <button
          v-show="!isCapturing(type) && !isAtDefault(type)"
          @click="resetHotkeyToDefault({ type })"
        >Default</button>
        <AppButton
          v-show="!isCapturing(type) && bindingFor(type)?.combo_display != null"
          class="btn-icon btn-danger"
          icon="x"
          title="Clear hotkey"
          aria-label="Clear hotkey"
          @click="clearNamedHotkey({ type })"
        />
      </div>
    </div>
  </div>
</template>
