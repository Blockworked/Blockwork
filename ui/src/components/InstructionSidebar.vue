<script setup lang="ts">
import { computed, watchEffect } from 'vue';
import { Trash2 } from 'lucide-vue-next';
import { INSTRUCTION_TYPE_LABELS } from '../icons';
import { PaletteInstructionBlock, PaletteValueBlock, beginSidebarResize, sidebarWidth, type ValueNode } from 'blockstitch';
import PaletteCallBlock from './PaletteCallBlock.vue';
import PaletteCallValueBlock from './PaletteCallValueBlock.vue';
import MakeVariableDialog from './MakeVariableDialog.vue';
import MakeBlockDialog from './MakeBlockDialog.vue';
import MakeListDialog from './MakeListDialog.vue';
import ListPanel from './ListPanel.vue';
import { OPERATOR_KINDS, specForKind, setListNameOptions } from '../valueOps';
import { applyPaletteValueEdit, paletteInstructions, paletteValueFor, syncPaletteTargetDefaults } from '../paletteState';
import { state } from '../store';
import { blockShapeReturnsValue, sortedListNames, sortedVariableNames } from '../types';
import type { InstructionDto, ValueDto, ValueKind } from '../types';
import { closeVariableDialog, openCreateVariableDialog, variableDialog } from '../variableDialogs';
import { blockDialog, closeBlockDialog, openCreateBlockDialog } from '../blockDialogs';
import { closeListDialog, listDialog, openCreateListDialog } from '../listDialogs';

// SetVariable/ChangeVariable render in the Variables section below and
// Return in the "My Blocks" section, not here; BlockHeader/CallBlock are
// never dragged from a fixed prefab at all (see PaletteCallBlock.vue);
// Comment is a floating note now (right-click canvas/a block), not a sidebar
// prefab - all filtered out of the generic "Instruction" group.
const instructionTypes = (Object.keys(INSTRUCTION_TYPE_LABELS) as InstructionDto['type'][])
  .filter((t): t is Exclude<InstructionDto['type'], 'SetVariable' | 'ChangeVariable' | 'AddToList' | 'DeleteOfList' | 'DeleteAllOfList' | 'ShiftList' | 'InsertIntoList' | 'ReplaceItemOfList' | 'ReverseList' | 'BlockHeader' | 'CallBlock' | 'Return' | 'Comment'> =>
    !['SetVariable', 'ChangeVariable', 'AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList', 'BlockHeader', 'CallBlock', 'Return', 'Comment'].includes(t));

const commandBlocks = computed(() => (state.current_macro?.block_defs ?? []).filter(b => !blockShapeReturnsValue(b.shape)));
const reporterBlocks = computed(() => (state.current_macro?.block_defs ?? []).filter(b => blockShapeReturnsValue(b.shape)));
const LIST_COMMAND_TYPES = ['AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList'] as const;

// Number/Text literals, plus every operator registered in valueOps.ts's
// OPERATOR_KINDS - adding an operator there is enough to get it a palette
// entry, no edit needed here.
const LIST_VALUE_KINDS: ValueKind[] = ['ListItem', 'ListItemNumber', 'ListAmount', 'ListLength', 'ListContains', 'ListItemExists', 'ListIsEmpty'];
const VALUE_KINDS: ValueKind[] = ['Number', 'Text', ...OPERATOR_KINDS.filter(s => !LIST_VALUE_KINDS.includes(s.kind)).map(s => s.kind)];

// One reporter block per declared variable, alphabetical.
const variableKinds = computed<ValueKind[]>(() => sortedVariableNames(state.current_macro).map(n => `Var:${n}` as ValueKind));

// PaletteValueBlock derives its hexagon-vs-capsule shape from blockstitch's
// own operator registry (via `specForKind(kind)`), but the canvas's
// ValueBlock derives the same shape from `specForOp(op)` - drive the palette
// explicitly from Blockwork's own registry instead, so the two can never
// disagree (a boolean-returning operator must already look boolean here,
// not only after it's dropped onto the canvas).
function isBoolKind(kind: ValueKind): boolean {
  return specForKind(kind)?.resultType === 'bool';
}

// A running older backend may briefly send a macro DTO without `lists` while
// the frontend reloads after an update. Treat that exactly like no lists
// rather than throwing during sidebar render.
watchEffect(() => {
  const variableNames = sortedVariableNames(state.current_macro);
  const listNames = sortedListNames(state.current_macro);
  setListNameOptions(listNames);
  syncPaletteTargetDefaults(variableNames, listNames);
});

// blockstitch's PaletteValueBlock emits its own generic ValueNode shape (its
// `op` is a plain string, since blockstitch doesn't know Blockwork's ValueOp
// union) - safe to treat as Blockwork's own ValueDto here, since Blockwork's
// operator registry is the only thing that ever populates it.
function onValueUpdate(kind: ValueKind, next: ValueNode) {
  applyPaletteValueEdit(kind, next as ValueDto);
}

// Empty palette space has no app action, so a browser menu there is just
// distracting. Keep native text-input menus intact for normal copy/paste.
function onSidebarContextMenu(event: MouseEvent) {
  if ((event.target as Element | null)?.closest('input, textarea')) return;
  event.preventDefault();
}
</script>

<template>
  <div class="instruction-sidebar" id="instruction-sidebar" :style="{ width: sidebarWidth + 'px' }" @contextmenu="onSidebarContextMenu">
    <div class="sidebar-trash-hint">
      <Trash2 />
      <span>Drag a block here to delete it</span>
    </div>
    <div class="sidebar-scroll">
      <div class="sidebar-palette" id="sidebar-palette">
        <PaletteInstructionBlock v-for="type in instructionTypes" :key="type" :type="type" :instruction="paletteInstructions[type]" />
      </div>

      <div class="sidebar-section-label">Operator</div>
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-values">
        <PaletteValueBlock
          v-for="kind in VALUE_KINDS"
          :key="kind"
          :kind="kind"
          :value="paletteValueFor(kind)"
          :bool-override="isBoolKind(kind)"
          @update:value="v => onValueUpdate(kind, v)"
        />
      </div>

      <div class="sidebar-section-label-row">
        <span class="sidebar-section-label">Variables</span>
        <button type="button" class="btn-make-variable" @click="openCreateVariableDialog">Make a Variable</button>
      </div>
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-variables">
        <PaletteValueBlock v-for="kind in variableKinds" :key="kind" :kind="kind" />
      </div>
      <div class="sidebar-palette">
        <PaletteInstructionBlock type="SetVariable" :instruction="paletteInstructions.SetVariable" />
        <PaletteInstructionBlock type="ChangeVariable" :instruction="paletteInstructions.ChangeVariable" />
      </div>

      <div class="sidebar-section-label-row">
        <span class="sidebar-section-label">Lists</span>
        <button type="button" class="btn-make-variable" @click="openCreateListDialog">Make a List</button>
      </div>
      <ListPanel />
      <div class="sidebar-palette sidebar-palette-values" id="sidebar-palette-lists">
        <PaletteValueBlock
          v-for="kind in LIST_VALUE_KINDS"
          :key="kind"
          :kind="kind"
          :value="paletteValueFor(kind)"
          @update:value="v => onValueUpdate(kind, v)"
        />
      </div>
      <div class="sidebar-palette">
        <PaletteInstructionBlock v-for="type in LIST_COMMAND_TYPES" :key="type" :type="type" :instruction="paletteInstructions[type]" />
      </div>

      <div class="sidebar-section-label-row">
        <span class="sidebar-section-label">My Blocks</span>
        <button type="button" class="btn-make-variable" @click="openCreateBlockDialog">Make a Block</button>
      </div>
      <div class="sidebar-palette sidebar-palette-values" v-if="reporterBlocks.length">
        <PaletteCallValueBlock v-for="def in reporterBlocks" :key="def.id" :def="def" />
      </div>
      <div class="sidebar-palette">
        <PaletteCallBlock v-for="def in commandBlocks" :key="def.id" :def="def" />
        <PaletteInstructionBlock type="Return" :instruction="paletteInstructions.Return" />
      </div>
    </div>
    <div class="sidebar-resize-handle" @pointerdown="beginSidebarResize" />
  </div>
  <MakeVariableDialog
    v-if="variableDialog.mode"
    :rename-target="variableDialog.mode === 'rename' ? variableDialog.renameTarget : null"
    @close="closeVariableDialog"
  />
  <MakeBlockDialog
    v-if="blockDialog.mode"
    :edit-target="blockDialog.mode === 'edit' ? blockDialog.editTarget : null"
    @close="closeBlockDialog"
  />
  <MakeListDialog
    v-if="listDialog.open"
    :rename-target="listDialog.renameTarget || null"
    @close="closeListDialog"
  />
</template>
