// Mirrors src-tauri/src/state.rs's StateDto and friends exactly (serde field
// names/shapes are a fixed contract with the Rust backend — do not rename).
//
// The generic block-graph addressing/navigation helpers (path types,
// resolveInstructionList, isHeaderType, etc.) now live in blockstitch — this
// file re-exports them under their original names so every existing call
// site keeps working unchanged, and keeps only Blockwork's own concrete DTOs.
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
import type { NodePath, PathStep, ValueLocation } from 'blockstitch';

export type KeyDirection = 'Click' | 'Press' | 'Release';
export type MouseButton = 'Left' | 'Right' | 'Middle' | 'Side' | 'Extra';
export type Coordinate = 'Absolute' | 'Relative';
export type ScrollAxis = 'Vertical' | 'Horizontal';
// Which window an accessibility-tree locator searches — mirrors blockwork-core's
// `ElementScope`. 'Focused Window' ignores app/title_contains entirely (see
// ClickElementFields.vue/SetElementValueFields.vue, which hide those fields
// for this choice) in favor of whichever window currently has OS focus.
// 'Specific App' shows only the app field (title_contains ignored); 'Specific
// Window' shows only the title_contains field (app ignored).
export type ElementScope = 'Focused Window' | 'Specific App' | 'Specific Window';

export type WeekdayDto = 'Sunday' | 'Monday' | 'Tuesday' | 'Wednesday' | 'Thursday' | 'Friday' | 'Saturday';

// One entry in the "Open App" picker's list — mirrors src-tauri/src/state.rs's
// AppEntryDto. `command` is opaque (platform-specific launch string) and gets
// stored as-is on the `OpenApp` instruction if picked; `icon`, when present,
// is a `data:` URI ready to drop straight into an <img src>.
export interface AppEntryDto {
  name: string;
  command: string;
  icon: string | null;
}

// A recurring point in local time — mirrors blockwork-core's `TimeSchedule`
// (input/schedule.rs), reused as-is on the wire (same shape as `Op` being
// reused directly in `ValueDto`, not mirrored through a *Dto type). `hour`/
// `minute` are always 24-hour; see timeSchedule.ts for the 12h/24h display
// split and per-kind option lists.
export type TimeScheduleDto =
  | { kind: 'Daily'; hour: number; minute: number }
  | { kind: 'Weekly'; weekday: WeekdayDto; hour: number; minute: number }
  | { kind: 'Monthly'; day: number; hour: number; minute: number }
  | { kind: 'Yearly'; month: number; day: number; hour: number; minute: number };

// A small recursive expression tree backing a value field — a number, text,
// or an operator applied to nested `args` (e.g. `(5) + (3)`). Mirrors
// src-tauri/src/state.rs's ValueDto, and satisfies blockstitch's generic
// `ValueNode` shape structurally (see values/valueNode.ts there).
export type ValueOp =
  | 'Add' | 'Sub' | 'Mul' | 'Div' | 'Mod' | 'Round' | 'Math' | 'Random' | 'Join' | 'NewLine' | 'Tab'
  | 'IndexOf' | 'LastIndexOf' | 'LetterOf' | 'Length' | 'Case'
  // Boolean: comparisons, logic, and the two standalone true/false literals
  // (separate blocks, not a toggle — see valueOps.ts's OPERATOR_KINDS).
  | 'Eq' | 'Neq' | 'Gt' | 'Lt' | 'Gte' | 'Lte' | 'And' | 'Or' | 'Not' | 'True' | 'False'
  // Zero-arity — the system's current battery charge, 0-100.
  | 'BatteryPercentage'
  // Zero-arity boolean — whether the system is currently on external power
  // (always true with no battery/UPS present).
  | 'PluggedIn'
  // Zero-arity text — the app owning the currently focused window (e.g.
  // "firefox"), or the currently focused window's title, respectively.
  | 'FocusedApplication' | 'FocusedWindowTitle'
  // One arg (app name, case-insensitive substring match): whether a
  // matching process is running. Two args (app filter, title-substring
  // filter, either blank matching anything): whether any window matches both.
  | 'ApplicationIsRunning' | 'WindowExists'
  // Four args (app filter, title-substring filter, element-name filter,
  // element-role filter — e.g. "push button" — each blank matching
  // anything): best-effort text content / whether any element matches /
  // whether a matching element exists and is enabled, respectively.
  | 'ElementText' | 'ElementExists' | 'ElementEnabled'
  // Zero-arity — the live clipboard contents as text.
  | 'ClipboardText'
  // Zero-arity booleans — whether the clipboard currently holds image data
  // or a file list, respectively.
  | 'ClipboardHasImage' | 'ClipboardHasFiles'
  // One fixed-dropdown arg (year/month/date/day of week/hour/minute/second)
  // — always numeric, see timeSchedule.ts's CURRENT_TIME_OPTIONS.
  | 'CurrentTime'
  | 'ListItem' | 'ListItemNumber' | 'ListAmount' | 'ListLength' | 'ListContains' | 'ListItemExists' | 'ListIsEmpty';
// Join/Join3 both emit op 'Join' (only args.length differs, no 'Join3' on the
// wire). `Var:<name>`/`Param:<name>` are per-variable/per-input identifiers,
// not fixed operators. `Call:<blockId>` (a "My Blocks" reporter) has dynamic
// arity, so it isn't handled by defaultValueForKind/paletteValueFor — see
// blockDefs.ts's paletteCallValueFor.
export type ValueKind = 'Number' | 'Text' | ValueOp | 'Join3' | `Var:${string}` | `Param:${string}` | `Call:${string}`;
// `saved` is the value the operator displaced when it took over the slot —
// carried along so the backend can hand it back if this block is dragged out.
export type ValueDto =
  | { kind: 'Number'; value: number }
  | { kind: 'Text'; value: string }
  // Bare boolean leaf, no value of its own — the "nothing plugged in here"
  // state of a boolean slot (an If's condition, an And/Or/Not operand).
  // Renders as a blank hexagon (blockstitch's ValueBlock.vue) and evaluates as
  // false.
  | { kind: 'Bool' }
  | { kind: 'Op'; op: ValueOp; args: ValueDto[]; saved: ValueDto }
  | { kind: 'Var'; name: string }
  | { kind: 'Param'; name: string }
  | { kind: 'Call'; block_id: string; args: ValueDto[]; branches: InstructionDto[][]; saved: ValueDto };

/** Literal-only list item. Lists never store expressions, booleans, or refs. */
export type ListItemDto = { kind: 'Number'; value: number } | { kind: 'Text'; value: string };

export interface ListDto {
  name: string;
  items: ListItemDto[];
  editor_visible: boolean;
  editor_x: number;
  editor_y: number;
}

export function numberValue(value: number): ValueDto {
  return bsNumberValue(value);
}

export function textValue(value: string): ValueDto {
  return bsTextValue(value);
}

// Fresh boolean-typed default — blank, same spirit as
// `numberValue(0)`/`textValue('')`, not a pre-filled "false".
export function blankBoolValue(): ValueDto {
  return bsBlankBoolValue();
}

// A `Param:` kind carries a plain `<name>` for display (see
// BlockHeaderFields.vue's `paramKind`, and PaletteValueBlock.vue's `kind`
// prop, which slices it straight off) or, only as blockstitch's `dragKind`
// override for that same header oval, `<blockId>:<name>` — the one extra
// bit of context needed to recover which block a dropped-out `Param`
// reporter came from once it's parked as a floating value with no strand of
// its own to trace back to (see BlockHeaderFields.vue's `paramDragKind` and
// blockstitchSetup.ts's `createFloatingValue`/`paramIsBool`). `blockId`
// never contains ':' (a UUID), so splitting on the first one is
// unambiguous even if `name` itself does.
export function parseParamKind(kind: string): { blockId: string | null; name: string } {
  const rest = kind.slice('Param:'.length);
  const sep = rest.indexOf(':');
  return sep === -1 ? { blockId: null, name: rest } : { blockId: rest.slice(0, sep), name: rest.slice(sep + 1) };
}

// Fresh default tree for a value block dragged off the sidebar palette —
// mirrors src-tauri/src/commands.rs's apply_value_kind, looked up from
// valueOps.ts's registry so a new operator never needs a new case here.
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

/** Resolves `basePath` against a strand to the (possibly nested) instruction
 * list it addresses — a strand's own top-level list for `basePath: []`, or
 * an If/IfElse's body for anything longer. Shared by canvasDrag.ts (DOM-less
 * placement checks), clipboard.ts, and ContextMenu.vue. */
export function resolveInstructionList(strand: StrandDto | null | undefined, basePath: PathStep[]): InstructionDto[] {
  return bsResolveInstructionList(strand ?? undefined, basePath);
}

/** The single instruction `path` addresses, or `null` if any step along the
 * way doesn't resolve (e.g. stale state mid-edit). */
export function resolveInstructionAt(strand: StrandDto | null | undefined, path: InstrPath): InstructionDto | null {
  return bsResolveInstructionAt(strand ?? undefined, path);
}

/** The path of the instruction immediately after `path`, in the same body
 * list — e.g. for "insert a duplicate right after this block." */
export function nextSiblingPath(path: InstrPath): InstrPath {
  return bsNextSiblingPath(path);
}

/** The base path for an If/IfElse instruction's own nested body — `path` is
 * that instruction's own address (its last step has no `slot`, since
 * nothing follows it yet); this stamps `slot` onto that last step, so an
 * InstructionList rendering the body can append its own children's indices
 * after it. `slot` is 0 for an If's body or IfElse's `then_body`, 1 for
 * IfElse's `else_body`. */
export function bodyBasePath(path: InstrPath, slot: number): InstrPath {
  return bsBodyBasePath(path, slot);
}

// Addresses a single Value node: inside an instruction field (Field) or a
// floating canvas block (Floating), at `path` within that root. Mirrors
// src-tauri/src/state.rs's ValueLocation/ValueLocationDto, and is structurally
// identical to blockstitch's generic `ValueLocation`.
export type ValueLocationDto = ValueLocation;

export interface FloatingValueDto {
  id: string;
  x: number;
  y: number;
  value: ValueDto;
  origin_block_id: string | null;
}

// A floating, collapsible note — freestanding (`attached_to: null`, `x`/`y`
// an absolute canvas position, same convention as FloatingValueDto) or
// pinned to an instruction (`attached_to` its id, `x`/`y` an *offset* from
// that instruction's on-screen position instead — canvasDrag.ts's
// positionCanvas resolves the absolute position at render time, since only
// the frontend's own DOM measurement knows where a given instruction row
// actually renders).
export interface CommentDto {
  id: string;
  x: number;
  y: number;
  text: string;
  collapsed: boolean;
  attached_to: string | null;
}

// Root-of-field location for the field components under ui/src/components/fields/.
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
  // Legacy-only: an old inline comment instruction, or the placeholder shown
  // for a raw/unmapped recorded keycode. No longer user-creatable — see
  // Comment (the floating-note type) and store.ts's comments list instead.
  | { type: 'Comment'; comment: string }
  | { type: 'WhenRan' }
  | { type: 'WhenBatteryDischargedTo'; threshold: ValueDto }
  | { type: 'WhenBatteryChargedTo'; threshold: ValueDto }
  | { type: 'WhenTime'; schedule: TimeScheduleDto }
  | { type: 'WhenPowerPluggedIn' }
  | { type: 'WhenPowerUnplugged' }
  | { type: 'WhenClipboardChanged' }
  | { type: 'OpenApp'; command: string; name: string; icon: string | null }
  | { type: 'CloseApp'; command: string; name: string; icon: string | null }
  | { type: 'SetVariable'; name: string; value: ValueDto }
  | { type: 'ChangeVariable'; name: string; value: ValueDto }
  | { type: 'SetClipboard'; value: ValueDto }
  | { type: 'AddToList'; value: ValueDto; name: string }
  | { type: 'DeleteOfList'; index: ValueDto; name: string }
  | { type: 'DeleteAllOfList'; name: string }
  | { type: 'ShiftList'; name: string; amount: ValueDto }
  | { type: 'InsertIntoList'; value: ValueDto; index: ValueDto; name: string }
  | { type: 'ReplaceItemOfList'; index: ValueDto; name: string; value: ValueDto }
  | { type: 'ReverseList'; name: string }
  | { type: 'BlockHeader'; block_id: string }
  | { type: 'CallBlock'; block_id: string; args: ValueDto[] }
  | { type: 'BranchCallBlock'; block_id: string; args: ValueDto[]; branches: InstructionDto[][] }
  | { type: 'RunBranch'; name: string }
  | { type: 'Return'; value: ValueDto }
  | { type: 'If'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'IfElse'; condition: ValueDto; then_body: InstructionDto[]; else_body: InstructionDto[] }
  | { type: 'Repeat'; count: ValueDto; body: InstructionDto[] }
  | { type: 'Forever'; body: InstructionDto[] }
  | { type: 'While'; condition: ValueDto; body: InstructionDto[] }
  | { type: 'EscapeLoop' }
  | { type: 'ContinueLoop' }
  | { type: 'WaitUntil'; condition: ValueDto }
  | { type: 'FocusWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto }
  | { type: 'MinimizeWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto }
  | { type: 'MaximizeWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto }
  | { type: 'MoveWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto; x: ValueDto; y: ValueDto }
  | { type: 'ClickElement'; scope: ElementScope; app: ValueDto; title_contains: ValueDto; name: ValueDto; role: ValueDto }
  | { type: 'SetElementValue'; scope: ElementScope; app: ValueDto; title_contains: ValueDto; name: ValueDto; role: ValueDto; value: ValueDto }
  | { type: 'SendKeyToWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto; key: string; modifiers: number }
  | { type: 'TypeTextInWindow'; scope: ElementScope; app: ValueDto; title_contains: ValueDto; text: ValueDto }
);

export type InstructionType = InstructionDto['type'];

// A fresh id for a brand-new instruction/comment — same fallback pattern as
// MakeBlockDialog.vue's piece ids, for a webview without `crypto.randomUUID`.
export function newId(): string {
  return bsNewId();
}

// Fresh instruction for a given type — mirrors src-tauri/src/commands.rs's
// defaults. Seeds both a brand-new sidebar drop and a prefab's editable state.
export function defaultInstruction(type: InstructionType): InstructionDto {
  const id = newId();
  switch (type) {
    case 'WhenRan': return { id, type: 'WhenRan' };
    case 'WhenBatteryDischargedTo': return { id, type: 'WhenBatteryDischargedTo', threshold: numberValue(20) };
    case 'WhenBatteryChargedTo': return { id, type: 'WhenBatteryChargedTo', threshold: numberValue(100) };
    case 'WhenTime': return { id, type: 'WhenTime', schedule: { kind: 'Daily', hour: 9, minute: 0 } };
    case 'WhenPowerPluggedIn': return { id, type: 'WhenPowerPluggedIn' };
    case 'WhenPowerUnplugged': return { id, type: 'WhenPowerUnplugged' };
    case 'WhenClipboardChanged': return { id, type: 'WhenClipboardChanged' };
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
    case 'SetClipboard': return { id, type: 'SetClipboard', value: textValue('text') };
    case 'AddToList': return { id, type: 'AddToList', name: '', value: textValue('thing') };
    case 'DeleteOfList': return { id, type: 'DeleteOfList', name: '', index: numberValue(1) };
    case 'DeleteAllOfList': return { id, type: 'DeleteAllOfList', name: '' };
    case 'ShiftList': return { id, type: 'ShiftList', name: '', amount: numberValue(1) };
    case 'InsertIntoList': return { id, type: 'InsertIntoList', name: '', value: textValue('thing'), index: numberValue(1) };
    case 'ReplaceItemOfList': return { id, type: 'ReplaceItemOfList', name: '', index: numberValue(1), value: textValue('thing') };
    case 'ReverseList': return { id, type: 'ReverseList', name: '' };
    case 'BlockHeader': return { id, type: 'BlockHeader', block_id: '' };
    case 'CallBlock': return { id, type: 'CallBlock', block_id: '', args: [] };
    case 'BranchCallBlock': return { id, type: 'BranchCallBlock', block_id: '', args: [], branches: [] };
    case 'RunBranch': return { id, type: 'RunBranch', name: '' };
    case 'Return': return { id, type: 'Return', value: numberValue(0) };
    case 'If': return { id, type: 'If', condition: blankBoolValue(), body: [] };
    case 'IfElse': return { id, type: 'IfElse', condition: blankBoolValue(), then_body: [], else_body: [] };
    case 'Repeat': return { id, type: 'Repeat', count: numberValue(10), body: [] };
    case 'Forever': return { id, type: 'Forever', body: [] };
    case 'While': return { id, type: 'While', condition: blankBoolValue(), body: [] };
    case 'EscapeLoop': return { id, type: 'EscapeLoop' };
    case 'ContinueLoop': return { id, type: 'ContinueLoop' };
    case 'WaitUntil': return { id, type: 'WaitUntil', condition: blankBoolValue() };
    case 'FocusWindow': return { id, type: 'FocusWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue('') };
    case 'MinimizeWindow': return { id, type: 'MinimizeWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue('') };
    case 'MaximizeWindow': return { id, type: 'MaximizeWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue('') };
    case 'MoveWindow': return { id, type: 'MoveWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue(''), x: numberValue(0), y: numberValue(0) };
    case 'ClickElement': return { id, type: 'ClickElement', scope: 'Focused Window', app: textValue(''), title_contains: textValue(''), name: textValue(''), role: textValue('') };
    case 'SetElementValue': return { id, type: 'SetElementValue', scope: 'Focused Window', app: textValue(''), title_contains: textValue(''), name: textValue(''), role: textValue(''), value: textValue('') };
    case 'SendKeyToWindow': return { id, type: 'SendKeyToWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue(''), key: 'Enter', modifiers: 0 };
    case 'TypeTextInWindow': return { id, type: 'TypeTextInWindow', scope: 'Focused Window', app: textValue(''), title_contains: textValue(''), text: textValue('text') };
    default: return { id, type: 'Comment', comment: '' };
  }
}

// Deep-clones an instruction (and, for a wrap block, everything nested in its
// body/then_body/else_body) with a fresh id at every level — duplicating or
// pasting an existing instruction must never leave two live instructions
// sharing an id, since Comment.attached_to and the connecting-line rendering
// resolve "the instruction" by id alone. Delegates to blockstitch's generic
// version, which walks bodies via the shapes registered in
// blockstitchSetup.ts.
export function regenerateInstructionIds(ins: InstructionDto): InstructionDto {
  return bsRegenerateInstructionIds(ins);
}

export function isHeaderType(type: InstructionDto['type']): boolean {
  return bsIsHeaderType(type);
}

// The subset of header types that are "entry point" triggers (WhenRan and
// every When-condition block) rather than a custom block's own definition
// header (BlockHeader renders its own params instead of a fixed label, so it
// gets its own look — see BlockHeaderFields.vue — not this quiet accent
// tint). Drives `.instruction-row-when-ran`'s styling.
export function isEntryTriggerType(type: InstructionDto['type']): boolean {
  return bsIsEntryTriggerType(type);
}

// "Cap" blocks (the mirror of header blocks) never have anything stacked
// below them — `Return` ends the strand's control flow, and `EscapeLoop`/
// `ContinueLoop` jump straight to the enclosing loop's boundary — so they
// render with a flat bottom edge instead of a connector tab.
export function isCapType(type: InstructionDto['type']): boolean {
  return bsIsCapType(type);
}

// "Wrap"/C-blocks encase a nested body (or two, for If-Else) between their
// own top notch and bottom tab — unlike header/cap, they keep both, since
// they snap above/below like any ordinary block.
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
  /** Floating/attached notes — see CommentDto. */
  comments: CommentDto[];
  /** Declared variable names only — no current-value "watcher" UI. Insertion
   * order; use `sortedVariableNames` for display. */
  variables: string[];
  lists: ListDto[];
  /** User-defined custom blocks ("My Blocks") — see `BlockDefDto`. */
  block_defs: BlockDefDto[];
  /** Settings edited from the "Macro Settings" popup — see `MacroSettingsDto`. */
  settings: MacroSettingsDto;
}

// Mirrors src-tauri/src/state.rs's MacroSettingsDto — per-macro settings
// edited from the "Macro Settings" popup next to the macro dropdown, and
// included in macro export/import like everything else in MacroDto.
export interface MacroSettingsDto {
  /** When true, this macro's When-Battery/-Time/-Power strands are watched
   * by the background watchers even while a different macro is selected. */
  always_listen: boolean;
}

export function defaultMacroSettings(): MacroSettingsDto {
  return { always_listen: false };
}

/** One non-default `MacroSettingsDto` field an import wants confirmed —
 * mirrors src-tauri/src/state.rs's CustomMacroSettingDto. */
export interface CustomMacroSettingDto {
  key: string;
  label: string;
  enabled: boolean;
}

/** What `importMacro` needs the user to resolve before the staged import can
 * be committed — mirrors src-tauri/src/state.rs's ImportPromptDto. `null`
 * (Rust's `None`) means the import needed no confirmation and already
 * committed. */
export interface ImportPromptDto {
  needs_command_warning: boolean;
  custom_settings: CustomMacroSettingDto[];
}

/** Alphabetical variable names for a macro — shared by the sidebar reporter
 * list and every Set/Change dropdown. */
export function sortedVariableNames(macro: MacroDto | null | undefined): string[] {
  return [...(macro?.variables ?? [])].sort((a, b) => a.localeCompare(b));
}

export function sortedListNames(macro: MacroDto | null | undefined): string[] {
  return [...(macro?.lists ?? [])].map(list => list.name).sort((a, b) => a.localeCompare(b));
}

// What kind of value an input slot expects — 'Any' (number-or-text, the
// long-standing default) or 'Bool' (renders as a hexagon, blank-defaults to
// `{ kind: 'Bool' }` instead of `0`, see blockDefs.ts). Mirrors
// blockwork-core/src/macros/mod.rs's InputValueType.
export type InputValueType = 'Any' | 'Bool';

// One piece of a custom block's prototype, in declaration order — mirrors
// src-tauri/src/macros/mod.rs's BlockPiece. `id` is a stable identifier
// (not the name) so the backend can tell "renamed" apart from "removed +
// added" when reconciling call sites' args on edit_block.
export type BlockPieceDto =
  | { kind: 'Label'; id: string; text: string }
  | { kind: 'Input'; id: string; name: string; value_type: InputValueType }
  | { kind: 'Branch'; id: string; name: string };

// What a custom block's own call site looks like — 'Normal' (a plain
// stackable instruction), 'Ending' (stackable, but nothing can be placed
// below it — same shape family as the built-in Return/EscapeLoop/
// ContinueLoop), 'ReturnsValue' (a number-or-text reporter, an oval — the
// long-standing `returns_value: true`), or 'ReturnsBool' (a boolean
// reporter, a hexagon). Mirrors blockwork-core/src/macros/mod.rs's
// BlockShape.
export type BlockShapeDto = 'Normal' | 'Ending' | 'ReturnsValue' | 'ReturnsBool';

/** True for either reporter shape — mirrors BlockShape::returns_value(). */
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

/** A block's declared input pieces, in prototype order — the positional key
 * `CallBlock`/`Value.Call`'s `args` line up against. */
export function blockInputPieces(def: BlockDefDto): Extract<BlockPieceDto, { kind: 'Input' }>[] {
  return def.pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Input' }> => p.kind === 'Input');
}

export function blockBranchPieces(def: BlockDefDto): Extract<BlockPieceDto, { kind: 'Branch' }>[] {
  return def.pieces.filter((p): p is Extract<BlockPieceDto, { kind: 'Branch' }> => p.kind === 'Branch');
}

/** A block's declared input names, in prototype order — see `blockInputPieces`. */
export function blockInputNames(def: BlockDefDto): string[] {
  return blockInputPieces(def).map(p => p.name);
}

/** Looks up a custom block by id in the current macro's `block_defs` — used
 * wherever a call/header needs its prototype, not just its id. */
export function findBlockDef(macro: MacroDto | null | undefined, blockId: string): BlockDefDto | undefined {
  return macro?.block_defs.find(b => b.id === blockId);
}

export interface KeyCaptureDto {
  kind: 'Strand' | 'Standalone';
  strand_id: string | null;
  index: InstrPath | null;
}

/** Structural equality for two InstrPaths — used wherever a path is
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

// Mirrors src-tauri's `WindowAtPointDto`/`PickedElementDto` — the element
// picker's hover-hit-test result. `window` round-trips back into the next
// `hit_test_element_at` call as a cache hint (see that command's doc
// comment) so the overlay never needs its own geometry logic.
export interface WindowAtPointDto {
  app: string;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PickedElementDto {
  app: string;
  window_title: string;
  name: string;
  role: string;
  x: number;
  y: number;
  width: number;
  height: number;
  window: WindowAtPointDto;
}

// Mirrors src-tauri's `ElementPickerMonitorDto`/`ElementPickerGeometryDto` —
// the logical-space coordinate system `start_element_picker` opened the
// overlay in. The overlay fetches this on mount (see
// `ElementPickerOverlay.vue`) to map cursor positions to screen pixels
// exactly on mixed-DPI desktops, where no single scale factor is right for
// every monitor.
export interface ElementPickerMonitorDto {
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
}

export interface ElementPickerGeometryDto {
  origin_x: number;
  origin_y: number;
  width: number;
  height: number;
  monitors: ElementPickerMonitorDto[];
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
