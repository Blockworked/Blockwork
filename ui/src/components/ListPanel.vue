<script setup lang="ts">
import { computed } from 'vue';
import { Check } from 'lucide-vue-next';
import { state } from '../store';
import { isListEditorOpen, setListEditorOpen } from '../listEditors';
import { openListMenu } from '../contextMenu';

// Presentation order only — list data itself stays in its persisted order so
// sorting the sidebar never changes a macro or an open editor.
const lists = computed(() => [...(state.current_macro?.lists ?? [])]
  .sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: 'base' })));

function toggle(name: string) {
  setListEditorOpen(name, !isListEditorOpen(name));
}

function onContextMenu(event: MouseEvent, name: string) {
  openListMenu(event, name);
}
</script>

<template>
  <div class="list-launcher">
    <p v-if="!lists.length" class="list-launcher-empty">Make a list, then check it to open its editor on the canvas.</p>
    <div v-for="list in lists" :key="list.name" class="list-launcher-row" @contextmenu="onContextMenu($event, list.name)">
      <button
        type="button"
        class="list-visibility-toggle"
        :class="{ active: isListEditorOpen(list.name) }"
        role="checkbox"
        :aria-checked="isListEditorOpen(list.name)"
        :aria-label="`Show ${list.name} on canvas`"
        @click="toggle(list.name)"
      >
        <Check v-if="isListEditorOpen(list.name)" :size="12" :stroke-width="3" />
      </button>
      <span class="list-launcher-name">{{ list.name }}</span>
      <span class="list-launcher-count">{{ list.items.length }}</span>
    </div>
  </div>
</template>

<style scoped>
.list-launcher{padding:0 var(--blockstitch-spacing-sm) var(--blockstitch-spacing-sm);display:flex;flex-direction:column;gap:3px}.list-launcher-row{min-height:27px;display:flex;align-items:center;gap:7px;padding:4px 6px;border-radius:var(--blockstitch-radius);color:var(--blockstitch-text)}.list-launcher-row:hover{background:var(--blockstitch-hover-overlay)}.list-visibility-toggle{width:18px;min-width:18px;height:18px;min-height:18px;padding:0;display:grid;place-items:center;border:1px solid var(--blockstitch-border);border-radius:4px;background:var(--blockstitch-bg);color:var(--blockstitch-text);box-shadow:var(--blockstitch-bevel-top);cursor:pointer}.list-visibility-toggle:hover{border-color:var(--blockstitch-accent);background:var(--blockstitch-hover-overlay)}.list-visibility-toggle.active{background:var(--blockstitch-accent);border-color:var(--blockstitch-accent);color:#fff;box-shadow:inset 0 1px 0 rgba(255,255,255,.2)}.list-visibility-toggle:focus-visible{outline:2px solid var(--blockstitch-accent);outline-offset:2px}.list-launcher-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12px}.list-launcher-count{margin-left:auto;color:var(--blockstitch-text-dim);font-size:11px}.list-launcher-empty{padding:4px 0;color:var(--blockstitch-text-dim);font-size:12px;line-height:1.35}
</style>
