import { reactive } from 'vue';

export const listDialog = reactive({ open: false, renameTarget: '' });

export function openCreateListDialog(): void {
  listDialog.renameTarget = '';
  listDialog.open = true;
}

export function openRenameListDialog(name: string): void {
  listDialog.renameTarget = name;
  listDialog.open = true;
}

export function closeListDialog(): void {
  listDialog.open = false;
  listDialog.renameTarget = '';
}
