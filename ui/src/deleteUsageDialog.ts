import { reactive } from 'vue';

export type DeletableEntity = 'variable' | 'list';

export const deleteUsageDialog = reactive({ open: false, kind: 'variable' as DeletableEntity, name: '' });

export function openDeleteUsageDialog(kind: DeletableEntity, name: string): void {
  deleteUsageDialog.kind = kind;
  deleteUsageDialog.name = name;
  deleteUsageDialog.open = true;
}

export function closeDeleteUsageDialog(): void {
  deleteUsageDialog.open = false;
  deleteUsageDialog.name = '';
}
