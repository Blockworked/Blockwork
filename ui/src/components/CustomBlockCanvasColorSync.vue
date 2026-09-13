<script setup lang="ts">
// blockstitch owns the canvas row element, so keep its custom-block accent at
// that level rather than depending on a nested field component's mount order.
// This runs after every backend state patch, including a drag that changes a
// custom block header's strand contents.
import { nextTick, onBeforeUnmount, onMounted, watch } from 'vue';
import { state } from '../store';
import { blockBranchPieces, resolveInstructionAt } from '../types';
import type { InstructionDto, ValueDto, ValueLocationDto } from '../types';

function collectCustomBlockRows(
  instructions: InstructionDto[],
  rows: Map<string, string>,
  standaloneBranchOwners: Map<string, string | null>,
  definitionBlockId: string | null = null,
) {
  const strandDefinitionBlockId = definitionBlockId
    ?? instructions.find((instruction): instruction is Extract<InstructionDto, { type: 'BlockHeader' }> => instruction.type === 'BlockHeader')?.block_id
    ?? null;
  for (const instruction of instructions) {
    if (instruction.type === 'BlockHeader' || instruction.type === 'CallBlock' || instruction.type === 'BranchCallBlock') {
      rows.set(instruction.id, instruction.block_id);
    }
    if (instruction.type === 'RunBranch') {
      const blockId = strandDefinitionBlockId ?? standaloneBranchOwners.get(instruction.name);
      if (blockId) rows.set(instruction.id, blockId);
    }
    if (instruction.type === 'If' || instruction.type === 'Repeat' || instruction.type === 'Forever' || instruction.type === 'While') {
      collectCustomBlockRows(instruction.body, rows, standaloneBranchOwners, strandDefinitionBlockId);
    } else if (instruction.type === 'IfElse') {
      collectCustomBlockRows(instruction.then_body, rows, standaloneBranchOwners, strandDefinitionBlockId);
      collectCustomBlockRows(instruction.else_body, rows, standaloneBranchOwners, strandDefinitionBlockId);
    } else if (instruction.type === 'BranchCallBlock') {
      instruction.branches.forEach(branch => collectCustomBlockRows(branch, rows, standaloneBranchOwners, strandDefinitionBlockId));
    }
  }
}

function fieldRootValue(instruction: InstructionDto, fieldId: string): ValueDto | null {
  switch (instruction.type) {
    case 'Wait': return fieldId === 'WaitDuration' ? instruction.duration : null;
    case 'Text': return fieldId === 'TextValue' ? instruction.text : null;
    case 'MoveMouse': return fieldId === 'MoveMouseX' ? instruction.x : fieldId === 'MoveMouseY' ? instruction.y : null;
    case 'Scroll': return fieldId === 'ScrollAmount' ? instruction.amount : null;
    case 'SetVariable': return fieldId === 'SetVariableValue' ? instruction.value : null;
    case 'ChangeVariable': return fieldId === 'ChangeVariableValue' ? instruction.value : null;
    case 'Return': return fieldId === 'ReturnValue' ? instruction.value : null;
    case 'CallBlock':
    case 'BranchCallBlock': {
      const index = fieldId.startsWith('CallArg:') ? Number(fieldId.slice('CallArg:'.length)) : NaN;
      return Number.isInteger(index) ? instruction.args[index] ?? null : null;
    }
    case 'If': case 'IfElse': case 'While': return fieldId === 'Condition' ? instruction.condition : null;
    case 'Repeat': return fieldId === 'RepeatCount' ? instruction.count : null;
    case 'WhenBatteryDischargedTo': return fieldId === 'BatteryDischargeThreshold' ? instruction.threshold : null;
    case 'WhenBatteryChargedTo': return fieldId === 'BatteryChargeThreshold' ? instruction.threshold : null;
    default: return null;
  }
}

function valueAtPath(value: ValueDto | null, path: number[]): ValueDto | null {
  for (const index of path) {
    if (!value || (value.kind !== 'Op' && value.kind !== 'Call')) return null;
    value = value.args[index] ?? null;
  }
  return value;
}

function valueAtLocation(location: ValueLocationDto): ValueDto | null {
  const macro = state.current_macro;
  if (!macro) return null;
  if (location.kind === 'Floating') {
    return valueAtPath(macro.floating_values.find(value => value.id === location.floating_id)?.value ?? null, location.path);
  }
  const strand = macro.strands.find(strand => strand.id === location.strand_id);
  const instruction = resolveInstructionAt(strand, location.index);
  return valueAtPath(instruction ? fieldRootValue(instruction, location.field_id) : null, location.path);
}

function ownerBlockIdForParam(location: ValueLocationDto): string | null {
  const macro = state.current_macro;
  if (!macro) return null;
  if (location.kind === 'Floating') {
    // Fresh parameter reporters preserve their source definition when they
    // are pulled from a custom block header onto the canvas.
    return macro.floating_values.find(value => value.id === location.floating_id)?.origin_block_id ?? null;
  }
  // A parameter living in a field belongs to the custom block whose body
  // strand it is in; that strand always starts with this header.
  const header = macro.strands.find(strand => strand.id === location.strand_id)?.instructions[0];
  return header?.type === 'BlockHeader' ? header.block_id : null;
}

function syncCanvasColors() {
  const macro = state.current_macro;
  if (!macro) return;
  const blockIdsByInstruction = new Map<string, string>();
  // A RunBranch marker normally lives under its definition header. A marker
  // parked on the canvas after being dragged from that header has no owner
  // strand yet, so use its branch name only when it identifies one definition
  // unambiguously.
  const standaloneBranchOwners = new Map<string, string | null>();
  for (const def of macro.block_defs) {
    for (const branch of blockBranchPieces(def)) {
      standaloneBranchOwners.set(
        branch.name,
        standaloneBranchOwners.has(branch.name) ? null : def.id,
      );
    }
  }
  for (const strand of macro.strands) collectCustomBlockRows(strand.instructions, blockIdsByInstruction, standaloneBranchOwners);
  const colorsByBlockId = new Map(macro.block_defs.map(def => [def.id, def.color]));

  document.querySelectorAll<HTMLElement>('#canvas-inner .instruction-row[data-instr-id]').forEach(row => {
    const blockId = blockIdsByInstruction.get(row.dataset.instrId ?? '');
    const color = blockId ? colorsByBlockId.get(blockId) : undefined;
    row.classList.toggle('blockwork-wrap-ending', row.classList.contains('instruction-row-wrap') && macro.block_defs.some(def => def.id === blockId && def.shape === 'Ending'));
    if (color) {
      row.classList.add('blockwork-custom-block');
      row.style.setProperty('--blockwork-custom-block-color', color);
    } else {
      row.classList.remove('blockwork-custom-block');
      row.style.removeProperty('--blockwork-custom-block-color');
    }
  });

  document.querySelectorAll<HTMLElement>('#canvas-inner [data-value-location]').forEach(element => {
    let location: ValueLocationDto;
    try {
      location = JSON.parse(element.dataset.valueLocation ?? '') as ValueLocationDto;
    } catch {
      return;
    }
    const value = valueAtLocation(location);
    const blockId = value?.kind === 'Call'
      ? value.block_id
      : value?.kind === 'Param'
        ? ownerBlockIdForParam(location)
        : null;
    const color = blockId ? colorsByBlockId.get(blockId) : undefined;
    if (color) {
      element.classList.add('blockwork-custom-value-block');
      element.classList.toggle('blockwork-custom-operator', value?.kind === 'Call');
      element.style.setProperty('--blockwork-custom-block-color', color);
    } else {
      element.classList.remove('blockwork-custom-value-block');
      element.classList.remove('blockwork-custom-operator');
      element.style.removeProperty('--blockwork-custom-block-color');
    }
  });
}

let syncQueued = false;
function scheduleSync() {
  if (syncQueued) return;
  syncQueued = true;
  queueMicrotask(() => {
    syncQueued = false;
    syncCanvasColors();
  });
}

// Floating value cards can mount after the macro-state watcher has completed
// its post-render pass. Watch the canvas itself as well so a just-created
// reporter block is colored on the exact DOM insertion that displays it.
let canvasObserver: MutationObserver | null = null;
let valueDragObserver: MutationObserver | null = null;

function syncValueDragState() {
  // Value drags are owned by blockstitch and their temporary ghost is placed
  // directly under <body>. Expose that short-lived state to the app theme so
  // a source header does not keep its hover accent while its parameter is
  // being carried elsewhere.
  document.body.classList.toggle('blockwork-value-dragging', !!document.querySelector('.value-drag-ghost'));
}

onMounted(async () => {
  await nextTick();
  scheduleSync();
  const canvas = document.getElementById('canvas-inner');
  if (canvas) {
    canvasObserver = new MutationObserver(scheduleSync);
    canvasObserver.observe(canvas, { childList: true, subtree: true });
  }
  valueDragObserver = new MutationObserver(syncValueDragState);
  valueDragObserver.observe(document.body, { childList: true });
  syncValueDragState();
});
onBeforeUnmount(() => {
  canvasObserver?.disconnect();
  valueDragObserver?.disconnect();
  document.body.classList.remove('blockwork-value-dragging');
});
watch(() => state.current_macro, scheduleSync, { deep: true, flush: 'post' });
</script>

<template />
