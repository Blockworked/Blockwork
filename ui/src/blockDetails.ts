// Content for the right-click "Details" popup — a plain-language explainer
// of what a block does, plus its name and wire identifier (the `type`/`kind`
// string, or a custom block's id). Purely descriptive/static data; nothing
// here reads live state.
import { INSTRUCTION_TYPE_LABELS } from './icons';
import type { BlockDefDto, InstructionType, ValueKind } from './types';

export interface BlockDetails {
  name: string;
  identifier: string;
  explainer: string;
}

const INSTRUCTION_EXPLAINERS: Record<InstructionType, string> = {
  WhenRan: 'Runs this strand as soon as the macro starts (Run button, hotkey, or command).',
  WhenBatteryDischargedTo: 'Runs this strand the moment the battery level drops to (or below) the given percentage.',
  WhenBatteryChargedTo: 'Runs this strand the moment the battery level rises to (or above) the given percentage.',
  WhenTime: 'Runs this strand at a recurring point in local time — daily, weekly, monthly, or yearly.',
  WhenPowerPluggedIn: 'Runs this strand the moment the system is connected to external power.',
  WhenPowerUnplugged: 'Runs this strand the moment the system is disconnected from external power.',
  Wait: 'Pauses this strand for the given number of milliseconds before continuing.',
  Text: 'Types the given text, as if entered on the keyboard.',
  Key: 'Presses, releases, or clicks a single keyboard key.',
  Button: 'Presses, releases, or clicks a mouse button.',
  MoveMouse: 'Moves the mouse cursor to an absolute screen position, or by a relative offset.',
  Scroll: 'Scrolls the mouse wheel vertically or horizontally by the given amount.',
  Command: 'Runs a shell command in the background.',
  OpenApp: 'Launches the chosen application.',
  CloseApp: 'Closes the chosen application.',
  Comment: 'A free-form note left on the canvas — has no effect when the macro runs.',
  SetVariable: "Sets a variable to the given value, replacing whatever it held before.",
  ChangeVariable: "Adds the given number to a variable's current value.",
  BlockHeader: 'The definition header of one of your custom "My Blocks" — everything below it runs each time the block is called.',
  CallBlock: 'Calls one of your own custom "My Blocks" definitions, running its body inline.',
  Return: "Ends a custom block's body immediately, handing the given value back to whoever called it.",
  If: 'Runs the blocks inside only if the condition is true.',
  IfElse: 'Runs the first set of blocks if the condition is true, otherwise runs the second set.',
  Repeat: 'Runs the blocks inside a fixed number of times.',
  Forever: 'Runs the blocks inside in an endless loop, until stopped or escaped.',
  While: 'Runs the blocks inside repeatedly for as long as the condition stays true.',
  EscapeLoop: 'Immediately exits the nearest enclosing loop, skipping any remaining iterations.',
  ContinueLoop: 'Immediately skips to the next iteration of the nearest enclosing loop.',
};

export function detailsForInstructionType(type: InstructionType): BlockDetails {
  return {
    name: INSTRUCTION_TYPE_LABELS[type],
    identifier: type,
    explainer: INSTRUCTION_EXPLAINERS[type],
  };
}

const VALUE_KIND_LABELS: Partial<Record<ValueKind, string>> = {
  Number: 'Number',
  Text: 'Text',
  Add: 'Add', Sub: 'Subtract', Mul: 'Multiply', Div: 'Divide', Mod: 'Modulo',
  Round: 'Round', Random: 'Pick Random', Join: 'Join', Join3: 'Join (3)',
  NewLine: 'New Line', Tab: 'Tab',
  IndexOf: 'Index Of', LastIndexOf: 'Last Index Of', LetterOf: 'Letter Of',
  Length: 'Length', Case: 'Change Case',
  Eq: 'Equals', Neq: 'Not Equals', Gt: 'Greater Than', Lt: 'Less Than',
  Gte: 'Greater Than or Equal', Lte: 'Less Than or Equal',
  And: 'And', Or: 'Or', Not: 'Not', True: 'True', False: 'False',
  BatteryPercentage: 'Battery Percentage', PluggedIn: 'Plugged In',
  CurrentTime: 'Current Time',
};

const VALUE_KIND_EXPLAINERS: Partial<Record<ValueKind, string>> = {
  Number: 'A literal numeric value you can type in directly.',
  Text: 'A literal piece of text you can type in directly.',
  Add: 'Adds two numbers together.',
  Sub: 'Subtracts the second number from the first.',
  Mul: 'Multiplies two numbers together.',
  Div: 'Divides the first number by the second.',
  Mod: 'Returns the remainder of dividing the first number by the second.',
  Round: 'Rounds a number to the nearest whole number.',
  Random: 'Picks a random number between the two given bounds, inclusive.',
  Join: 'Joins two pieces of text together, end to end.',
  Join3: 'Joins three pieces of text together, end to end.',
  NewLine: 'A line-break character, for building multi-line text.',
  Tab: 'A tab character, for building spaced-out text.',
  IndexOf: 'Finds the position of the first occurrence of one piece of text inside another (0 if not found).',
  LastIndexOf: 'Finds the position of the last occurrence of one piece of text inside another (0 if not found).',
  LetterOf: 'Gets the single character at a given position within a piece of text.',
  Length: 'Counts the number of characters in a piece of text.',
  Case: 'Converts a piece of text to uppercase or lowercase.',
  Eq: 'True if two numbers are equal.',
  Neq: 'True if two numbers are not equal.',
  Gt: 'True if the first number is greater than the second.',
  Lt: 'True if the first number is less than the second.',
  Gte: 'True if the first number is greater than or equal to the second.',
  Lte: 'True if the first number is less than or equal to the second.',
  And: 'True only if both conditions are true.',
  Or: 'True if either condition is true.',
  Not: "True if the condition is false, and false if it's true.",
  True: "The fixed boolean value 'true'.",
  False: "The fixed boolean value 'false'.",
  BatteryPercentage: "The system's current battery charge, from 0 to 100.",
  PluggedIn: 'True if the system is currently connected to external power.',
  CurrentTime: 'A component (year, month, date, day of week, hour, minute, or second) of the current local time.',
};

export function detailsForValueKind(kind: string): BlockDetails {
  const label = VALUE_KIND_LABELS[kind as ValueKind];
  return {
    name: label ?? kind,
    identifier: kind,
    explainer: VALUE_KIND_EXPLAINERS[kind as ValueKind] ?? 'An operator block.',
  };
}

const BLOCK_SHAPE_EXPLAINERS: Record<BlockDefDto['shape'], string> = {
  Normal: 'A custom block you defined — drag it onto a strand to call it, running the blocks under its "My Blocks" definition.',
  Ending: 'A custom block you defined that ends a strand — drag it onto a strand to call it; nothing can be placed below it.',
  ReturnsValue: 'A custom block you defined that returns a number or text — drag it into a value slot to call it and use its result.',
  ReturnsBool: 'A custom block you defined that returns a boolean — drag it into a boolean slot to call it and use its result.',
};

export function detailsForBlockDef(def: BlockDefDto): BlockDetails {
  const name = def.pieces.map(p => (p.kind === 'Label' ? p.text : `[${p.name}]`)).join(' ').trim() || '(unnamed block)';
  return {
    name,
    identifier: def.id,
    explainer: BLOCK_SHAPE_EXPLAINERS[def.shape],
  };
}
