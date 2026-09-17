// Operator value-block kinds.
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
  { kind: 'CurrentTime', op: 'CurrentTime', arity: 1, argTypes: ['text'], resultType: 'number', prefix: 'current', enumArg: { index: 0, options: CURRENT_TIME_OPTIONS } },
];

export function specForKind(kind: ValueKind): OperatorKindSpec | undefined {
  return OPERATOR_KINDS.find(s => s.kind === kind);
}

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
