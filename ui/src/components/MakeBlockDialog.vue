<script setup lang="ts">
// "Make a Block"/"Edit Block" popup - a centered live prototype preview
// (labels + input ovals) the user builds up by inserting pieces, plus a
// return-type choice, teleported to <body> like MakeVariableDialog.vue.
// Doubles as both "Make a Block" (no `editTarget`) and "Edit Block" (from a
// prefab's context menu, see ContextMenu.vue/blockDialogs.ts) - editing
// works on a local copy of `pieces`/`shape` and only writes back via
// createBlock/editBlock on OK, so Cancel is a true no-op.
import type { ComponentPublicInstance } from 'vue';
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { Blocks, ChevronLeft, ChevronRight, Pipette, X } from 'lucide-vue-next';
import { createBlock, editBlock } from '../tauri';
import BranchHeaderPiece from './fields/BranchHeaderPiece.vue';
import { blockShapeReturnsValue } from '../types';
import type { BlockDefDto, BlockPieceDto, BlockShapeDto, InputValueType } from '../types';

const props = defineProps<{ editTarget: BlockDefDto | null }>();
const emit = defineEmits<{ close: [] }>();

const isEdit = computed(() => !!props.editTarget);

// Stable per-piece id - preserved verbatim for every existing piece (so
// `edit_block` can tell a rename apart from a remove+add, see types.ts's
// BlockPieceDto comment), freshly generated only for a piece created in
// this session (`addPiece`).
function newPieceId(): string {
  return crypto.randomUUID?.() ?? `p${Math.random().toString(36).slice(2)}`;
}

const pieces = reactive<BlockPieceDto[]>(
  props.editTarget ? props.editTarget.pieces.map(p => ({ ...p })) : [{ kind: 'Label', id: newPieceId(), text: 'block name' }],
);
const shape = ref<BlockShapeDto>(props.editTarget?.shape ?? 'Normal');
const color = ref(props.editTarget?.color ?? '#4C97FF');
// The final swatch opens a native color picker for colors outside this palette.
const COLOR_PRESETS = [
  '#4C97FF', '#9966FF', '#C65BCF', '#FFBF00', '#FFAB19', '#5BA9D0',
  '#59C059', '#FF8C1A', '#FF5B1F', '#FF6680', '#19B88E', '#FF4D4F',
  '#FF7F7F', '#FFB77B', '#FFF28A', '#8BF77A', '#78F0B0', '#70D5E8',
  '#7DB5F5', '#8080F5', '#C667E8', '#F27AED', '#B3B3B3',
] as const;
const isPresetColor = computed(() => COLOR_PRESETS.includes(color.value as typeof COLOR_PRESETS[number]));
function selectColor(next: string) {
  color.value = next;
}
const customPickerOpen = ref(false);
const customHue = ref(215);
const customSaturation = ref(100);
const customLightness = ref(65);
const customColor = computed(() => hslToHex(customHue.value, customSaturation.value, customLightness.value));

function hslToHex(hue: number, saturation: number, lightness: number): string {
  const s = saturation / 100;
  const l = lightness / 100;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs((hue / 60) % 2 - 1));
  const m = l - c / 2;
  const [r, g, b] = hue < 60 ? [c, x, 0] : hue < 120 ? [x, c, 0] : hue < 180 ? [0, c, x] : hue < 240 ? [0, x, c] : hue < 300 ? [x, 0, c] : [c, 0, x];
  return `#${[r, g, b].map(v => Math.round((v + m) * 255).toString(16).padStart(2, '0')).join('').toUpperCase()}`;
}

function hexToHsl(hex: string): [number, number, number] | null {
  const match = /^#([\dA-F]{6})$/i.exec(hex);
  if (!match) return null;
  const channels = [0, 2, 4].map(i => parseInt(match[1].slice(i, i + 2), 16) / 255);
  const max = Math.max(...channels);
  const min = Math.min(...channels);
  const delta = max - min;
  const l = (max + min) / 2;
  const s = delta === 0 ? 0 : delta / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (delta !== 0) {
    h = max === channels[0] ? 60 * (((channels[1] - channels[2]) / delta) % 6)
      : max === channels[1] ? 60 * ((channels[2] - channels[0]) / delta + 2)
        : 60 * ((channels[0] - channels[1]) / delta + 4);
  }
  return [(h + 360) % 360, s * 100, l * 100];
}

function openCustomPicker() {
  const hsl = hexToHsl(color.value);
  if (hsl) [customHue.value, customSaturation.value, customLightness.value] = hsl;
  customPickerOpen.value = !customPickerOpen.value;
}

function updateCustomColor() {
  color.value = customColor.value;
}
// The "returns a value" checkbox is a view over `shape`, not separate state
// of its own - it just picks which pair of mutually-exclusive shapes the two
// wide buttons below offer (Normal/Ending vs ReturnsValue/ReturnsBool).
const isValueMode = computed(() => blockShapeReturnsValue(shape.value));
// Toggling the checkbox always lands on the *first* option of whichever pair
// it switches to, per the dialog's spec - never tries to preserve e.g. an
// Ending block's "endingness" as a boolean-return choice, since the two
// pairs don't correspond piece-for-piece.
function onToggleReturnsValue(e: Event) {
  shape.value = (e.target as HTMLInputElement).checked ? 'ReturnsValue' : 'Normal';
}
function selectPrimaryShape() {
  shape.value = isValueMode.value ? 'ReturnsValue' : 'Normal';
}
function selectSecondaryShape() {
  shape.value = isValueMode.value ? 'ReturnsBool' : 'Ending';
}
const error = ref<string | null>(null);
const submitting = ref(false);

const editingIndex = ref<number | null>(null);
const editingText = ref('');
const editInputEl = ref<HTMLInputElement | null>(null);
// The piece the remove/move-left/move-right toolbar floats above. Separate
// from `editingIndex` (which only tracks the live rename text field) so the
// toolbar stays put once a rename commits instead of disappearing.
const selectedIndex = ref<number | null>(null);

// The toolbar floats above whichever piece is selected, but the preview now
// reuses the real .instruction-row/.instruction-shape markup (see template)
// so it looks exactly like the block that actually spawns on the canvas --
// and .instruction-shape clip-paths its own content to cut the puzzle-piece
// notch, which would silently clip away a CSS-only `position: absolute`
// toolbar nested inside it. Instead the toolbar is a sibling of the clipped
// shape, positioned in JS from the selected piece's actual measured
// position, and re-measured on every resize (the preview's pieces grow/
// shrink live while typing, see editingText below) via ResizeObserver.
const previewAnchorEl = ref<HTMLElement | null>(null);
// The whole block silhouette (.instruction-shape or .value-block) -- the
// toolbar hovers above the entire block, not just the selected piece, so it
// clears the shape's own top edge (and the drop-shadow .instruction-row
// grows on hover) instead of floating at whatever height the piece itself
// happens to sit at within the row.
const previewShapeEl = ref<HTMLElement | null>(null);
const pieceEls: (HTMLElement | null)[] = [];
function setPieceEl(i: number, el: Element | ComponentPublicInstance | null) {
  pieceEls[i] = el as HTMLElement | null;
}
const toolbarPos = reactive({ left: 0, top: 0 });
function updateToolbarPos() {
  const i = selectedIndex.value;
  const anchor = previewAnchorEl.value;
  const shapeEl = previewShapeEl.value;
  const pieceEl = i === null ? null : pieceEls[i];
  if (i === null || !anchor || !shapeEl || !pieceEl) return;
  const anchorRect = anchor.getBoundingClientRect();
  const shapeRect = shapeEl.getBoundingClientRect();
  const pieceRect = pieceEl.getBoundingClientRect();
  toolbarPos.left = (pieceRect.left - anchorRect.left + pieceRect.width / 2) / 0.75;
  toolbarPos.top = (shapeRect.top - anchorRect.top) / 0.75;
}
watch([selectedIndex, shape], () => nextTick(updateToolbarPos));
let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  resizeObserver = new ResizeObserver(() => updateToolbarPos());
  if (previewAnchorEl.value) resizeObserver.observe(previewAnchorEl.value);
});
onBeforeUnmount(() => resizeObserver?.disconnect());

// Left/middle-click drag-to-pan for the preview canvas -- mirrors the real
// canvas's own pan gesture (blockstitch's canvasDrag.ts: beginPan/onPointerMove/
// onPointerUp) but scoped to just this one scrollable box via pointer capture,
// since there's no strand/drag/snap machinery to hook into here.
const canvasEl = ref<HTMLElement | null>(null);
const isPanning = ref(false);
let pan: { pointerId: number; startX: number; startY: number; startScrollLeft: number; startScrollTop: number } | null = null;
// On Linux, middle click pastes X11's primary selection into whatever's
// focused -- CEF honors it regardless of hit-testing, so preventDefault() on
// pointerdown alone doesn't stop it (same issue canvasDrag.ts's beginPan
// works around). Swallow the paste that follows a middle-click pan instead,
// since this dialog's rename inputs would otherwise eat it.
let blockPrimaryPaste = false;
let blockPrimaryPasteGeneration = 0;
function onCanvasPaste(e: ClipboardEvent) {
  if (!blockPrimaryPaste) return;
  e.preventDefault();
  e.stopImmediatePropagation();
}
function onCanvasPointerDown(e: PointerEvent) {
  if (e.button !== 0 && e.button !== 1) return;
  // A left click only pans when it starts on genuinely empty canvas space --
  // one landing on the block shape itself (or its toolbar) is left alone so
  // piece selection/editing and the toolbar buttons keep working. Middle
  // click always pans, even over the block.
  const target = e.target as HTMLElement;
  if (e.button === 0 && target.closest('.instruction-row, .value-block, .make-block-piece-toolbar, input, button')) return;
  const canvas = canvasEl.value;
  if (!canvas) return;
  if (e.button === 1) {
    const active = document.activeElement;
    if (active instanceof HTMLElement && (active.tagName === 'INPUT' || active.isContentEditable)) active.blur();
    blockPrimaryPaste = true;
    blockPrimaryPasteGeneration++;
    document.addEventListener('paste', onCanvasPaste, true);
  }
  e.preventDefault();
  canvas.setPointerCapture(e.pointerId);
  pan = { pointerId: e.pointerId, startX: e.clientX, startY: e.clientY, startScrollLeft: canvas.scrollLeft, startScrollTop: canvas.scrollTop };
  isPanning.value = true;
}
function onCanvasPointerMove(e: PointerEvent) {
  if (!pan || pan.pointerId !== e.pointerId) return;
  const canvas = canvasEl.value;
  if (!canvas) return;
  canvas.scrollLeft = pan.startScrollLeft - (e.clientX - pan.startX);
  canvas.scrollTop = pan.startScrollTop - (e.clientY - pan.startY);
}
function endPan(e: PointerEvent) {
  if (!pan || pan.pointerId !== e.pointerId) return;
  pan = null;
  isPanning.value = false;
  // The primary-selection paste fires slightly after pointerup on middle-
  // button release, so clear the flag after a tick rather than synchronously;
  // guarded by a generation counter against a fresh pan starting in between.
  const generation = blockPrimaryPasteGeneration;
  setTimeout(() => {
    if (blockPrimaryPasteGeneration === generation) {
      blockPrimaryPaste = false;
      document.removeEventListener('paste', onCanvasPaste, true);
    }
  }, 200);
}
onBeforeUnmount(() => document.removeEventListener('paste', onCanvasPaste, true));

// Starts past however many inputs already exist so a freshly-inserted
// input's default name doesn't collide with an existing "valueN" (which
// would otherwise immediately trip the uniqueness check on OK).
let nextInputSeq = pieces.filter(p => p.kind === 'Input').length + 1;
let nextBranchSeq = 1;
function newBranchName(): string {
  while (pieces.some(piece => piece.kind === 'Branch' && piece.name === `branch${nextBranchSeq}`)) nextBranchSeq++;
  return `branch${nextBranchSeq++}`;
}

function pieceText(piece: BlockPieceDto): string {
  return piece.kind === 'Label' ? (piece.text || '(label)') : piece.name;
}

// A boolean input piece previews as the same hexagon shape it actually gets
// once the block is called (see .make-block-piece-bool) instead of the
// ordinary number/text capsule, so the prototype editor already reads as
// "this is a boolean" instead of looking identical to every other input.
function isBoolPiece(piece: BlockPieceDto): boolean {
  return piece.kind === 'Input' && piece.value_type === 'Bool';
}

const branchIndexes = computed(() => pieces.flatMap((piece, index) => piece.kind === 'Branch' ? [index] : []));
const hasBranches = computed(() => branchIndexes.value.length > 0);
const firstBranchIndex = computed(() => branchIndexes.value[0] ?? -1);
function separatorIndex(branchOrdinal: number): number | null {
  const branchIndex = branchIndexes.value[branchOrdinal];
  const nextBranch = branchIndexes.value[branchOrdinal + 1];
  if (branchIndex === undefined || nextBranch === undefined) return null;
  for (let i = branchIndex + 1; i < nextBranch; i++) {
    if (pieces[i].kind === 'Label') return i;
  }
  return null;
}
function separatorText(branchOrdinal: number): string {
  const index = separatorIndex(branchOrdinal);
  const piece = index === null ? null : pieces[index];
  return piece?.kind === 'Label' ? piece.text : '';
}

function updateSeparator(branchOrdinal: number, event: Event) {
  let index = separatorIndex(branchOrdinal);
  if (index === null) {
    commitEditing();
    index = branchIndexes.value[branchOrdinal] + 1;
    pieces.splice(index, 0, { kind: 'Label', id: newPieceId(), text: '' });
  }
  const piece = pieces[index];
  if (piece.kind === 'Label') piece.text = (event.target as HTMLInputElement).value;
}

function startEditing(i: number) {
  if (editingIndex.value !== null && editingIndex.value !== i) commitEditing();
  const piece = pieces[i];
  editingIndex.value = i;
  selectedIndex.value = i;
  editingText.value = piece.kind === 'Label' ? piece.text : piece.name;
  nextTick(() => {
    const input = previewAnchorEl.value?.querySelector<HTMLInputElement>('.make-block-piece-input-el');
    input?.focus();
    input?.select();
  });
}


// Fresh "Make a Block" opens with the block-name piece already selected and
// ready to type over, since it's the one field every block needs. Editing an
// existing block leaves selection alone -- its pieces are already named, so
// nothing should jump into rename mode just from opening the dialog.
onMounted(() => {
  if (!props.editTarget) startEditing(0);
  nextTick(() => {
    const canvas = canvasEl.value;
    if (!canvas) return;
    canvas.scrollLeft = (canvas.scrollWidth - canvas.clientWidth) / 2;
    canvas.scrollTop = 240;
  });
});

function commitEditing() {
  if (editingIndex.value === null) return;
  const piece = pieces[editingIndex.value];
  const text = editingText.value.trim();
  if (piece.kind === 'Label') {
    piece.text = text;
  } else if (text) {
    piece.name = text;
  }
  editingIndex.value = null;
}

function addPiece(kind: 'Label' | 'Input' | 'Branch', valueType: InputValueType = 'Any') {
  commitEditing();
  // A label between two callback mouths is distinct from either callback's
  // name. Seed it blank when a second (or later) branch is added so users can
  // click the mid-bar in the preview and type their own "else"-style text.
  if (kind === 'Branch' && hasBranches.value) {
    pieces.push({ kind: 'Label', id: newPieceId(), text: '' });
  }
  const piece: BlockPieceDto =
    kind === 'Label'
      ? { kind: 'Label', id: newPieceId(), text: 'label' }
      : kind === 'Branch'
        ? { kind: 'Branch', id: newPieceId(), name: newBranchName() }
        : { kind: 'Input', id: newPieceId(), name: `value${nextInputSeq++}`, value_type: valueType };
  const index = kind === 'Label' && hasBranches.value ? firstBranchIndex.value : pieces.length;
  pieces.splice(index, 0, piece);
  // Newly-added pieces land pre-selected and already in rename mode, matching
  // "click the name to edit it" for every other piece, so typing can start
  // immediately instead of requiring a click on the placeholder text first.
  nextTick(() => startEditing(index));
}

function removePiece(i: number) {
  pieces.splice(i, 1);
  if (editingIndex.value === i) editingIndex.value = null;
  if (selectedIndex.value === i) selectedIndex.value = null;
  else if (selectedIndex.value !== null && selectedIndex.value > i) selectedIndex.value--;
}

function movePiece(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= pieces.length) return;
  [pieces[i], pieces[j]] = [pieces[j], pieces[i]];
  selectedIndex.value = j;
}

async function onOk() {
  if (submitting.value) return;
  const flatLabel = pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Label' }> => p.kind === 'Label').map(p => p.text.trim()).join(' ').trim();
  if (!flatLabel) {
    error.value = 'Give the block a name';
    return;
  }
  const namedPieceNames = pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' | 'Branch' }> => p.kind !== 'Label').map(p => p.name.trim());
  if (namedPieceNames.some(n => !n)) {
    error.value = 'Every input and branch needs a name';
    return;
  }
  if (new Set(namedPieceNames).size !== namedPieceNames.length) {
    error.value = 'Input and branch names must be unique';
    return;
  }
  submitting.value = true;
  try {
    // Trailing/leading spaces are fine mid-edit (the live preview shows
    // exactly what's typed, see .make-block-piece-text's `white-space: pre`)
    // but shouldn't survive into the saved block -- trim here rather than
    // relying solely on commitEditing's own trim-on-blur, since a piece
    // still focused at the moment OK is pressed commits (and its blur fires)
    // as part of this same click, and this makes the guarantee explicit.
    const snapshot = pieces.map(p => (p.kind === 'Label' ? { ...p, text: p.text.trim() } : { ...p, name: p.name.trim() }));
    if (props.editTarget) {
      await editBlock(props.editTarget.id, snapshot, shape.value, color.value);
    } else {
      await createBlock(snapshot, shape.value, color.value);
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
      <div class="modal-panel make-block-panel">
        <h2 class="modal-title">{{ isEdit ? 'Edit Block' : 'Make a Block' }}</h2>

        <div
          class="make-block-canvas"
          ref="canvasEl"
          :class="{ panning: isPanning }"
          @pointerdown="onCanvasPointerDown"
          @pointermove="onCanvasPointerMove"
          @pointerup="endPan"
          @pointercancel="endPan"
        >
          <div class="make-block-preview-anchor" ref="previewAnchorEl">
            <div :class="hasBranches && isValueMode ? ['value-block', 'blockwork-custom-value-block', 'blockwork-branch-reporter', shape === 'ReturnsBool' ? 'value-card-shape-bool' : 'value-card-shape'] : undefined" :style="{ '--blockwork-custom-block-color': color }">
            <div
              v-if="selectedIndex !== null"
              class="make-block-piece-toolbar"
              :style="{ left: toolbarPos.left + 'px', top: toolbarPos.top + 'px' }"
            >
              <button type="button" class="make-block-piece-move" title="Move left" :disabled="selectedIndex === 0" @click.stop="movePiece(selectedIndex, -1)">
                <ChevronLeft />
              </button>
              <button type="button" class="make-block-piece-remove" title="Remove" @click.stop="removePiece(selectedIndex)">
                <X />
              </button>
              <button
                type="button"
                class="make-block-piece-move"
                title="Move right"
                :disabled="selectedIndex === pieces.length - 1"
                @click.stop="movePiece(selectedIndex, 1)"
              >
                <ChevronRight />
              </button>
            </div>

            <span
              v-if="isValueMode && !hasBranches"
              class="value-block"
              :class="[shape === 'ReturnsBool' ? 'value-card-shape-bool' : 'value-card-shape', 'blockwork-custom-value-block']"
              :style="{ '--blockwork-custom-block-color': color }"
              ref="previewShapeEl"
              @pointerdown.self="selectedIndex = null"
            >
              <template v-for="(piece, i) in pieces" :key="piece.id">
                <span
                  class="make-block-piece"
                  :ref="(el) => setPieceEl(i, el)"
                :class="{ 'make-block-piece-input': piece.kind === 'Input' && !isBoolPiece(piece), 'make-block-piece-bool': isBoolPiece(piece), 'make-block-piece-branch': piece.kind === 'Branch', 'make-block-piece-selected': selectedIndex === i }"
                >
                  <span class="make-block-piece-field">
                    <span
                      class="make-block-piece-text"
                      :class="{ 'make-block-piece-text-hidden': editingIndex === i }"
                      @click="startEditing(i)"
                    >{{ editingIndex === i ? editingText || ' ' : pieceText(piece) }}</span>
                    <input
                      v-if="editingIndex === i"
                      :ref="(el) => (editInputEl = el as HTMLInputElement | null)"
                      type="text"
                      class="make-block-piece-input-el"
                      v-model="editingText"
                      @blur="commitEditing"
                      @keydown.enter="commitEditing"
                      @keydown.esc="editingIndex = null"
                    />
                  </span>
                </span>
              </template>
            </span>

            <div
              v-else-if="hasBranches"
              class="instruction-row instruction-row-wrap blockwork-custom-block make-block-branch-preview"
              :class="{ 'blockwork-wrap-ending': shape === 'Ending' }"
              :style="{ '--blockwork-custom-block-color': color }"
              ref="previewShapeEl"
            >
              <div class="wrap-head-line">
                <Blocks class="instruction-type-icon-inline" />
                <template v-for="(piece, i) in pieces" :key="piece.id">
                  <span
                    v-if="i < firstBranchIndex || piece.kind === 'Input'"
                    class="make-block-piece"
                    :ref="(el) => setPieceEl(i, el)"
                    :class="{ 'make-block-piece-input': piece.kind === 'Input' && !isBoolPiece(piece), 'make-block-piece-bool': isBoolPiece(piece), 'make-block-piece-selected': selectedIndex === i }"
                  >
                    <span class="make-block-piece-field">
                      <span class="make-block-piece-text" :class="{ 'make-block-piece-text-hidden': editingIndex === i }" @click="startEditing(i)">{{ editingIndex === i ? editingText || ' ' : pieceText(piece) }}</span>
                      <input v-if="editingIndex === i" :ref="(el) => (editInputEl = el as HTMLInputElement | null)" type="text" class="make-block-piece-input-el" v-model="editingText" @blur="commitEditing" @keydown.enter="commitEditing" @keydown.esc="editingIndex = null" />
                    </span>
                  </span>
                </template>
              </div>
              <template v-for="(branchIndex, branchOrdinal) in branchIndexes" :key="pieces[branchIndex].id">
                <div v-if="branchOrdinal > 0" class="wrap-mid-bar">
                  <span class="make-block-bar-label-field">
                  <span class="make-block-bar-label-measure" aria-hidden="true">{{ separatorText(branchOrdinal - 1) || 'Add label' }}</span>
                  <input
                    class="make-block-bar-label"
                    type="text"
                    :aria-label="`Label between branches ${branchOrdinal} and ${branchOrdinal + 1}`"
                    placeholder="Add label"
                    :value="separatorText(branchOrdinal - 1)"
                    @pointerdown.stop
                    @input="updateSeparator(branchOrdinal - 1, $event)"
                  />
                  </span>
                </div>
                <div class="wrap-mouth make-block-branch-mouth">
                  <BranchHeaderPiece block-id="" :name="pieceText(pieces[branchIndex])" :color="color" editable>
                    <span class="make-block-piece" :ref="el => setPieceEl(branchIndex, el)" :class="{ 'make-block-piece-selected': selectedIndex === branchIndex }">
                      <span class="make-block-piece-field">
                        <span class="make-block-piece-text" :class="{ 'make-block-piece-text-hidden': editingIndex === branchIndex }" @click="startEditing(branchIndex)">{{ editingIndex === branchIndex ? editingText || ' ' : pieceText(pieces[branchIndex]) }}</span>
                        <input v-if="editingIndex === branchIndex" type="text" class="make-block-piece-input-el" v-model="editingText" @blur="commitEditing" @keydown.enter="commitEditing" @keydown.esc="editingIndex = null" />
                      </span>
                    </span>
                  </BranchHeaderPiece>
                </div>
              </template>
              <div class="wrap-foot-bar" />
            </div>

            <div
              v-else
              class="instruction-row blockwork-custom-block"
              :class="{ 'instruction-row-cap': shape === 'Ending' }"
              :style="{ '--blockwork-custom-block-color': color }"
            >
              <div class="instruction-shape" ref="previewShapeEl">
                <Blocks class="instruction-type-icon" />
                <div class="instruction-content" @pointerdown.self="selectedIndex = null">
                  <template v-for="(piece, i) in pieces" :key="piece.id">
                    <span
                      class="make-block-piece"
                      :ref="(el) => setPieceEl(i, el)"
                      :class="{ 'make-block-piece-input': piece.kind === 'Input' && !isBoolPiece(piece), 'make-block-piece-bool': isBoolPiece(piece), 'make-block-piece-branch': piece.kind === 'Branch', 'make-block-piece-selected': selectedIndex === i }"
                    >
                      <span class="make-block-piece-field">
                        <span
                          class="make-block-piece-text"
                          :class="{ 'make-block-piece-text-hidden': editingIndex === i }"
                          @click="startEditing(i)"
                        >{{ editingIndex === i ? editingText || ' ' : pieceText(piece) }}</span>
                        <input
                          v-if="editingIndex === i"
                          :ref="(el) => (editInputEl = el as HTMLInputElement | null)"
                          type="text"
                          class="make-block-piece-input-el"
                          v-model="editingText"
                          @blur="commitEditing"
                          @keydown.enter="commitEditing"
                          @keydown.esc="editingIndex = null"
                        />
                      </span>
                    </span>
                  </template>
                </div>
              </div>
            </div>
            </div>
          </div>
        </div>

        <div class="make-block-color-row">
          <div class="make-block-color-picker-wrap">
          <div class="make-block-color-picker" role="radiogroup" aria-label="Block color">
            <button
              v-for="preset in COLOR_PRESETS"
              :key="preset"
              type="button"
              class="make-block-color-swatch"
              :class="{ 'make-block-color-swatch-selected': color === preset }"
              :style="{ background: preset }"
              :aria-label="`Use ${preset}`"
              :aria-checked="color === preset"
              role="radio"
              @click="selectColor(preset)"
            />
            <button
              type="button"
              class="make-block-color-swatch make-block-color-custom"
              :class="{ 'make-block-color-swatch-selected': customPickerOpen || !isPresetColor }"
              title="Choose a custom color"
              aria-label="Choose a custom block color"
              @click="openCustomPicker"
            >
              <Pipette aria-hidden="true" />
            </button>
          </div>
          <div v-if="customPickerOpen" class="make-block-manual-picker">
            <div class="make-block-manual-preview" :style="{ background: customColor }" aria-hidden="true" />
            <div class="make-block-manual-controls">
              <label>Hue <input v-model.number="customHue" type="range" min="0" max="359" @input="updateCustomColor" /></label>
              <label>Saturation <input v-model.number="customSaturation" type="range" min="0" max="100" @input="updateCustomColor" /></label>
              <label>Lightness <input v-model.number="customLightness" type="range" min="0" max="100" @input="updateCustomColor" /></label>
            </div>
            <span class="make-block-manual-hex">{{ customColor }}</span>
            <button type="button" class="btn-primary make-block-manual-done" @click="customPickerOpen = false">Done</button>
          </div>
          </div>
        </div>

        <div class="make-block-add-row">
          <button type="button" class="make-block-add-btn" @click="addPiece('Input')">
            <span class="make-block-add-preview make-block-add-preview-input">123</span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add an input</span>
              <span class="make-block-add-sub">number or text</span>
            </span>
          </button>
          <button type="button" class="make-block-add-btn" @click="addPiece('Input', 'Bool')">
            <span class="make-block-add-preview">
              <span class="value-block value-hex-blank">
                <span class="value-op value-hex-blank-spacer">&nbsp;</span>
              </span>
            </span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add an input</span>
              <span class="make-block-add-sub">boolean</span>
            </span>
          </button>
          <button type="button" class="make-block-add-btn" @click="addPiece('Branch')">
            <span class="make-block-add-preview make-block-add-preview-branch">⌞</span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add an input</span>
              <span class="make-block-add-sub">branch</span>
            </span>
          </button>
          <button type="button" class="make-block-add-btn" @click="addPiece('Label')">
            <span class="make-block-add-preview make-block-add-preview-label">Abc</span>
            <span class="make-block-add-text">
              <span class="make-block-add-title">Add a label</span>
            </span>
          </button>
        </div>

        <div class="make-block-add-row">
          <button
            type="button"
            class="make-block-add-btn make-block-shape-btn"
            :class="{ 'make-block-shape-btn-selected': shape === (isValueMode ? 'ReturnsValue' : 'Normal') }"
            @click="selectPrimaryShape"
          >
            <span class="make-block-add-title">{{ isValueMode ? 'Return Text or Number' : 'Normal block' }}</span>
          </button>
          <button
            type="button"
            class="make-block-add-btn make-block-shape-btn"
            :class="{ 'make-block-shape-btn-selected': shape === (isValueMode ? 'ReturnsBool' : 'Ending') }"
            @click="selectSecondaryShape"
          >
            <span class="make-block-add-title">{{ isValueMode ? 'Return a Boolean' : 'Ending block' }}</span>
          </button>
        </div>

        <div class="make-block-return-row">
          <label class="make-block-radio">
            <input type="checkbox" :checked="isValueMode" @change="onToggleReturnsValue" /> Returns a value
          </label>
        </div>

        <span v-if="error" class="invalid-hint">{{ error }}</span>
        <div class="modal-actions">
          <button type="button" @click="onCancel">Cancel</button>
          <button type="button" class="btn-primary" :disabled="submitting" @click="onOk">OK</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
