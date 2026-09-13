// Reference checks used before deleting a variable or list. These walk the
// document shown on the canvas, including nested C-block bodies and floating
// reporter blocks, but never mutate it.
import type { InstructionDto, MacroDto, ValueDto, ValueOp } from './types';

const LIST_NAME_ARG: Partial<Record<ValueOp, number>> = {
  ListItem: 1,
  ListItemNumber: 1,
  ListAmount: 1,
  ListLength: 0,
  ListContains: 0,
  ListItemExists: 1,
  ListIsEmpty: 0,
};

function textArgumentIs(value: ValueDto | undefined, name: string): boolean {
  return value?.kind === 'Text' && value.value === name;
}

function valueUsesVariable(value: ValueDto, name: string): boolean {
  if (value.kind === 'Var') return value.name === name;
  if (value.kind === 'Op' || value.kind === 'Call') {
    return value.args.some(arg => valueUsesVariable(arg, name)) || valueUsesVariable(value.saved, name);
  }
  return false;
}

function valueUsesList(value: ValueDto, name: string): boolean {
  if (value.kind !== 'Op' && value.kind !== 'Call') return false;
  if (value.kind === 'Op') {
    const listNameIndex = LIST_NAME_ARG[value.op];
    if (listNameIndex !== undefined && textArgumentIs(value.args[listNameIndex], name)) return true;
  }
  return value.args.some(arg => valueUsesList(arg, name)) || valueUsesList(value.saved, name);
}

function instructionUsesVariable(instruction: InstructionDto, name: string): boolean {
  switch (instruction.type) {
    case 'SetVariable':
    case 'ChangeVariable':
      return instruction.name === name || valueUsesVariable(instruction.value, name);
    case 'Wait': case 'Text': case 'WhenBatteryDischargedTo': case 'WhenBatteryChargedTo': case 'Return':
      return valueUsesVariable(instruction.type === 'Wait' ? instruction.duration : instruction.type === 'Text' ? instruction.text : instruction.type === 'Return' ? instruction.value : instruction.threshold, name);
    case 'MoveMouse': return valueUsesVariable(instruction.x, name) || valueUsesVariable(instruction.y, name);
    case 'Scroll': return valueUsesVariable(instruction.amount, name);
    case 'AddToList': return valueUsesVariable(instruction.value, name);
    case 'DeleteOfList': return valueUsesVariable(instruction.index, name);
    case 'ShiftList': return valueUsesVariable(instruction.amount, name);
    case 'InsertIntoList': return valueUsesVariable(instruction.value, name) || valueUsesVariable(instruction.index, name);
    case 'ReplaceItemOfList': return valueUsesVariable(instruction.index, name) || valueUsesVariable(instruction.value, name);
    case 'CallBlock': return instruction.args.some(arg => valueUsesVariable(arg, name));
    case 'BranchCallBlock': return instruction.args.some(arg => valueUsesVariable(arg, name)) || instruction.branches.some(branch => instructionsUseVariable(branch, name));
    case 'If': return valueUsesVariable(instruction.condition, name) || instructionsUseVariable(instruction.body, name);
    case 'IfElse': return valueUsesVariable(instruction.condition, name) || instructionsUseVariable(instruction.then_body, name) || instructionsUseVariable(instruction.else_body, name);
    case 'Repeat': return valueUsesVariable(instruction.count, name) || instructionsUseVariable(instruction.body, name);
    case 'While': return valueUsesVariable(instruction.condition, name) || instructionsUseVariable(instruction.body, name);
    case 'Forever': return instructionsUseVariable(instruction.body, name);
    default: return false;
  }
}

function instructionUsesList(instruction: InstructionDto, name: string): boolean {
  switch (instruction.type) {
    case 'AddToList': return instruction.name === name || valueUsesList(instruction.value, name);
    case 'DeleteOfList': return instruction.name === name || valueUsesList(instruction.index, name);
    case 'DeleteAllOfList': case 'ReverseList': return instruction.name === name;
    case 'ShiftList': return instruction.name === name || valueUsesList(instruction.amount, name);
    case 'InsertIntoList': return instruction.name === name || valueUsesList(instruction.value, name) || valueUsesList(instruction.index, name);
    case 'ReplaceItemOfList': return instruction.name === name || valueUsesList(instruction.index, name) || valueUsesList(instruction.value, name);
    case 'SetVariable': case 'ChangeVariable': return valueUsesList(instruction.value, name);
    case 'Wait': case 'Text': case 'WhenBatteryDischargedTo': case 'WhenBatteryChargedTo': case 'Return':
      return valueUsesList(instruction.type === 'Wait' ? instruction.duration : instruction.type === 'Text' ? instruction.text : instruction.type === 'Return' ? instruction.value : instruction.threshold, name);
    case 'MoveMouse': return valueUsesList(instruction.x, name) || valueUsesList(instruction.y, name);
    case 'Scroll': return valueUsesList(instruction.amount, name);
    case 'CallBlock': return instruction.args.some(arg => valueUsesList(arg, name));
    case 'BranchCallBlock': return instruction.args.some(arg => valueUsesList(arg, name)) || instruction.branches.some(branch => instructionsUseList(branch, name));
    case 'If': return valueUsesList(instruction.condition, name) || instructionsUseList(instruction.body, name);
    case 'IfElse': return valueUsesList(instruction.condition, name) || instructionsUseList(instruction.then_body, name) || instructionsUseList(instruction.else_body, name);
    case 'Repeat': return valueUsesList(instruction.count, name) || instructionsUseList(instruction.body, name);
    case 'While': return valueUsesList(instruction.condition, name) || instructionsUseList(instruction.body, name);
    case 'Forever': return instructionsUseList(instruction.body, name);
    default: return false;
  }
}

function instructionsUseVariable(instructions: InstructionDto[], name: string): boolean {
  return instructions.some(instruction => instructionUsesVariable(instruction, name));
}

function instructionsUseList(instructions: InstructionDto[], name: string): boolean {
  return instructions.some(instruction => instructionUsesList(instruction, name));
}

export function macroUsesVariable(macro: MacroDto | null | undefined, name: string): boolean {
  return !!macro && (
    macro.strands.some(strand => instructionsUseVariable(strand.instructions, name)) ||
    macro.floating_values.some(floating => valueUsesVariable(floating.value, name))
  );
}

export function macroUsesList(macro: MacroDto | null | undefined, name: string): boolean {
  return !!macro && (
    macro.strands.some(strand => instructionsUseList(strand.instructions, name)) ||
    macro.floating_values.some(floating => valueUsesList(floating.value, name))
  );
}
