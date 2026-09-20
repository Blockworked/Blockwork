import { reactive, ref } from 'vue';
import { emptyState, type InstructionDto, type StateDto } from './types';
import { getAppVersion, getState, onStateUpdated, requestAbsoluteMouseSupport } from './tauri';

// Reactive backend state snapshot.
export const state = reactive<StateDto>(emptyState());
export const appVersion = ref('');

let initialized = false;
let absoluteSupportRequested = false;

function instructionsContainAbsoluteMove(instructions: InstructionDto[]): boolean {
  return instructions.some(instruction => {
    if (instruction.type === 'MoveMouse' && instruction.coordinate === 'Absolute') return true;
    if (instruction.type === 'If' || instruction.type === 'Repeat' || instruction.type === 'Forever' || instruction.type === 'While') {
      return instructionsContainAbsoluteMove(instruction.body);
    }
    if (instruction.type === 'IfElse') {
      return instructionsContainAbsoluteMove(instruction.then_body) || instructionsContainAbsoluteMove(instruction.else_body);
    }
    return false;
  });
}

function requestAbsoluteSupportForLoadedMacro(snapshot: StateDto) {
  if (absoluteSupportRequested || !snapshot.current_macro) return;
  if (!snapshot.current_macro.strands.some(strand => instructionsContainAbsoluteMove(strand.instructions))) return;
  absoluteSupportRequested = true;
  void requestAbsoluteMouseSupport().catch(error => {
    console.error('Absolute mouse permission is unavailable:', error);
  });
}

export async function initState(): Promise<void> {
  if (initialized) return;
  initialized = true;

  try {
    appVersion.value = await getAppVersion();
  } catch (e) {
    console.error('Failed to get app version:', e);
  }

  try {
    const s = await getState();
    Object.assign(state, s);
    requestAbsoluteSupportForLoadedMacro(s);
  } catch (e) {
    console.error('Failed to get initial state:', e);
  }

  try {
    await onStateUpdated(s => {
      Object.assign(state, s);
      requestAbsoluteSupportForLoadedMacro(s);
    });
  } catch (e) {
    console.error('Failed to subscribe to state updates:', e);
  }
}
