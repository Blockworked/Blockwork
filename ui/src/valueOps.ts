// Single source of truth for every operator value-block kind - mirrors
// blockwork-core/src/input/value.rs's OPERATOR_KINDS. Add a new operator (or
// arity variant, like Join/Join3) as one row here.
//
// Type-only import below avoids a circular-init hazard with types.ts.
import { reactive } from 'vue';
import type { ValueDto, ValueKind, ValueOp } from './types';

/** Every operator `ValueKind` (excludes the `Number`/`Text` leaves). */
export type OperatorValueKind = Exclude<ValueKind, 'Number' | 'Text'>;

export interface OperatorKindSpec {
  kind: OperatorValueKind;
  op: ValueOp;
  arity: number;
  argTypes: ('number' | 'text' | 'bool')[];
  resultType: 'number' | 'text' | 'bool';
  prefix?: string;
  infix?: string;
  /** Rendered after the final arg. */
  suffix?: string;
  /** If set, `args[enumArg.index]` is a fixed dropdown choice, not a
   * draggable Value slot - e.g. Case's upper/lowercase toggle. */
  enumArg?: { index: number; options: { value: string; label: string }[] };
}

const CASE_OPTIONS = [
  { value: 'Upper', label: 'uppercase' },
  { value: 'Lower', label: 'lowercase' },
];

const MATH_OPTIONS = [
  { value: 'Abs', label: 'abs' },
  { value: 'Floor', label: 'floor' },
  { value: 'Ceiling', label: 'ceiling' },
  { value: 'Sign', label: 'sign' },
  { value: 'Sqrt', label: 'sqrt' },
  { value: 'Sin', label: 'sin' },
  { value: 'Cos', label: 'cos' },
  { value: 'Tan', label: 'tan' },
  { value: 'Asin', label: 'asin' },
  { value: 'Acos', label: 'acos' },
  { value: 'Atan', label: 'atan' },
  { value: 'Ln', label: 'ln' },
  { value: 'Log', label: 'log' },
  { value: 'Log2', label: 'log2' },
  { value: 'EPower', label: 'e ^' },
  { value: 'TenPower', label: '10 ^' },
];

/** Live list-name choices shared by real and palette value blocks. The
 * reactive array stays stable so blockstitch's registered operator specs see
 * updates when a list is created, renamed, or deleted. */
// The palette initializes before backend state arrives, so both arrays need a
// usable first option immediately (defaultArgFor reads options[0]). They are
// replaced with the real macro lists as soon as the sidebar mounts.
export const LIST_NAME_OPTIONS = reactive<{ value: string; label: string }[]>([{ value: '', label: 'list' }]);
const LIST_EMPTY_OPTIONS = reactive<{ value: string; label: string }[]>([{ value: '', label: 'list' }]);

export function setListNameOptions(names: string[]) {
  const choices = names.length ? names : [''];
  LIST_NAME_OPTIONS.splice(0, LIST_NAME_OPTIONS.length, ...choices.map(name => ({ value: name, label: name || 'list' })));
  LIST_EMPTY_OPTIONS.splice(0, LIST_EMPTY_OPTIONS.length, ...choices.map(name => ({ value: name, label: name || 'list' })));
}

// Mirrors blockwork-core's `Value::eval`'s `Op::CurrentTime` match arm - always
// numeric (`DayOfWeek` is 1=Sunday..7=Saturday, `Hour` is always 24-hour),
// matching Scratch's own "current ()" sensing block.
const CURRENT_TIME_OPTIONS = [
  { value: 'Year', label: 'year' },
  { value: 'Month', label: 'month' },
  { value: 'Date', label: 'date (day of month)' },
  { value: 'DayOfWeek', label: 'day of week' },
  { value: 'Hour', label: 'hour' },
  { value: 'Minute', label: 'minute' },
  { value: 'Second', label: 'second' },
];

export const OPERATOR_KINDS: OperatorKindSpec[] = [
  { kind: 'Add', op: 'Add', arity: 2, argTypes: ['number', 'number'], resultType: 'number', infix: '+' },
  { kind: 'Sub', op: 'Sub', arity: 2, argTypes: ['number', 'number'], resultType: 'number', infix: '−' },
  { kind: 'Mul', op: 'Mul', arity: 2, argTypes: ['number', 'number'], resultType: 'number', infix: '×' },
  { kind: 'Div', op: 'Div', arity: 2, argTypes: ['number', 'number'], resultType: 'number', infix: '/' },
  { kind: 'Mod', op: 'Mod', arity: 2, argTypes: ['number', 'number'], resultType: 'number', infix: 'mod' },
  { kind: 'Round', op: 'Round', arity: 1, argTypes: ['number'], resultType: 'number', prefix: 'round' },
  { kind: 'Math', op: 'Math', arity: 2, argTypes: ['text', 'number'], resultType: 'number', infix: 'of', enumArg: { index: 0, options: MATH_OPTIONS } },
  { kind: 'Random', op: 'Random', arity: 2, argTypes: ['number', 'number'], resultType: 'number', prefix: 'pick random from', infix: 'to' },
  { kind: 'Join', op: 'Join', arity: 2, argTypes: ['text', 'text'], resultType: 'text', prefix: 'join' },
  { kind: 'Join3', op: 'Join', arity: 3, argTypes: ['text', 'text', 'text'], resultType: 'text', prefix: 'join' },
  { kind: 'NewLine', op: 'NewLine', arity: 0, argTypes: [], resultType: 'text', prefix: 'new line' },
  { kind: 'Tab', op: 'Tab', arity: 0, argTypes: [], resultType: 'text', prefix: 'tab character' },
  { kind: 'IndexOf', op: 'IndexOf', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'index of', infix: 'in' },
  { kind: 'LastIndexOf', op: 'LastIndexOf', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'last index of', infix: 'in' },
  { kind: 'LetterOf', op: 'LetterOf', arity: 2, argTypes: ['number', 'text'], resultType: 'text', prefix: 'letter', infix: 'of' },
  { kind: 'Length', op: 'Length', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'length of' },
  { kind: 'Case', op: 'Case', arity: 2, argTypes: ['text', 'text'], resultType: 'text', infix: 'to', enumArg: { index: 1, options: CASE_OPTIONS } },
  { kind: 'Eq', op: 'Eq', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '=' },
  { kind: 'Neq', op: 'Neq', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '≠' },
  { kind: 'Gt', op: 'Gt', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '>' },
  { kind: 'Lt', op: 'Lt', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '<' },
  { kind: 'Gte', op: 'Gte', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '≥' },
  { kind: 'Lte', op: 'Lte', arity: 2, argTypes: ['number', 'number'], resultType: 'bool', infix: '≤' },
  { kind: 'And', op: 'And', arity: 2, argTypes: ['bool', 'bool'], resultType: 'bool', infix: 'and' },
  { kind: 'Or', op: 'Or', arity: 2, argTypes: ['bool', 'bool'], resultType: 'bool', infix: 'or' },
  { kind: 'Not', op: 'Not', arity: 1, argTypes: ['bool'], resultType: 'bool', prefix: 'not' },
  { kind: 'True', op: 'True', arity: 0, argTypes: [], resultType: 'bool', prefix: 'true' },
  { kind: 'False', op: 'False', arity: 0, argTypes: [], resultType: 'bool', prefix: 'false' },
  { kind: 'BatteryPercentage', op: 'BatteryPercentage', arity: 0, argTypes: [], resultType: 'number', prefix: 'battery percentage' },
  { kind: 'PluggedIn', op: 'PluggedIn', arity: 0, argTypes: [], resultType: 'bool', prefix: 'plugged in?' },
  // Zero-arity, like BatteryPercentage - evaluates to the live clipboard text.
  { kind: 'ClipboardText', op: 'ClipboardText', arity: 0, argTypes: [], resultType: 'text', prefix: 'clipboard text' },
  // Zero-arity, like PluggedIn - evaluates to whether the clipboard currently
  // holds image data or a file list, respectively.
  { kind: 'ClipboardHasImage', op: 'ClipboardHasImage', arity: 0, argTypes: [], resultType: 'bool', prefix: 'clipboard has image' },
  { kind: 'ClipboardHasFiles', op: 'ClipboardHasFiles', arity: 0, argTypes: [], resultType: 'bool', prefix: 'clipboard has files' },
  { kind: 'CurrentTime', op: 'CurrentTime', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'current', enumArg: { index: 0, options: CURRENT_TIME_OPTIONS } },
  { kind: 'ListItem', op: 'ListItem', arity: 2, argTypes: ['number', 'text'], resultType: 'text', prefix: 'item', infix: 'of', enumArg: { index: 1, options: LIST_NAME_OPTIONS } },
  { kind: 'ListItemNumber', op: 'ListItemNumber', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'item # of', infix: 'in', enumArg: { index: 1, options: LIST_NAME_OPTIONS } },
  { kind: 'ListAmount', op: 'ListAmount', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'amount of', infix: 'in', enumArg: { index: 1, options: LIST_NAME_OPTIONS } },
  { kind: 'ListLength', op: 'ListLength', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'length of', enumArg: { index: 0, options: LIST_NAME_OPTIONS } },
  { kind: 'ListContains', op: 'ListContains', arity: 2, argTypes: ['text', 'text'], resultType: 'bool', infix: 'contains', enumArg: { index: 0, options: LIST_NAME_OPTIONS } },
  { kind: 'ListItemExists', op: 'ListItemExists', arity: 2, argTypes: ['number', 'text'], resultType: 'bool', prefix: 'item', infix: 'exists in', enumArg: { index: 1, options: LIST_NAME_OPTIONS } },
  { kind: 'ListIsEmpty', op: 'ListIsEmpty', arity: 1, argTypes: ['text'], resultType: 'bool', prefix: 'is', suffix: 'empty?', enumArg: { index: 0, options: LIST_EMPTY_OPTIONS } },
];

export function specForKind(kind: ValueKind): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.kind === kind);
}

export function specForOp(op: ValueOp): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.op === op);
}

export function labelForOp(op: ValueOp): Pick<OperatorKindSpec, 'prefix' | 'infix' | 'suffix'> | undefined {
  const spec = specForOp(op);
  return spec && { prefix: spec.prefix, infix: spec.infix, suffix: spec.suffix };
}

export function defaultArgFor(spec: OperatorKindSpec, index: number): ValueDto {
  if (spec.enumArg?.index === index) return { kind: 'Text', value: spec.enumArg.options[0].value };
  // Lists are one-based. Keep the reporter palette consistent with the
  // command-block defaults and the backend's operator construction.
  if ((spec.kind === 'ListItem' || spec.kind === 'ListItemExists') && index === 0) {
    return { kind: 'Number', value: 1 };
  }
  if (spec.argTypes[index] === 'bool') return { kind: 'Bool' };
  return spec.argTypes[index] === 'text' ? { kind: 'Text', value: '' } : { kind: 'Number', value: 0 };
}
