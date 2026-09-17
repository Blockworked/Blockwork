// Backend DTOs. Field names are part of the wire contract - do not rename.
import { defaultArgFor, specForKind } from './valueOps';
import {
  bodyBasePath as bsBodyBasePath,
  fieldLocation as bsFieldLocation,
  newId as bsNewId,
  nextSiblingPath as bsNextSiblingPath,
  pathsEqual as bsPathsEqual,
  regenerateInstructionIds as bsRegenerateInstructionIds,
  resolveInstructionAt as bsResolveInstructionAt,
  resolveInstructionList as bsResolveInstructionList,
  topLevelPath as bsTopLevelPath,
  blankBoolValue as bsBlankBoolValue,
  numberValue as bsNumberValue,
  textValue as bsTextValue,
  isCapType as bsIsCapType,
  isEntryTriggerType as bsIsEntryTriggerType,
  isHeaderType as bsIsHeaderType,
  isWrapType as bsIsWrapType,
} from 'blockstitch';
import type { BlockNode, NodePath, PathStep, ValueLocation } from 'blockstitch';

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';

export type WeekdayDto = 'Sunday' | 'Monday' | 'Tuesday' | 'Wednesday' | 'Thursday' | 'Friday' | 'Saturday';

// One entry in the "Open App" picker list.
export interface AppEntryDto {
  name: string;
  command: string;
  icon: string | null;
}

// A recurring point in local time.
export type TimeScheduleDto =
  | { kind: 'Daily'; hour: number; minute: number }
  | { kind: 'Weekly'; weekday: WeekdayDto; hour: number; minute: number }
  | { kind: 'Monthly'; day: number; hour: number; minute: number }
  | { kind: 'Yearly'; month: number; day: number; hour: number; minute: number };

// Expression tree for a value field.
export type ValueOp =
  | 'Add' | 'Sub' | 'Mul' | 'Div' | 'Mod' | 'Round' | 'Math' | 'Random' | 'Join' | 'NewLine' | 'Tab'
  | 'IndexOf' | 'LastIndexOf' | 'LetterOf' | 'Length' | 'Case'
  | 'Eq' | 'Neq' | 'Gt' | 'Lt' | 'Gte' | 'Lte' | 'And' | 'Or' | 'Not' | 'True' | 'False'
  | 'BatteryPercentage'
  | 'PluggedIn'
  | 'CurrentTime';
export type ValueKind = 'Number' | 'Text' | ValueOp | 'Join3' | `Var:${string}` | `Param:${string}` | `Call:${string}`;
export type ValueDto =
  | { kind: 'Number'; value: number }
  | { kind: 'Text'; value: string }
  | { kind: 'Bool' }
  | { kind: 'Op'; op: ValueOp; args: ValueDto[]; saved: ValueDto }
  | { kind: 'Var'; name: string }
  | { kind: 'Param'; name: string }
  | { kind: 'Call'; block_id: string; args: ValueDto[]; branches: BlockNode[][]; saved: ValueDto };

export function numberValue(value: number): ValueDto {
  return bsNumberValue(value);
}

export function textValue(value: string): ValueDto {
  return bsTextValue(value);
}

export function blankBoolValue(): ValueDto {
  return bsBlankBoolValue();
}

export function parseParamKind(kind: string): { blockId: string | null; name: string } {
  const rest = kind.slice('Param:'.length);
  const sep = rest.indexOf(':');
  return sep === -1 ? { blockId: null, name: rest } : { blockId: rest.slice(0, sep), name: rest.slice(sep + 1) };
}

export function defaultValueForKind(kind: ValueKind): ValueDto {
  if (kind === 'Number') return { kind: 'Number', value: 0 };
  if (kind === 'Text') return { kind: 'Text', value: '' };
  if (kind.startsWith('Var:')) return { kind: 'Var', name: kind.slice('Var:'.length) };
  if (kind.startsWith('Param:')) return { kind: 'Param', name: parseParamKind(kind).name };
  // Normally blockDefs.ts's paletteCallValueFor handles `Call:` (it needs the
  // block's input count); this is just a safe zero-arg fallback.
  if (kind.startsWith('Call:')) return { kind: 'Call', block_id: kind.slice('Call:'.length), args: [], branches: [], saved: numberValue(0) };
  const spec = specForKind(kind);
  if (!spec) throw new Error(`Unknown value kind: ${kind}`);
  return { kind: 'Op', op: spec.op, args: Array.from({ length: spec.arity }, (_, i) => defaultArgFor(spec, i)), saved: numberValue(0) };
}

// ── Block-graph addressing (re-exported from blockstitch) ─────────────────────
export type { PathStep };
export type InstrPath = NodePath;

export function topLevelPath(index: number): InstrPath {
  return bsTopLevelPath(index);
}

export function resolveInstructionList(strand: StrandDto | null | undefined, basePath: PathStep[]): InstructionDto[] {
  return bsResolveInstructionList(strand ?? undefined, basePath);
}

export function resolveInstructionAt(strand: StrandDto | null | undefined, path: InstrPath): InstructionDto | null {
  return bsResolveInstructionAt(strand ?? undefined, path);
}

export function nextSiblingPath(path: InstrPath): InstrPath {
  return bsNextSiblingPath(path);
}

export function bodyBasePath(path: InstrPath, slot: number): InstrPath {
  return bsBodyBasePath(path, slot);
}

export type ValueLocationDto = ValueLocation;

export interface FloatingValueDto {
  id: string;
  x: number;
  y: number;
  value: ValueDto;
  origin_block_id: string | null;
}

export interface CommentDto {
  id: string;
  x: number;
  y: number;
  text: string;
  collapsed: boolean;
  attached_to: string | null;
}

export function fieldLocation(strandId: string, instrPath: InstrPath, fieldId: string): ValueLocationDto {
  return bsFieldLocation(strandId, instrPath, fieldId);
}

export type InstructionDto = { id: string } & (
  | { type: 'Wait'; duration: ValueDto }
  | { type: 'Text'; text: ValueDto }
  | { type: 'Key'; key: string; direction: KeyDirection }
  | { type: 'Button'; button: MouseButton; direction: KeyDirection }
  | { type: 'MoveMouse'; x: ValueDto; y: ValueDto; coordinate: Coordinate }
  | { type: 'Scroll'; amount: ValueDto; axis: ScrollAxis }
  | { type: 'Command'; command: string }
  | { type: 'Comment'; comment: string }
  | { type: 'WhenRan' }
  | { type: 'WhenBatteryDischargedTo'; threshold: ValueDto }
  | { type: 'WhenBatteryChargedTo'; threshold: ValueDto }
  | { type: 'WhenTime'; schedule: TimeScheduleDto }
  | { type: 'WhenPowerPluggedIn' }
  | { type: 'WhenPowerUnplugged' }
  | { type: 'OpenApp'; command: string; name: string; icon: string | null }
  | { type: 'CloseApp'; command: string; name: string; icon: string | null }
  | { type: 'SetVariable'; name: string; value: ValueDto }
  | { type: 'ChangeVariable'; name: string; value: ValueDto }
  | { type: 'BlockHeader'; block_id: string }
  | { type: 'CallBlock'; block_id: string; args: ValueDto[] }
  | { type: 'Return'; value: ValueDto }
  | { type: 'If'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'IfElse'; condition: ValueDto; then_body: InstructionDto[]; else_body: InstructionDto[] }
  | { type: 'Repeat'; count: ValueDto; body: InstructionDto[] }
  | { type: 'Forever'; body: InstructionDto[] }
  | { type: 'While'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'EscapeLoop' }
  | { type: 'ContinueLoop' }
);

export type InstructionType = InstructionDto['type'];

export function newId(): string {
  return bsNewId();
}

export function defaultInstruction(type: InstructionType): InstructionDto {
  const id = newId();
  switch (type) {
    case 'WhenRan': return { id, type: 'WhenRan' };
    case 'WhenBatteryDischargedTo': return { id, type: 'WhenBatteryDischargedTo', threshold: numberValue(20) };
    case 'WhenBatteryChargedTo': return { id, type: 'WhenBatteryChargedTo', threshold: numberValue(100) };
    case 'WhenTime': return { id, type: 'WhenTime', schedule: { kind: 'Daily', hour: 9, minute: 0 } };
    case 'WhenPowerPluggedIn': return { id, type: 'WhenPowerPluggedIn' };
    case 'WhenPowerUnplugged': return { id, type: 'WhenPowerUnplugged' };
    case 'OpenApp': return { id, type: 'OpenApp', command: '', name: '', icon: null };
    case 'CloseApp': return { id, type: 'CloseApp', command: '', name: '', icon: null };
    case 'Wait': return { id, type: 'Wait', duration: numberValue(1000) };
    case 'Text': return { id, type: 'Text', text: textValue('text') };
    case 'Key': return { id, type: 'Key', key: 'a', direction: 'Click' };
    case 'Button': return { id, type: 'Button', button: 'Left', direction: 'Click' };
    case 'MoveMouse': return { id, type: 'MoveMouse', x: numberValue(0), y: numberValue(0), coordinate: 'Relative' };
    case 'Scroll': return { id, type: 'Scroll', amount: numberValue(4), axis: 'Vertical' };
    case 'Command': return { id, type: 'Command', command: '' };
    case 'Comment': return { id, type: 'Comment', comment: '' };
    case 'SetVariable': return { id, type: 'SetVariable', name: '', value: numberValue(0) };
    case 'ChangeVariable': return { id, type: 'ChangeVariable', name: '', value: numberValue(0) };
    case 'BlockHeader': return { id, type: 'BlockHeader', block_id: '' };
    case 'CallBlock': return { id, type: 'CallBlock', block_id: '', args: [] };
    case 'Return': return { id, type: 'Return', value: numberValue(0) };
    case 'If': return { id, type: 'If', condition: blankBoolValue(), body: [] };
    case 'IfElse': return { id, type: 'IfElse', condition: blankBoolValue(), then_body: [], else_body: [] };
    case 'Repeat': return { id, type: 'Repeat', count: numberValue(10), body: [] };
    case 'Forever': return { id, type: 'Forever', body: [] };
    case 'While': return { id, type: 'While', condition: blankBoolValue(), body: [] };
    case 'EscapeLoop': return { id, type: 'EscapeLoop' };
    case 'ContinueLoop': return { id, type: 'ContinueLoop' };
    default: return { id, type: 'Comment', comment: '' };
  }
}

export function regenerateInstructionIds(ins: InstructionDto): InstructionDto {
  return bsRegenerateInstructionIds(ins);
}

export function isHeaderType(type: InstructionDto['type']): boolean {
  return bsIsHeaderType(type);
}

export function isEntryTriggerType(type: InstructionDto['type']): boolean {
  return bsIsEntryTriggerType(type);
}

export function isCapType(type: InstructionDto['type']): boolean {
  return bsIsCapType(type);
}

export function isWrapType(type: InstructionDto['type']): boolean {
  return bsIsWrapType(type);
}

export function hasElseSlot(type: InstructionDto['type']): boolean {
  return type === 'IfElse';
}

export interface StrandDto {
  id: string;
  x: number;
  y: number;
  instructions: InstructionDto[];
}

export interface MacroDto {
  id: string;
  name: string;
  description: string;
  strands: StrandDto[];
  recording_target_strand_id: string | null;
  speed_multiplier: number;
  floating_values: FloatingValueDto[];
  comments: CommentDto[];
  /** Declared variable names in insertion order. */
  variables: string[];
  block_defs: BlockDefDto[];
  settings: MacroSettingsDto;
}

// Per-macro settings.
export interface MacroSettingsDto {
  /** When true, this macro's event strands are watched even while another macro is selected. */
  always_listen: boolean;
}

export function defaultMacroSettings(): MacroSettingsDto {
  return { always_listen: false };
}

export interface CustomMacroSettingDto {
  key: string;
  label: string;
  enabled: boolean;
}

export interface ImportPromptDto {
  needs_command_warning: boolean;
  custom_settings: CustomMacroSettingDto[];
}

export function sortedVariableNames(macro: MacroDto | null | undefined): string[] {
  return [...(macro?.variables ?? [])].sort((a, b) => a.localeCompare(b));
}

export type InputValueType = 'Any' | 'Bool';

export type BlockPieceDto =
  | { kind: 'Label'; id: string; text: string }
  | { kind: 'Input'; id: string; name: string; value_type: InputValueType };

export type BlockShapeDto = 'Normal' | 'Ending' | 'ReturnsValue' | 'ReturnsBool';

export function blockShapeReturnsValue(shape: BlockShapeDto): boolean {
  return shape === 'ReturnsValue' || shape === 'ReturnsBool';
}

export interface BlockDefDto {
  id: string;
  pieces: BlockPieceDto[];
  shape: BlockShapeDto;
  /** Hex accent used by this custom block's icon and hover outline. */
  color: string;
}

export function blockInputPieces(def: BlockDefDto): Extract<BlockPieceDto, { kind: 'Input' }>[] {
  return def.pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' }> => p.kind === 'Input');
}

export function blockInputNames(def: BlockDefDto): string[] {
  return blockInputPieces(def).map(p => p.name);
}

export function findBlockDef(macro: MacroDto | null | undefined, blockId: string): BlockDefDto | undefined {
  return macro?.block_defs.find(b => b.id === blockId);
}

export interface KeyCaptureDto {
  kind: 'Strand' | 'Standalone';
  strand_id: string | null;
  index: InstrPath | null;
}

/** Structural equality for two InstrPaths - used wherever a path is
 * compared instead of a bare index (e.g. "is this the row being captured"). */
export function pathsEqual(a: InstrPath | null | undefined, b: InstrPath | null | undefined): boolean {
  return bsPathsEqual(a, b);
}

export type HotkeyActionDto =
  | { type: 'RunMacro' }
  | { type: 'StopLoop' }
  | { type: 'NextMacro' }
  | { type: 'PrevMacro' }
  | { type: 'ToggleLoop' }
  | { type: 'StartRecordingImmediate' }
  | { type: 'StopRecording' }
  | { type: 'Undo' }
  | { type: 'Redo' }
  | { type: 'RunSpecificMacro'; macro_id: string };

export interface HotkeyBindingDto {
  binding_index: number;
  action: HotkeyActionDto;
  combo_display: string;
  macro_name: string | null;
}

export interface NamedHotkeyDefaultDto {
  action: HotkeyActionDto;
  combo_display: string | null;
}

export interface ComboCaptureDto {
  kind: 'Named' | 'Pending';
  action: HotkeyActionDto | null;
}

export interface PendingMacroHotkeyDto {
  macro_index: number | null;
  combo_display: string | null;
}

export interface InvalidFieldDto {
  location: ValueLocationDto;
  text: string;
}

export type RecordingPhaseName = 'Idle' | 'Countdown' | 'Active';

export interface RecordingPhaseDto {
  phase: RecordingPhaseName;
  countdown: number | null;
}

export type UpdateCheckStateName = 'Idle' | 'Checking' | 'UpToDate' | 'UpdateAvailable' | 'Applying' | 'Error';

export interface UpdateCheckStateDto {
  state: UpdateCheckStateName;
  version: string | null;
  error: string | null;
}

export type PageName = 'Main' | 'Settings';

export interface StateDto {
  macro_names: string[];
  macro_selected: number | null;
  current_macro: MacroDto | null;
  macros_data: MacroDto[];
  loop_mode_enabled: boolean;
  global_speed_multiplier: number;
  is_looping: boolean;
  ipc_active_port: number | null;
  ipc_auto_start: boolean;
  close_to_tray: boolean;
  confirm_clear_instructions: boolean;
  confirm_clear_instructions_remaining_secs: number;
  key_capture: KeyCaptureDto | null;
  standalone_key: string | null;
  can_undo: boolean;
  can_redo: boolean;
  recording_phase: RecordingPhaseDto;
  record_mouse_relative: boolean;
  record_mouse_movement: boolean;
  page: PageName;
  combo_capture: ComboCaptureDto | null;
  hotkey_bindings: HotkeyBindingDto[];
  named_hotkey_defaults: NamedHotkeyDefaultDto[];
  pending_macro_hotkey: PendingMacroHotkeyDto | null;
  invalid_field_buffers: InvalidFieldDto[];
  ipc_port_text: string;
  ipc_port_invalid: boolean;
  emulator_available: boolean;
  grab_available: boolean;
  razer_permission_warning: boolean;
  update_check_state: UpdateCheckStateDto;
}

export function emptyState(): StateDto {
  return {
    macro_names: [],
    macro_selected: null,
    current_macro: null,
    macros_data: [],
    loop_mode_enabled: false,
    global_speed_multiplier: 1.0,
    is_looping: false,
    ipc_active_port: null,
    ipc_auto_start: false,
    close_to_tray: false,
    confirm_clear_instructions: false,
    confirm_clear_instructions_remaining_secs: 0,
    key_capture: null,
    standalone_key: null,
    can_undo: false,
    can_redo: false,
    recording_phase: { phase: 'Idle', countdown: null },
    record_mouse_relative: true,
    record_mouse_movement: false,
    page: 'Main',
    combo_capture: null,
    hotkey_bindings: [],
    named_hotkey_defaults: [],
    pending_macro_hotkey: null,
    invalid_field_buffers: [],
    ipc_port_text: '',
    ipc_port_invalid: false,
    emulator_available: true,
    grab_available: true,
    razer_permission_warning: false,
    update_check_state: { state: 'Idle', version: null, error: null },
  };
}
