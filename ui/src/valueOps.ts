// Single source of truth for every operator value-block kind — mirrors
// src-tauri/src/input/value.rs's OPERATOR_KINDS. Add a new operator (or
// arity variant, like Join/Join3) as one row here.
//
// Type-only import below avoids a circular-init hazard with types.ts.
import type { ValueDto, ValueKind, ValueOp } from './types';

/** Every operator `ValueKind` (excludes the `Number`/`Text` leaves) — lets
 * `Record<OperatorValueKind, ...>` state stay in sync automatically. */
export type OperatorValueKind = Exclude<ValueKind, 'Number' | 'Text'>;

export interface OperatorKindSpec {
  kind: OperatorValueKind;
  op: ValueOp;
  arity: number;
  /** One entry per arg, in order — lets an operator mix types (e.g.
   * LetterOf's number-then-text pair). */
  argTypes: ('number' | 'text' | 'bool')[];
  /** What this operator's result "is" — drives shape (booleans render as a
   * hexagon, see ValueBlock.vue). */
  resultType: 'number' | 'text' | 'bool';
  /** Rendered before the first arg (word-phrase operators like Random/Join). */
  prefix?: string;
  /** Rendered between each consecutive pair of args, symbol or word alike. */
  infix?: string;
  /** If set, `args[enumArg.index]` is a fixed dropdown choice, not a
   * draggable Value slot — e.g. Case's upper/lowercase toggle. */
  enumArg?: { index: number; options: { value: string; label: string }[] };
}

const CASE_OPTIONS = [
  { value: 'Upper', label: 'uppercase' },
  { value: 'Lower', label: 'lowercase' },
];

// Scratch's `([abs v] of ())` number reporter. Trig values use degrees; the
// backend owns that behavior, while these strings are the serialized dropdown
// values it matches on. `log2` appears in the supplied Scratch menu alongside
// the standard base-10 `log`.
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

// Mirrors blockwork-core's `Value::eval`'s `Op::CurrentTime` match arm — always
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
  // Zero-arity text constants — argTypes is unused (no args to render).
  { kind: 'NewLine', op: 'NewLine', arity: 0, argTypes: [], resultType: 'text', prefix: 'new line' },
  { kind: 'Tab', op: 'Tab', arity: 0, argTypes: [], resultType: 'text', prefix: 'tab character' },
  { kind: 'IndexOf', op: 'IndexOf', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'index of', infix: 'in' },
  { kind: 'LastIndexOf', op: 'LastIndexOf', arity: 2, argTypes: ['text', 'text'], resultType: 'number', prefix: 'last index of', infix: 'in' },
  { kind: 'LetterOf', op: 'LetterOf', arity: 2, argTypes: ['number', 'text'], resultType: 'text', prefix: 'letter', infix: 'of' },
  { kind: 'Length', op: 'Length', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'length of' },
  { kind: 'Case', op: 'Case', arity: 2, argTypes: ['text', 'text'], resultType: 'text', infix: 'to', enumArg: { index: 1, options: CASE_OPTIONS } },
  // Boolean: comparisons, logic, and two standalone true/false literal
  // blocks (separate blocks per design, not a toggle).
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
  // Zero-arity, like NewLine/Tab — evaluates to the live system battery percentage.
  { kind: 'BatteryPercentage', op: 'BatteryPercentage', arity: 0, argTypes: [], resultType: 'number', prefix: 'battery percentage' },
  { kind: 'PluggedIn', op: 'PluggedIn', arity: 0, argTypes: [], resultType: 'bool', prefix: 'plugged in' },
  // One arg, entirely a fixed dropdown (no draggable operand) — same enumArg
  // shape as Case, just with nothing else alongside it.
  { kind: 'CurrentTime', op: 'CurrentTime', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'current', enumArg: { index: 0, options: CURRENT_TIME_OPTIONS } },
];

export function specForKind(kind: ValueKind): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.kind === kind);
}

/** An existing `Op` node only carries `op`, not which palette kind built it —
 * returns the first spec matching `op` (labels match across arities, e.g. Join/Join3). */
export function specForOp(op: ValueOp): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.op === op);
}

export function labelForOp(op: ValueOp): Pick<OperatorKindSpec, 'prefix' | 'infix'> | undefined {
  const spec = specForOp(op);
  return spec && { prefix: spec.prefix, infix: spec.infix };
}

export function defaultArgFor(spec: OperatorKindSpec, index: number): ValueDto {
  if (spec.enumArg?.index === index) return { kind: 'Text', value: spec.enumArg.options[0].value };
  if (spec.argTypes[index] === 'bool') return { kind: 'Bool' };
  return spec.argTypes[index] === 'text' ? { kind: 'Text', value: '' } : { kind: 'Number', value: 0 };
}
