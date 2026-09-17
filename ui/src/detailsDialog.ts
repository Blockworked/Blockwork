// Reactive singleton for the "Details" popup - same shape as blockDialogs.ts,
// but read-only (no mode/editTarget split, just "show this content").
import { reactive } from 'vue';
import type { BlockDetails } from './blockDetails';

interface DetailsDialogState {
  open: boolean;
  details: BlockDetails | null;
}

export const detailsDialog = reactive<DetailsDialogState>({ open: false, details: null });

export function openDetailsDialog(details: BlockDetails): void {
  detailsDialog.open = true;
  detailsDialog.details = details;
}

export function closeDetailsDialog(): void {
  detailsDialog.open = false;
  detailsDialog.details = null;
}
