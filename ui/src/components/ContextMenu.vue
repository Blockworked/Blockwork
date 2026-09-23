<script setup lang="ts">
// Thin wrapper around blockstitch's generic <ContextMenuPanel> - this is where
// Blockwork's own menu content (what each menu variant offers, and what each
// item does) lives; the panel itself only handles positioning, outside-click/
// Escape/scroll close, and rendering the item list.
import { computed } from 'vue';
import { state } from '../store';
import { contextMenu, closeContextMenu } from '../contextMenu';
import { copyAll, copyBlock, clipboardContents, hasClipboard } from '../clipboard';
import { addInstruction, clearInstructions, createAttachedComment, createComment, deleteBlock, deleteInstruction, deleteList, deleteVariable, pasteInstructions, setCommentCollapsed, setRecordingTarget } from '../tauri';
import { ContextMenuPanel, clientToCanvas, focusCommentOnMount, type ContextMenuItem } from 'blockstitch';
import { ICONS } from '../icons';
import { openRenameVariableDialog } from '../variableDialogs';
import { openRenameListDialog } from 'blockstitch';
import { forgetListEditor } from 'blockstitch';
import { openDeleteUsageDialog } from '../deleteUsageDialog';
import { macroUsesList, macroUsesVariable } from '../entityUsage';
import { openEditBlockDialog } from '../blockDialogs';
import { findBlockDef, nextSiblingPath, regenerateInstructionIds, resolveInstructionAt } from '../types';
import type { InstructionType } from '../types';
import { detailsForBlockDef, detailsForInstructionType, detailsForList, detailsForValueKind, type BlockDetails } from '../blockDetails';
import { openDetailsDialog } from '../detailsDialog';

// Default offset (canvas units) a freshly-attached comment spawns at,
// relative to its block - clear of the block itself, matching the spirit of
// commands.rs's next_strand_position ("offset from the existing thing").
const ATTACHED_COMMENT_OFFSET = { dx: 220, dy: 0 };

const strand = computed(() => state.current_macro?.strands.find(s => s.id === contextMenu.strandId) ?? null);
const instruction = computed(() => resolveInstructionAt(strand.value, contextMenu.path));
const headerBlockId = computed(() => (instruction.value?.type === 'BlockHeader' ? instruction.value.block_id : null));
// A block/header can carry at most one attached comment (Scratch-style) -
// this is that comment, if one already exists, so the menu can offer
// "Comment" (focus it) instead of creating a duplicate.
const attachedComment = computed(() => {
  const id = instruction.value?.id;
  if (!id) return null;
  return state.current_macro?.comments?.find(c => c.attached_to === id) ?? null;
});
const isRecordingTarget = computed(
  () => contextMenu.strandId !== '' && contextMenu.strandId === state.current_macro?.recording_target_strand_id,
);

// What the "Details" item shows for the current 'value' menu target - a
// placed operator or "My Blocks" reporter call on the canvas (openValueMenu
// in contextMenu.ts already declines to open for anything else, e.g. a
// Var/Param reporter with no fixed behavior to explain).
const valueDetails = computed<BlockDetails | null>(() => {
  const v = contextMenu.valueNode;
  if (!v) return null;
  if (v.kind === 'Call') {
    const def = findBlockDef(state.current_macro, v.block_id);
    return def ? detailsForBlockDef(def) : null;
  }
  return detailsForValueKind(v.kind === 'Op' ? v.op : v.kind);
});

// What the "Details" item shows for the current 'block' menu target - a
// BlockHeader row or a CallBlock instruction both describe a custom block
// (the "My Blocks" definition itself), everything else is a fixed built-in
// instruction type.
const blockDetails = computed<BlockDetails | null>(() => {
  const ins = instruction.value;
  if (!ins) return null;
  const callId = headerBlockId.value ?? (ins.type === 'CallBlock' ? ins.block_id : null);
  if (callId) {
    const def = findBlockDef(state.current_macro, callId);
    if (def) return detailsForBlockDef(def);
  }
  return detailsForInstructionType(ins.type);
});

const totalBlocks = computed(() =>
  (state.current_macro?.strands ?? []).reduce((n, s) => n + s.instructions.length, 0),
);
const clearIcon = computed(() => (state.confirm_clear_instructions ? 'alert-triangle' : 'trash'));
const clearLabel = computed(() =>
  state.confirm_clear_instructions
    ? `Confirm delete (${state.confirm_clear_instructions_remaining_secs}s)?`
    : `Delete ${totalBlocks.value} Blocks`,
);

function deleteBlockPosition(): [number, number] {
  return clientToCanvas(contextMenu.x, contextMenu.y);
}

function onSetRecordingTarget() {
  setRecordingTarget(contextMenu.strandId);
  closeContextMenu();
}
function onDeleteBlock() {
  // A custom block's header row: delete the block definition (which also
  // removes this body strand and every call site referencing it) rather
  // than just detaching the strand and leaving an orphaned "My Blocks"
  // prefab behind - mirrors the drag-to-trash handling in blockstitch's
  // canvas/canvasDrag.ts.
  if (headerBlockId.value) {
    deleteBlock(headerBlockId.value);
    closeContextMenu();
    return;
  }
  const [x, y] = deleteBlockPosition();
  deleteInstruction(contextMenu.strandId, contextMenu.path, x, y);
  closeContextMenu();
}
function onCopyBlock() {
  if (strand.value) copyBlock(strand.value, contextMenu.path);
  closeContextMenu();
}
function onCopyAll() {
  if (strand.value) copyAll(strand.value, contextMenu.path);
  closeContextMenu();
}
function onCutBlock() {
  if (strand.value) copyBlock(strand.value, contextMenu.path);
  const [x, y] = deleteBlockPosition();
  deleteInstruction(contextMenu.strandId, contextMenu.path, x, y);
  closeContextMenu();
}
function onDuplicateBlock() {
  const ins = instruction.value;
  if (ins) addInstruction(contextMenu.strandId, nextSiblingPath(contextMenu.path), regenerateInstructionIds(ins));
  closeContextMenu();
}
async function onAddOrFocusComment() {
  const existing = attachedComment.value;
  if (existing) {
    if (existing.collapsed) await setCommentCollapsed(existing.id, false);
    focusCommentOnMount(existing.id);
  } else {
    const id = instruction.value?.id;
    if (id) focusCommentOnMount(await createAttachedComment(id, ATTACHED_COMMENT_OFFSET.dx, ATTACHED_COMMENT_OFFSET.dy, ''));
  }
  closeContextMenu();
}
async function onAddCanvasComment() {
  focusCommentOnMount(await createComment(contextMenu.canvasX, contextMenu.canvasY, ''));
  closeContextMenu();
}
function onDeleteAllBlocks() {
  // Deliberately doesn't close the menu - the two-click confirm (armed on
  // first click, executed on second, matching the toolbar button this
  // replaced) needs the menu to stay open so the label/icon can update in
  // between; it closes via the normal outside-click/Escape handling once
  // the user's done with it.
  clearInstructions();
}
function onPaste() {
  const contents = clipboardContents();
  if (contents) pasteInstructions(contextMenu.canvasX, contextMenu.canvasY, contents.map(regenerateInstructionIds));
  closeContextMenu();
}
function onRenameVariable() {
  openRenameVariableDialog(contextMenu.variableName);
  closeContextMenu();
}
function onDeleteVariable() {
  if (macroUsesVariable(state.current_macro, contextMenu.variableName)) {
    openDeleteUsageDialog('variable', contextMenu.variableName);
    closeContextMenu();
    return;
  }
  deleteVariable(contextMenu.variableName);
  closeContextMenu();
}
function onRenameList() {
  openRenameListDialog(contextMenu.listName);
  closeContextMenu();
}
function onDeleteList() {
  if (macroUsesList(state.current_macro, contextMenu.listName)) {
    openDeleteUsageDialog('list', contextMenu.listName);
    closeContextMenu();
    return;
  }
  deleteList(contextMenu.listName);
  forgetListEditor(contextMenu.listName);
  closeContextMenu();
}
function onShowListDetails() {
  openDetailsDialog(detailsForList(contextMenu.listName));
  closeContextMenu();
}
function onEditBlock() {
  const def = findBlockDef(state.current_macro, contextMenu.blockId);
  if (def) openEditBlockDialog(def);
  closeContextMenu();
}
function onEditBlockHeader() {
  const def = headerBlockId.value ? findBlockDef(state.current_macro, headerBlockId.value) : undefined;
  if (def) openEditBlockDialog(def);
  closeContextMenu();
}
function onDeleteBlockDef() {
  deleteBlock(contextMenu.blockId);
  closeContextMenu();
}
function onShowBlockDetails() {
  if (blockDetails.value) openDetailsDialog(blockDetails.value);
  closeContextMenu();
}
function onShowMyBlockDetails() {
  const def = findBlockDef(state.current_macro, contextMenu.blockId);
  if (def) openDetailsDialog(detailsForBlockDef(def));
  closeContextMenu();
}
function onShowPaletteInstructionDetails() {
  openDetailsDialog(detailsForInstructionType(contextMenu.paletteInstructionType as InstructionType));
  closeContextMenu();
}
function onShowPaletteValueDetails() {
  openDetailsDialog(detailsForValueKind(contextMenu.paletteValueKind));
  closeContextMenu();
}
function onShowValueDetails() {
  if (valueDetails.value) openDetailsDialog(valueDetails.value);
  closeContextMenu();
}

const items = computed<ContextMenuItem[]>(() => {
  if (contextMenu.type === 'block') {
    const list: ContextMenuItem[] = [];
    if (headerBlockId.value) {
      list.push({ key: 'edit-header', label: 'Edit block', icon: ICONS.blocks, onSelect: onEditBlockHeader });
    }
    list.push(
      { key: 'record-target', label: isRecordingTarget.value ? 'Recording Target (current)' : 'Set Recording Target', icon: ICONS.target, active: isRecordingTarget.value, onSelect: onSetRecordingTarget },
      { key: 'copy', label: 'Copy block', icon: ICONS.copy, onSelect: onCopyBlock },
      { key: 'copy-all', label: 'Copy all', icon: ICONS.layers, onSelect: onCopyAll },
      { key: 'duplicate', label: 'Duplicate block', icon: ICONS['corner-down-right'], onSelect: onDuplicateBlock },
      { key: 'cut', label: 'Cut block', icon: ICONS.scissors, onSelect: onCutBlock },
      { key: 'comment', label: attachedComment.value ? 'Comment' : 'Add Comment', icon: ICONS['message-square'], onSelect: onAddOrFocusComment },
      { key: 'details', label: 'Details', icon: ICONS.info, onSelect: onShowBlockDetails },
      { key: 'delete', label: 'Delete block', icon: ICONS.trash, danger: true, onSelect: onDeleteBlock },
    );
    return list;
  }
  if (contextMenu.type === 'canvas') {
    return [
      {
        key: 'clear', label: clearLabel.value, icon: ICONS[clearIcon.value], danger: true,
        extraClass: state.confirm_clear_instructions ? 'confirm-armed' : undefined, onSelect: onDeleteAllBlocks,
      },
      { key: 'paste', label: 'Paste', icon: ICONS['clipboard-paste'], disabled: !hasClipboard(), onSelect: onPaste },
      { key: 'add-comment', label: 'Add Comment', icon: ICONS['message-square'], onSelect: onAddCanvasComment },
    ];
  }
  if (contextMenu.type === 'variable') {
    return [
      { key: 'rename', label: 'Rename variable', icon: ICONS.equal, onSelect: onRenameVariable },
      { key: 'delete-var', label: 'Delete variable', icon: ICONS.trash, danger: true, onSelect: onDeleteVariable },
    ];
  }
  if (contextMenu.type === 'list') {
    return [
      { key: 'rename-list', label: 'Rename list', icon: ICONS.equal, onSelect: onRenameList },
      { key: 'list-details', label: 'Details', icon: ICONS.info, onSelect: onShowListDetails },
      { key: 'delete-list', label: 'Delete list', icon: ICONS.trash, danger: true, onSelect: onDeleteList },
    ];
  }
  if (contextMenu.type === 'paletteInstruction') {
    return [{ key: 'details', label: 'Details', icon: ICONS.info, onSelect: onShowPaletteInstructionDetails }];
  }
  if (contextMenu.type === 'paletteValue') {
    return [{ key: 'details', label: 'Details', icon: ICONS.info, onSelect: onShowPaletteValueDetails }];
  }
  if (contextMenu.type === 'value') {
    return [{ key: 'details', label: 'Details', icon: ICONS.info, onSelect: onShowValueDetails }];
  }
  return [
    { key: 'edit', label: 'Edit block', icon: ICONS.blocks, onSelect: onEditBlock },
    { key: 'details', label: 'Details', icon: ICONS.info, onSelect: onShowMyBlockDetails },
    { key: 'delete-def', label: 'Delete block', icon: ICONS.trash, danger: true, onSelect: onDeleteBlockDef },
  ];
});
</script>

<template>
  <ContextMenuPanel :open="contextMenu.open" :x="contextMenu.x" :y="contextMenu.y" :items="items" @close="closeContextMenu" />
</template>
