<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { Check, GripVertical, Pencil, Plus, X } from 'lucide-vue-next';
import { state } from '../store';
import { renameList, setListItems } from '../tauri';
import { listEditors, renameListEditor, saveListEditorPosition, setListEditorOpen } from '../listEditors';
import type { ListItemDto } from '../types';

const props = defineProps<{ name: string }>();
const list = computed(() => state.current_macro?.lists?.find(candidate => candidate.name === props.name) ?? null);
const position = computed(() => listEditors[props.name] ?? { x: 36, y: 36 });
const editingName = ref(false);
const nameDraft = ref('');
const error = ref<string | null>(null);
const pendingFocusIndex = ref<number | null>(null);
const itemInputs = new Map<number, HTMLInputElement>();
let drag: { pointerId: number; clientX: number; clientY: number; x: number; y: number } | null = null;

function captureItemInput(index: number, element: unknown) {
  if (element instanceof HTMLInputElement) itemInputs.set(index, element);
  else itemInputs.delete(index);
}

watch(
  () => list.value?.items.length,
  async length => {
    const index = pendingFocusIndex.value;
    if (index === null || length === undefined || length <= index) return;
    await nextTick();
    const input = itemInputs.get(index);
    input?.focus();
    input?.select();
    pendingFocusIndex.value = null;
  },
);

async function save(items: ListItemDto[]) {
  if (list.value) await setListItems(list.value.name, items);
}

function editItem(index: number, text: string) {
  const current = list.value;
  if (!current) return;
  const items = [...current.items];
  const number = Number(text);
  items[index] = text.trim() !== '' && Number.isFinite(number) ? { kind: 'Number', value: number } : { kind: 'Text', value: text };
  save(items);
}

async function addItem() {
  const current = list.value;
  if (!current) return;
  pendingFocusIndex.value = current.items.length;
  await save([...current.items, { kind: 'Text', value: '' }]);
}

async function addItemBelow(index: number, text: string) {
  const current = list.value;
  if (!current) return;
  const items = [...current.items];
  const number = Number(text);
  items[index] = text.trim() !== '' && Number.isFinite(number) ? { kind: 'Number', value: number } : { kind: 'Text', value: text };
  const nextIndex = index + 1;
  items.splice(nextIndex, 0, { kind: 'Text', value: '' });
  pendingFocusIndex.value = nextIndex;
  await save(items);
}

function removeItem(index: number) {
  if (list.value) save(list.value.items.filter((_, itemIndex) => itemIndex !== index));
}

async function rename() {
  const current = list.value;
  if (!current) return;
  try {
    const nextName = nameDraft.value.trim();
    await renameList(current.name, nextName);
    renameListEditor(current.name, nextName);
    editingName.value = false;
    error.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

function startDrag(event: PointerEvent) {
  if ((event.target as Element).closest('button, input')) return;
  const editor = listEditors[props.name];
  if (!editor) return;
  event.preventDefault();
  event.stopPropagation();
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  drag = { pointerId: event.pointerId, clientX: event.clientX, clientY: event.clientY, x: editor.x, y: editor.y };
}

function moveDrag(event: PointerEvent) {
  if (!drag || drag.pointerId !== event.pointerId) return;
  const editor = listEditors[props.name];
  if (!editor) return;
  editor.x = Math.max(0, drag.x + event.clientX - drag.clientX);
  editor.y = Math.max(0, drag.y + event.clientY - drag.clientY);
}

function endDrag(event: PointerEvent) {
  if (drag?.pointerId === event.pointerId) {
    drag = null;
    saveListEditorPosition(props.name);
  }
}
</script>

<template>
  <aside
    v-if="list"
    class="list-canvas-editor"
    :style="{ left: `${position.x}px`, top: `${position.y}px` }"
  >
    <header class="list-canvas-editor-header" @pointerdown="startDrag" @pointermove="moveDrag" @pointerup="endDrag" @pointercancel="endDrag">
      <GripVertical :size="16" class="list-grip" />
      <template v-if="editingName">
        <input v-model="nameDraft" @keydown.enter.prevent="rename" @keydown.esc="editingName = false">
        <button type="button" title="Save name" @click="rename"><Check :size="15" /></button>
      </template>
      <template v-else>
        <strong>{{ list.name }}</strong>
        <button type="button" title="Rename list" @click="nameDraft = list.name; editingName = true"><Pencil :size="14" /></button>
      </template>
      <button type="button" title="Hide editor" @click="setListEditorOpen(name, false)"><X :size="15" /></button>
    </header>
    <div class="list-canvas-editor-items">
      <div v-if="!list.items.length" class="list-canvas-editor-empty">This list is empty.</div>
      <div v-for="(item, index) in list.items" :key="index" class="list-canvas-editor-row">
        <span>{{ index + 1 }}</span>
        <input
          :ref="element => captureItemInput(index, element)"
          :value="String(item.value)"
          :class="item.kind === 'Number' ? 'is-number' : 'is-text'"
          @change="editItem(index, ($event.target as HTMLInputElement).value)"
          @keydown.enter.prevent="addItemBelow(index, ($event.target as HTMLInputElement).value)"
        >
        <button type="button" title="Remove item" @click="removeItem(index)"><X :size="15" /></button>
      </div>
    </div>
    <button type="button" class="list-canvas-add" @click="addItem"><Plus :size="15" /> Add item</button>
    <footer>{{ list.items.length }} {{ list.items.length === 1 ? 'item' : 'items' }}</footer>
    <p v-if="error" class="list-canvas-error">{{ error }}</p>
  </aside>
</template>

<style scoped>
.list-canvas-editor{position:absolute;z-index:12;width:300px;max-height:400px;display:flex;flex-direction:column;border:1px solid var(--blockstitch-border);border-radius:8px;background:var(--blockstitch-bg2);color:var(--blockstitch-text);box-shadow:var(--blockstitch-shadow-lg);overflow:hidden;pointer-events:auto}.list-canvas-editor-header{min-height:36px;display:flex;align-items:center;gap:4px;padding:5px 6px;border-bottom:1px solid var(--blockstitch-glass-border);background:var(--blockstitch-glass-bg-strong);cursor:grab;touch-action:none}.list-canvas-editor-header:active{cursor:grabbing}.list-grip{color:var(--blockstitch-text-dim);flex-shrink:0}.list-canvas-editor-header strong{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:13px}.list-canvas-editor-header input{min-width:0;flex:1;border:1px solid var(--blockstitch-border);border-radius:4px;background:var(--blockstitch-bg2);color:var(--blockstitch-text);padding:4px 5px;font:inherit;font-size:12px}.list-canvas-editor button{min-height:unset;padding:4px;border:0;background:transparent;color:var(--blockstitch-text-dim);border-radius:4px}.list-canvas-editor button:hover{background:var(--blockstitch-hover-overlay);color:var(--blockstitch-text)}.list-canvas-editor button.danger:hover,.list-canvas-editor-row button:hover{color:var(--blockstitch-red);background:rgba(var(--blockstitch-red-rgb),.12)}.list-canvas-editor-items{max-height:250px;overflow:auto;padding:6px}.list-canvas-editor-row{display:grid;grid-template-columns:22px 1fr 24px;align-items:center;gap:4px;padding:2px}.list-canvas-editor-row>span{text-align:right;color:var(--blockstitch-text-dim);font-size:11px;font-weight:700}.list-canvas-editor-row input{min-width:0;border:1px solid var(--blockstitch-border);border-radius:5px;background:var(--blockstitch-bg);color:var(--blockstitch-text);padding:5px 6px;font:inherit;font-size:12px}.list-canvas-editor-row input.is-number{color:var(--blockstitch-accent-light)}.list-canvas-editor-empty{padding:8px 4px;color:var(--blockstitch-text-dim);font-size:12px}.list-canvas-add{margin:0 6px 6px;border:1px dashed var(--blockstitch-border)!important;width:calc(100% - 12px);display:flex!important;justify-content:center;gap:4px;color:var(--blockstitch-text-dim)!important;font-size:12px;font-weight:700}.list-canvas-add:hover{color:var(--blockstitch-accent-light)!important;border-color:var(--blockstitch-accent)!important}.list-canvas-editor footer{border-top:1px solid var(--blockstitch-glass-border);padding:6px 8px;color:var(--blockstitch-text-dim);font-size:11px;font-weight:700}.list-canvas-editor footer span{float:right;font-weight:400}.list-canvas-error{padding:0 8px 6px;color:var(--blockstitch-red);font-size:11px}
.list-canvas-editor-items{scrollbar-width:thin;scrollbar-color:var(--blockstitch-scrollbar-thumb) transparent}.list-canvas-editor-items::-webkit-scrollbar{width:10px}.list-canvas-editor-items::-webkit-scrollbar-track{background:transparent}.list-canvas-editor-items::-webkit-scrollbar-thumb{background:var(--blockstitch-scrollbar-thumb);border:2px solid transparent;border-radius:8px;background-clip:padding-box}.list-canvas-editor-items::-webkit-scrollbar-thumb:hover{background:var(--blockstitch-scrollbar-thumb-hover);background-clip:padding-box}.list-canvas-editor-items::-webkit-scrollbar-button{display:none;width:0;height:0}
</style>
