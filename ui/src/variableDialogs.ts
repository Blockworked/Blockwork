// State for the variable name popup.
import { reactive } from 'vue';

type VariableDialogMode = 'create' | 'rename' | null;

interface VariableDialogState {
  mode: VariableDialogMode;
  renameTarget: string;
}

export const variableDialog = reactive<VariableDialogState>({ mode: null, renameTarget: '' });

export function openCreateVariableDialog(): void {
  variableDialog.mode = 'create';
  variableDialog.renameTarget = '';
}

export function openRenameVariableDialog(name: string): void {
  variableDialog.mode = 'rename';
  variableDialog.renameTarget = name;
}

export function closeVariableDialog(): void {
  variableDialog.mode = null;
}
