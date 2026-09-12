import { reactive } from 'vue';
import { setListEditorState } from './tauri';
import type { ListDto } from './types';

export interface ListEditorPosition { x: number; y: number }

/** Client-side positions for the selected macro's visible list monitors.
 * Their source of truth is persisted on each `ListDef`; this reactive copy
 * keeps drag feedback immediate while the backend save completes. */
export const listEditors = reactive<Record<string, ListEditorPosition>>({});
let activeMacroId: string | null = null;

function normalizedPosition(position: ListEditorPosition): ListEditorPosition {
  return { x: Math.max(0, Math.round(position.x)), y: Math.max(0, Math.round(position.y)) };
}

function save(name: string, visible: boolean, position: ListEditorPosition): void {
  void setListEditorState(name, visible, position.x, position.y).catch(error => {
    console.error('Failed to save list editor state:', error);
  });
}

/** Hydrate visible editors whenever a different macro becomes selected. */
export function activateListEditors(macroId: string | null, lists: readonly ListDto[]): void {
  if (macroId === activeMacroId) return;
  activeMacroId = macroId;
  for (const name of Object.keys(listEditors)) delete listEditors[name];
  for (const list of lists) {
    if (!list.editor_visible) continue;
    listEditors[list.name] = normalizedPosition({ x: list.editor_x, y: list.editor_y });
  }
}

export function isListEditorOpen(name: string): boolean {
  return name in listEditors;
}

export function setListEditorOpen(name: string, open: boolean): void {
  const position = listEditors[name] ?? { x: 36 + Object.keys(listEditors).length * 24, y: 36 + Object.keys(listEditors).length * 24 };
  if (open) listEditors[name] = normalizedPosition(position);
  else delete listEditors[name];
  save(name, open, normalizedPosition(position));
}

/** Removes an editor locally when its list itself has been deleted. */
export function forgetListEditor(name: string): void {
  delete listEditors[name];
}

/** Commit the most recent drag position without changing visibility. */
export function saveListEditorPosition(name: string): void {
  const position = listEditors[name];
  if (!position) return;
  const normalized = normalizedPosition(position);
  listEditors[name] = normalized;
  save(name, true, normalized);
}

export function renameListEditor(oldName: string, newName: string): void {
  const position = listEditors[oldName];
  if (!position) return;
  delete listEditors[oldName];
  listEditors[newName] = position;
}
