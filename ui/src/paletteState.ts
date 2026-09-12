// Sidebar prefab state. Cloned onto the canvas when dragged out.
import { reactive } from 'vue';
import { defaultInstruction, defaultValueForKind, newId, numberValue, textValue } from './types';
import type { InstructionDto, InstructionType, ValueDto, ValueKind } from './types';
import { OPERATOR_KINDS, specForKind } from './valueOps';
import type { OperatorKindSpec, OperatorValueKind } from './valueOps';

const INSTRUCTION_TYPES: InstructionType[] = [
  'WhenRan', 'WhenBatteryDischargedTo', 'WhenBatteryChargedTo', 'WhenTime', 'WhenPowerPluggedIn', 'WhenPowerUnplugged', 'Wait', 'Text', 'Key', 'Button', 'MoveMouse', 'Scroll', 'Command', 'OpenApp', 'CloseApp',
  'SetVariable', 'ChangeVariable', 'Return', 'If', 'IfElse',
  'AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList', 'InsertIntoList', 'ReplaceItemOfList', 'ReverseList',
  'Repeat', 'Forever', 'While', 'EscapeLoop', 'ContinueLoop',
];

export const paletteInstructions: Record<InstructionType, InstructionDto> = reactive(
  Object.fromEntries(INSTRUCTION_TYPES.map(t => [t, defaultInstruction(t)])) as Record<InstructionType, InstructionDto>,
);

export function clonePaletteInstruction(type: InstructionType): InstructionDto {
  return { ...JSON.parse(JSON.stringify(paletteInstructions[type])), id: newId() };
}

function argsFor(spec: OperatorKindSpec): (number | string)[] {
  const seed = defaultValueForKind(spec.kind);
  if (seed.kind !== 'Op') return [];
  return seed.args.map(a => (a.kind === 'Number' ? a.value : a.kind === 'Text' ? a.value : ''));
}

// Every operator prefab carries its own editable arg list (numbers or text
// per its `argTypes`) - no nesting, since the sidebar is never a valid drop
// target (see isOverSidebar), so these always stay plain leaves.
export const paletteOperatorArgs: Record<OperatorValueKind, (number | string)[]> = reactive(
  Object.fromEntries(OPERATOR_KINDS.map(s => [s.kind, argsFor(s)])) as Record<OperatorValueKind, (number | string)[]>,
);

const numberSeed = defaultValueForKind('Number');
const textSeed = defaultValueForKind('Text');
export const paletteNumber = reactive({ value: numberSeed.kind === 'Number' ? numberSeed.value : 0 });
export const paletteText = reactive({ value: textSeed.kind === 'Text' ? textSeed.value : '' });

const LIST_OPERATOR_KINDS: OperatorValueKind[] = [
  'ListItem', 'ListItemNumber', 'ListAmount', 'ListLength',
  'ListContains', 'ListItemExists', 'ListIsEmpty',
];
const LIST_COMMAND_TYPES: InstructionType[] = [
  'AddToList', 'DeleteOfList', 'DeleteAllOfList', 'ShiftList',
  'InsertIntoList', 'ReplaceItemOfList', 'ReverseList',
];

/** Keeps sidebar prefabs useful as soon as names arrive from the backend.
 * Only unselected or no-longer-valid palette targets are changed; blocks
 * already dropped onto the canvas are persisted separately and untouched. */
export function syncPaletteTargetDefaults(variableNames: string[], listNames: string[]): void {
  const firstVariable = variableNames[0];
  const firstList = listNames[0];
  if (firstVariable) {
    for (const type of ['SetVariable', 'ChangeVariable'] as const) {
      const instruction = paletteInstructions[type] as Extract<InstructionDto, { type: 'SetVariable' | 'ChangeVariable' }>;
      if (!instruction.name || !variableNames.includes(instruction.name)) instruction.name = firstVariable;
    }
  }
  if (!firstList) return;
  for (const type of LIST_COMMAND_TYPES) {
    const instruction = paletteInstructions[type] as Extract<InstructionDto, { type: 'AddToList' | 'DeleteOfList' | 'DeleteAllOfList' | 'ShiftList' | 'InsertIntoList' | 'ReplaceItemOfList' | 'ReverseList' }>;
    if (!instruction.name || !listNames.includes(instruction.name)) instruction.name = firstList;
  }
  for (const kind of LIST_OPERATOR_KINDS) {
    const spec = specForKind(kind);
    const index = spec?.enumArg?.index;
    const args = paletteOperatorArgs[kind];
    if (index !== undefined && (!args[index] || !listNames.includes(String(args[index])))) args[index] = firstList;
  }
}

/** Applies an edit blockstitch's generic `PaletteValueBlock` made (via its
 * `update:value` emit) back onto this kind's own persisted draft state -
 * the inverse of `paletteValueFor`. No-op for `Var:`/`Param:` kinds, which
 * have nothing to edit. */
export function applyPaletteValueEdit(kind: ValueKind, next: ValueDto): void {
  if (kind === 'Number' && next.kind === 'Number') {
    paletteNumber.value = next.value;
    return;
  }
  if (kind === 'Text' && next.kind === 'Text') {
    paletteText.value = next.value;
    return;
  }
  if (next.kind !== 'Op') return;
  const spec = specForKind(kind);
  if (!spec) return;
  const args = paletteOperatorArgs[kind as OperatorValueKind];
  if (!args) return;
  next.args.forEach((arg, i) => {
    if (arg.kind === 'Number') args[i] = arg.value;
    else if (arg.kind === 'Text') args[i] = arg.value;
  });
}

/** The ValueDto a value-palette entry currently represents, built from its
 * live edited state - what lands on the canvas when dragged out. */
export function paletteValueFor(kind: ValueKind): ValueDto {
  if (kind === 'Number') return numberValue(paletteNumber.value);
  if (kind === 'Text') return textValue(paletteText.value);
  if (kind.startsWith('Var:') || kind.startsWith('Param:')) return defaultValueForKind(kind);
  const spec = specForKind(kind);
  if (!spec) throw new Error(`Unknown value kind: ${kind}`);
  const args = paletteOperatorArgs[kind as OperatorValueKind];
  return {
    kind: 'Op',
    op: spec.op,
    args: args.map((v, i) => {
      const argType = spec.argTypes[i];
      // A list dropdown's reactive choices can arrive after this palette
      // state was initialized. Fall back to its first choice so the sidebar
      // and a fresh drag always agree on the visible default.
      const enumValue = spec.enumArg?.index === i && !v ? spec.enumArg.options[0]?.value ?? '' : v;
      if (argType === 'text') return textValue(String(enumValue));
      // No editable palette leaf for booleans - blank, same as
      // defaultArgFor's fallback for a bool-typed slot.
      if (argType === 'bool') return { kind: 'Bool' };
      return numberValue(Number(v));
    }),
    saved: numberValue(0),
  };
}
