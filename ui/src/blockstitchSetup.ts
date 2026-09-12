// The single startup wiring point between Blockwork and blockstitch - registers
// Blockwork's instruction vocabulary (shapes, field forms, icons, operators) and
// builds the CanvasBackend/CanvasHost blockstitch's canvas drives. Call
// `setupBlockstitch()` once, before mounting the app (see main.ts).
import {
  configureCanvas,
  locationsEqual,
  registerBlockField,
  registerBlockShape,
  registerIcons,
  registerOperators,
  registerPaletteBlockField,
  type CanvasBackend,
  type CanvasHost,
} from 'blockstitch';
import { state } from './store';
import * as tauri from './tauri';
import { ICONS, INSTRUCTION_TYPE_ICONS } from './icons';
import { OPERATOR_KINDS } from './valueOps';
import { clonePaletteInstruction, paletteValueFor } from './paletteState';
import { paletteCallInstructionFor, paletteCallValueFor } from './blockDefs';
import { findBlockDef, parseParamKind, type BlockPieceDto, type InstructionDto, type InstructionType, type ValueDto, type ValueKind } from './types';
import { openBlockMenu, openCanvasMenu, openPaletteInstructionMenu, openPaletteValueMenu, openValueMenu, openVariableMenu } from './contextMenu';

import WhenRanFields from './components/fields/WhenRanFields.vue';
import WhenBatteryDischargedToFields from './components/fields/WhenBatteryDischargedToFields.vue';
import WhenBatteryChargedToFields from './components/fields/WhenBatteryChargedToFields.vue';
import WhenTimeFields from './components/fields/WhenTimeFields.vue';
import WhenPowerPluggedInFields from './components/fields/WhenPowerPluggedInFields.vue';
import WhenPowerUnpluggedFields from './components/fields/WhenPowerUnpluggedFields.vue';
import WaitFields from './components/fields/WaitFields.vue';
import TextFields from './components/fields/TextFields.vue';
import KeyFields from './components/fields/KeyFields.vue';
import ButtonFields from './components/fields/ButtonFields.vue';
import MoveMouseFields from './components/fields/MoveMouseFields.vue';
import ScrollFields from './components/fields/ScrollFields.vue';
import CommandFields from './components/fields/CommandFields.vue';
import OpenAppFields from './components/fields/OpenAppFields.vue';
import CloseAppFields from './components/fields/CloseAppFields.vue';
import CommentFields from './components/fields/CommentFields.vue';
import SetVariableFields from './components/fields/SetVariableFields.vue';
import ChangeVariableFields from './components/fields/ChangeVariableFields.vue';
import ListCommandFields from './components/fields/ListCommandFields.vue';
import BlockHeaderFields from './components/fields/BlockHeaderFields.vue';
import CallBlockFields from './components/fields/CallBlockFields.vue';
import ReturnFields from './components/fields/ReturnFields.vue';
import IfFields from './components/fields/IfFields.vue';
import IfElseFields from './components/fields/IfElseFields.vue';
import RepeatFields from './components/fields/RepeatFields.vue';
import ForeverFields from './components/fields/ForeverFields.vue';
import WhileFields from './components/fields/WhileFields.vue';
import EscapeLoopFields from './components/fields/EscapeLoopFields.vue';
import ContinueLoopFields from './components/fields/ContinueLoopFields.vue';

import PaletteWhenRanFields from './components/fields/palette/PaletteWhenRanFields.vue';
import PaletteWhenBatteryDischargedToFields from './components/fields/palette/PaletteWhenBatteryDischargedToFields.vue';
import PaletteWhenBatteryChargedToFields from './components/fields/palette/PaletteWhenBatteryChargedToFields.vue';
import PaletteWhenTimeFields from './components/fields/palette/PaletteWhenTimeFields.vue';
import PaletteWhenPowerPluggedInFields from './components/fields/palette/PaletteWhenPowerPluggedInFields.vue';
import PaletteWhenPowerUnpluggedFields from './components/fields/palette/PaletteWhenPowerUnpluggedFields.vue';
import PaletteWaitFields from './components/fields/palette/PaletteWaitFields.vue';
import PaletteTextFields from './components/fields/palette/PaletteTextFields.vue';
import PaletteKeyFields from './components/fields/palette/PaletteKeyFields.vue';
import PaletteButtonFields from './components/fields/palette/PaletteButtonFields.vue';
import PaletteMoveMouseFields from './components/fields/palette/PaletteMoveMouseFields.vue';
import PaletteScrollFields from './components/fields/palette/PaletteScrollFields.vue';
import PaletteCommandFields from './components/fields/palette/PaletteCommandFields.vue';
import PaletteOpenAppFields from './components/fields/palette/PaletteOpenAppFields.vue';
import PaletteCloseAppFields from './components/fields/palette/PaletteCloseAppFields.vue';
import PaletteSetVariableFields from './components/fields/palette/PaletteSetVariableFields.vue';
import PaletteChangeVariableFields from './components/fields/palette/PaletteChangeVariableFields.vue';
import PaletteListCommandFields from './components/fields/palette/PaletteListCommandFields.vue';
import PaletteReturnFields from './components/fields/palette/PaletteReturnFields.vue';
import PaletteIfFields from './components/fields/palette/PaletteIfFields.vue';
import PaletteIfElseFields from './components/fields/palette/PaletteIfElseFields.vue';
import PaletteRepeatFields from './components/fields/palette/PaletteRepeatFields.vue';
import PaletteForeverFields from './components/fields/palette/PaletteForeverFields.vue';
import PaletteWhileFields from './components/fields/palette/PaletteWhileFields.vue';
import PaletteEscapeLoopFields from './components/fields/palette/PaletteEscapeLoopFields.vue';
import PaletteContinueLoopFields from './components/fields/palette/PaletteContinueLoopFields.vue';

let didSetup = false;

export function setupBlockstitch(): void {
  if (didSetup) return;
  didSetup = true;

  registerIcons(ICONS);
  registerOperators(OPERATOR_KINDS);
  registerShapes();
  registerFields();
  configureCanvas(buildCanvasHost());
}

// ── Shapes ───────────────────────────────────────────────────────────────────
const HEADER_TYPES: InstructionType[] = ['WhenRan', 'BlockHeader', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'WhenTime', 'WhenPowerPluggedIn', 'WhenPowerUnplugged'];
const ENTRY_TRIGGER_TYPES = new Set<InstructionType>(['WhenRan', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'WhenTime', 'WhenPowerPluggedIn', 'WhenPowerUnplugged']);
const CAP_TYPES: InstructionType[] = ['Return', 'EscapeLoop', 'ContinueLoop'];
const STACK_TYPES: InstructionType[] = ['Wait', 'Text', 'Key', 'Button', 'MoveMouse', 'Scroll', 'Command', 'Comment', 'OpenApp', 'CloseApp', 'SetVariable', 'ChangeVariable', 'AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList'];

function registerShapes() {
  for (const type of HEADER_TYPES) {
    registerBlockShape(type, { kind: 'header', icon: iconFor(type), isEntryTrigger: ENTRY_TRIGGER_TYPES.has(type) });
  }
  for (const type of CAP_TYPES) {
    registerBlockShape(type, { kind: 'cap', icon: iconFor(type) });
  }
  for (const type of STACK_TYPES) {
    registerBlockShape(type, { kind: 'stack', icon: iconFor(type) });
  }
  // Every custom block shares this one instruction type regardless of which
  // BlockDef it calls, so - unlike CAP_TYPES above - "no bottom notch" can't
  // be a static per-type registration here; `isCap` asks the specific block
  // being called instead (see shapeRegistry.ts's BlockShapeDescriptor).
  registerBlockShape<InstructionDto>('CallBlock', {
    kind: 'stack',
    icon: iconFor('CallBlock'),
    isCap: n => n.type === 'CallBlock' && findBlockDef(state.current_macro, n.block_id)?.shape === 'Ending',
  });
  // TNode is `InstructionDto` (the full union), not just the wrap variants -
  // a wrap block's own body/slots hold arbitrary instructions, not only
  // other wrap blocks, so getSlots/mapSlots must operate over the whole
  // union and narrow `n` themselves via `n.type === '...'`.
  registerBlockShape<InstructionDto>('If', {
    kind: 'wrap',
    icon: iconFor('If'),
    getSlots: n => [n.type === 'If' ? n.body : []],
    mapSlots: (n, fn) => (n.type === 'If' ? { ...n, body: fn(n.body, 0) } : n),
  });
  registerBlockShape<InstructionDto>('IfElse', {
    kind: 'wrap',
    icon: iconFor('IfElse'),
    getSlots: n => (n.type === 'IfElse' ? [n.then_body, n.else_body] : [[], []]),
    mapSlots: (n, fn) => (n.type === 'IfElse' ? { ...n, then_body: fn(n.then_body, 0), else_body: fn(n.else_body, 1) } : n),
  });
  registerBlockShape<InstructionDto>('Repeat', {
    kind: 'wrap',
    icon: iconFor('Repeat'),
    getSlots: n => [n.type === 'Repeat' ? n.body : []],
    mapSlots: (n, fn) => (n.type === 'Repeat' ? { ...n, body: fn(n.body, 0) } : n),
  });
  registerBlockShape<InstructionDto>('Forever', {
    kind: 'wrap',
    icon: iconFor('Forever'),
    getSlots: n => [n.type === 'Forever' ? n.body : []],
    mapSlots: (n, fn) => (n.type === 'Forever' ? { ...n, body: fn(n.body, 0) } : n),
  });
  registerBlockShape<InstructionDto>('While', {
    kind: 'wrap',
    icon: iconFor('While'),
    getSlots: n => [n.type === 'While' ? n.body : []],
    mapSlots: (n, fn) => (n.type === 'While' ? { ...n, body: fn(n.body, 0) } : n),
  });
}

function iconFor(type: InstructionType) {
  return ICONS[INSTRUCTION_TYPE_ICONS[type]];
}

// ── Field components ─────────────────────────────────────────────────────────
function registerFields() {
  registerBlockField('WhenRan', WhenRanFields);
  registerBlockField('WhenBatteryDischargedTo', WhenBatteryDischargedToFields);
  registerBlockField('WhenBatteryChargedTo', WhenBatteryChargedToFields);
  registerBlockField('WhenTime', WhenTimeFields);
  registerBlockField('WhenPowerPluggedIn', WhenPowerPluggedInFields);
  registerBlockField('WhenPowerUnplugged', WhenPowerUnpluggedFields);
  registerBlockField('Wait', WaitFields);
  registerBlockField('Text', TextFields);
  registerBlockField('Key', KeyFields);
  registerBlockField('Button', ButtonFields);
  registerBlockField('MoveMouse', MoveMouseFields);
  registerBlockField('Scroll', ScrollFields);
  registerBlockField('Command', CommandFields);
  registerBlockField('OpenApp', OpenAppFields);
  registerBlockField('CloseApp', CloseAppFields);
  registerBlockField('Comment', CommentFields);
  registerBlockField('SetVariable', SetVariableFields);
  registerBlockField('ChangeVariable', ChangeVariableFields);
  for (const type of ['AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList'] as const) registerBlockField(type, ListCommandFields);
  registerBlockField('BlockHeader', BlockHeaderFields);
  registerBlockField('CallBlock', CallBlockFields);
  registerBlockField('Return', ReturnFields);
  registerBlockField('If', IfFields);
  registerBlockField('IfElse', IfElseFields);
  registerBlockField('Repeat', RepeatFields);
  registerBlockField('Forever', ForeverFields);
  registerBlockField('While', WhileFields);
  registerBlockField('EscapeLoop', EscapeLoopFields);
  registerBlockField('ContinueLoop', ContinueLoopFields);

  // Palette variants - BlockHeader/CallBlock/Comment are never a fixed
  // sidebar prefab (see components/PaletteCallBlock.vue and
  // components/InstructionSidebar.vue for how CallBlock/BlockHeader are
  // actually offered), so they have no palette field component.
  registerPaletteBlockField('WhenRan', PaletteWhenRanFields);
  registerPaletteBlockField('WhenBatteryDischargedTo', PaletteWhenBatteryDischargedToFields);
  registerPaletteBlockField('WhenBatteryChargedTo', PaletteWhenBatteryChargedToFields);
  registerPaletteBlockField('WhenTime', PaletteWhenTimeFields);
  registerPaletteBlockField('WhenPowerPluggedIn', PaletteWhenPowerPluggedInFields);
  registerPaletteBlockField('WhenPowerUnplugged', PaletteWhenPowerUnpluggedFields);
  registerPaletteBlockField('Wait', PaletteWaitFields);
  registerPaletteBlockField('Text', PaletteTextFields);
  registerPaletteBlockField('Key', PaletteKeyFields);
  registerPaletteBlockField('Button', PaletteButtonFields);
  registerPaletteBlockField('MoveMouse', PaletteMoveMouseFields);
  registerPaletteBlockField('Scroll', PaletteScrollFields);
  registerPaletteBlockField('Command', PaletteCommandFields);
  registerPaletteBlockField('OpenApp', PaletteOpenAppFields);
  registerPaletteBlockField('CloseApp', PaletteCloseAppFields);
  registerPaletteBlockField('SetVariable', PaletteSetVariableFields);
  registerPaletteBlockField('ChangeVariable', PaletteChangeVariableFields);
  for (const type of ['AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList'] as const) registerPaletteBlockField(type, PaletteListCommandFields);
  registerPaletteBlockField('Return', PaletteReturnFields);
  registerPaletteBlockField('If', PaletteIfFields);
  registerPaletteBlockField('IfElse', PaletteIfElseFields);
  registerPaletteBlockField('Repeat', PaletteRepeatFields);
  registerPaletteBlockField('Forever', PaletteForeverFields);
  registerPaletteBlockField('While', PaletteWhileFields);
  registerPaletteBlockField('EscapeLoop', PaletteEscapeLoopFields);
  registerPaletteBlockField('ContinueLoop', PaletteContinueLoopFields);
}

// ── Canvas host ──────────────────────────────────────────────────────────────
function buildCanvasHost(): CanvasHost<InstructionDto> {
  const backend: CanvasBackend<InstructionDto> = {
    addInstruction: tauri.addInstruction,
    addStrand: tauri.addStrand,
    removeStrand: tauri.removeStrand,
    moveStrand: tauri.moveStrand,
    splitStrand: tauri.splitStrand,
    mergeStrand: tauri.mergeStrand,
    deleteBlockDef: tauri.deleteBlock,
    editValueField: tauri.editValueField,
    takeValue: tauri.takeValue,
    putValue: tauri.putValue,
    previewValue: tauri.previewValue,
    // `sourceKind` is only set for a fresh sidebar/header drag (see
    // host.ts's doc comment) - recover the origin block id a `Param:`
    // header oval packed into its `dragKind` (see BlockHeaderFields.vue's
    // `paramDragKind`) so paramIsBool can resolve the resulting floating
    // value exactly instead of guessing.
    createFloatingValue: (x, y, value, sourceKind) =>
      tauri.createFloatingValue(x, y, value as ValueDto, sourceKind?.startsWith('Param:') ? parseParamKind(sourceKind).blockId : null),
    moveFloatingValue: tauri.moveFloatingValue,
    removeFloatingValue: tauri.removeFloatingValue,
    moveComment: tauri.moveComment,
    removeComment: tauri.removeComment,
    editCommentText: tauri.editCommentText,
    setCommentCollapsed: tauri.setCommentCollapsed,
  };

  return {
    getDocument: () => {
      const macro = state.current_macro;
      if (!macro) return null;
      return { id: macro.id, strands: macro.strands, floating_values: macro.floating_values, comments: macro.comments };
    },
    isLocked: () => state.recording_phase.phase === 'Active',
    backend,
    resolveFreshValue: kind =>
      kind.startsWith('Call:') ? paletteCallValueFor(kind.slice('Call:'.length)) : paletteValueFor(kind as ValueKind),
    clonePaletteInstruction: (type, variantId) =>
      type === 'CallBlock' && variantId ? paletteCallInstructionFor(variantId) : clonePaletteInstruction(type as InstructionType),
    isRecordingTarget: strandId => strandId === state.current_macro?.recording_target_strand_id,
    onCanvasContextMenu: e => openCanvasMenu(e),
    onBlockContextMenu: (e, strandId, path) => openBlockMenu(e, strandId, path),
    onVariableContextMenu: (e, name) => openVariableMenu(e, name),
    onPaletteInstructionContextMenu: (e, type, variantId) => openPaletteInstructionMenu(e, type, variantId),
    onPaletteValueContextMenu: (e, kind) => openPaletteValueMenu(e, kind),
    onValueContextMenu: (e, _location, value) => openValueMenu(e, value as ValueDto),
    resolveCallPieces: blockId => findBlockDef(state.current_macro, blockId)?.pieces.map(p => (p.kind === 'Label' ? { kind: 'Label', text: p.text } : { kind: 'Input' })),
    floatingValueColor: floatingValue =>
      floatingValue.value.kind === 'Call'
        ? findBlockDef(state.current_macro, floatingValue.value.block_id)?.color
        : undefined,
    paramIsBool: (location, name) => {
      const pieceIsBool = (blockId: string) => {
        const piece = findBlockDef(state.current_macro, blockId)?.pieces.find(p => p.kind === 'Input' && p.name === name);
        return piece?.kind === 'Input' && piece.value_type === 'Bool';
      };
      // Exact: a Param sitting in an instruction field lives in a specific
      // strand, and a block's body strand always starts with its own
      // BlockHeader - walk straight to that block's own declared type.
      if (location.kind === 'Field') {
        const strand = state.current_macro?.strands.find(s => s.id === location.strand_id);
        const header = strand?.instructions[0];
        if (header?.type === 'BlockHeader') return pieceIsBool(header.block_id);
      }
      // Floating: a standalone FloatingValue has no strand of its own to
      // trace back to, but createFloatingValue records which block a
      // freshly-dropped `Param` reporter came from (see this file's
      // `createFloatingValue` and BlockHeaderFields.vue's `paramDragKind`)
      // - use that exact origin when present.
      if (location.kind === 'Floating') {
        const fv = state.current_macro?.floating_values.find(f => f.id === location.floating_id);
        if (fv?.origin_block_id) return pieceIsBool(fv.origin_block_id);
      }
      // No recorded origin (an older save from before that field existed,
      // or an existing placed param picked back up and re-floated rather
      // than freshly dragged from its header) - fall back to "every custom
      // block with an input named this agrees it's boolean". Exact for the
      // common case (one block currently being edited) and never worse than
      // the plain-capsule default on a genuine cross-block name collision.
      const matches = (state.current_macro?.block_defs ?? []).flatMap(d =>
        d.pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' }> => p.kind === 'Input' && p.name === name),
      );
      return matches.length > 0 && matches.every(p => p.value_type === 'Bool');
    },
    callIsBool: blockId => findBlockDef(state.current_macro, blockId)?.shape === 'ReturnsBool',
    getInvalidText: location => {
      const entry = state.invalid_field_buffers.find(b => locationsEqual(b.location, location));
      if (!entry) return null;
      const trimmed = entry.text.trim();
      let invalid = true;
      if (trimmed !== '') {
        const num = Number(trimmed);
        if (!isNaN(num)) {
          const fieldId = location.kind === 'Field' ? location.field_id : null;
          invalid = fieldId === 'MoveMouseX' || fieldId === 'MoveMouseY' || fieldId === 'ScrollAmount' ? !Number.isInteger(num) : false;
        }
      }
      return { text: entry.text, invalid };
    },
  };
}
