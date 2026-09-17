// State for the "Make a Block"/"Edit Block" popup.
import { reactive } from 'vue';
import type { BlockDefDto } from './types';

type BlockDialogMode = 'create' | 'edit' | null;

interface BlockDialogState {
  mode: BlockDialogMode;
  editTarget: BlockDefDto | null;
}

export const blockDialog = reactive<BlockDialogState>({ mode: null, editTarget: null });

export function openCreateBlockDialog(): void {
  blockDialog.mode = 'create';
  blockDialog.editTarget = null;
}

export function openEditBlockDialog(def: BlockDefDto): void {
  blockDialog.mode = 'edit';
  blockDialog.editTarget = def;
}

export function closeBlockDialog(): void {
  blockDialog.mode = null;
  blockDialog.editTarget = null;
}
