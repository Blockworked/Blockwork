import { invoke, listen, getVersion } from './bridge';
import type { AppEntryDto, BlockPieceDto, BlockShapeDto, HotkeyActionDto, ImportPromptDto, InstrPath, InstructionDto, StateDto, ValueDto, ValueKind, ValueLocationDto } from './types';

export function getState(): Promise<StateDto> {
  return invoke('get_state');
}

export function onStateUpdated(cb: (state: StateDto) => void): Promise<() => void> {
  return listen<StateDto>('state-updated', evt => cb(evt.payload));
}

export function getAppVersion(): Promise<string> {
  return getVersion();
}

// ─── Macro CRUD ─────────────────────────────────────────────────────────────
export const selectMacro = (index: number) => invoke<void>('select_macro', { index });
export const newMacro = () => invoke<void>('new_macro');
export const removeMacro = () => invoke<void>('remove_macro');
export const setTitle = (title: string) => invoke<void>('set_title', { title });
export const setMacroSpeedMultiplier = (multiplier: number) =>
  invoke<void>('set_macro_speed_multiplier', { multiplier });
export const setMacroAlwaysListen = (enabled: boolean) =>
  invoke<void>('set_macro_always_listen', { enabled });
export const saveMacro = () => invoke<void>('save_macro');
export const exportMacro = (macroId: string) => invoke<void>('export_macro', { macroId });
/** Resolves the prompt to show the user before the staged import can be
 * committed (a Command warning and/or non-default settings to confirm —
 * see `ImportPromptDto`), or `null` if the dialog was cancelled or the
 * macro imported immediately with nothing to confirm. */
export const importMacro = () => invoke<ImportPromptDto | null>('import_macro');
/** `keepSettings` maps each `ImportPromptDto.custom_settings` entry's `key`
 * to whether to keep its imported (non-default) value (`true`) or reset it
 * to the default (`false`). */
export const confirmImportMacro = (keepSettings: Record<string, boolean>) =>
  invoke<void>('confirm_import_macro', { keepSettings });
export const cancelImportMacro = () => invoke<void>('cancel_import_macro');
export const createVariable = (name: string) => invoke<void>('create_variable', { name });
export const renameVariable = (oldName: string, newName: string) =>
  invoke<void>('rename_variable', { oldName, newName });
export const deleteVariable = (name: string) => invoke<void>('delete_variable', { name });
export const createBlock = (pieces: BlockPieceDto[], shape: BlockShapeDto, color: string) =>
  invoke<string>('create_block', { pieces, shape, color });
export const editBlock = (blockId: string, pieces: BlockPieceDto[], shape: BlockShapeDto, color: string) =>
  invoke<void>('edit_block', { blockId, pieces, shape, color });
export const deleteBlock = (blockId: string) => invoke<void>('delete_block', { blockId });

// ─── Instructions ───────────────────────────────────────────────────────────
export const addInstruction = (strandId: string, path: InstrPath, instruction: InstructionDto) =>
  invoke<void>('add_instruction', { strandId, path, instruction });
export const editInstruction = (strandId: string, path: InstrPath, instruction: InstructionDto) =>
  invoke<void>('edit_instruction', { strandId, path, instruction });
export const editValueField = (location: ValueLocationDto, text: string) =>
  invoke<void>('edit_value_field', { location, text });
export const setValueKind = (location: ValueLocationDto, kind: ValueKind) =>
  invoke<void>('set_value_kind', { location, kind });
export const takeValue = (location: ValueLocationDto) =>
  invoke<ValueDto>('take_value', { location });
export const putValue = (location: ValueLocationDto, value: ValueDto) =>
  invoke<void>('put_value', { location, value });
export const previewValue = (value: ValueDto) => invoke<string>('preview_value', { value });
export const createFloatingValue = (x: number, y: number, value: ValueDto, originBlockId: string | null) =>
  invoke<string>('create_floating_value', { x, y, value, originBlockId });
export const moveFloatingValue = (floatingId: string, x: number, y: number) =>
  invoke<void>('move_floating_value', { floatingId, x, y });
export const removeFloatingValue = (floatingId: string) =>
  invoke<void>('remove_floating_value', { floatingId });

// ─── Comments (floating/attached notes) ────────────────────────────────────
export const createComment = (x: number, y: number, text: string) =>
  invoke<string>('create_comment', { x, y, text });
export const createAttachedComment = (instructionId: string, dx: number, dy: number, text: string) =>
  invoke<string>('create_attached_comment', { instructionId, dx, dy, text });
export const moveComment = (commentId: string, x: number, y: number) =>
  invoke<void>('move_comment', { commentId, x, y });
export const removeComment = (commentId: string) =>
  invoke<void>('remove_comment', { commentId });
export const editCommentText = (commentId: string, text: string) =>
  invoke<void>('edit_comment_text', { commentId, text });
export const setCommentCollapsed = (commentId: string, collapsed: boolean) =>
  invoke<void>('set_comment_collapsed', { commentId, collapsed });
export const removeInstruction = (strandId: string, path: InstrPath) =>
  invoke<void>('remove_instruction', { strandId, path });
export const reorderInstruction = (strandId: string, path: InstrPath, direction: number) =>
  invoke<void>('reorder_instruction', { strandId, path, direction });
export const clearInstructions = () => invoke<void>('clear_instructions');

// ─── Undo/redo ──────────────────────────────────────────────────────────────
export const undo = () => invoke<void>('undo');
export const redo = () => invoke<void>('redo');

// ─── Strands ────────────────────────────────────────────────────────────────
export const addStrand = (x: number | null, y: number | null, instruction: InstructionDto | null) =>
  invoke<string>('add_strand', { x, y, instruction });
export const removeStrand = (strandId: string) => invoke<void>('remove_strand', { strandId });
export const moveStrand = (strandId: string, x: number, y: number) =>
  invoke<void>('move_strand', { strandId, x, y });
export const splitStrand = (strandId: string, path: InstrPath, x: number, y: number) =>
  invoke<string>('split_strand', { strandId, path, x, y });
export const mergeStrand = (draggedId: string, targetId: string, path: InstrPath) =>
  invoke<void>('merge_strand', { draggedId, targetId, path });
export const deleteInstruction = (strandId: string, path: InstrPath, x: number, y: number) =>
  invoke<string | null>('delete_instruction', { strandId, path, x, y });
export const pasteInstructions = (x: number, y: number, instructions: InstructionDto[]) =>
  invoke<string>('paste_instructions', { x, y, instructions });
export const setRecordingTarget = (strandId: string) =>
  invoke<void>('set_recording_target', { strandId });

// ─── Key capture ────────────────────────────────────────────────────────────
export const startKeyCapture = (strandId: string, path: InstrPath) =>
  invoke<void>('start_key_capture', { strandId, path });
export const startStandaloneKeyCapture = () => invoke<void>('start_standalone_key_capture');
export const keyCaptureEvent = (code: string, key: string) =>
  invoke<void>('key_capture_event', { code, key });
export const clearStandaloneKeyCapture = () => invoke<void>('clear_standalone_key_capture');

// ─── Run / record / loop ────────────────────────────────────────────────────
export const runMacro = () => invoke<void>('run_macro');
export const toggleLoopMode = (enabled: boolean) => invoke<void>('toggle_loop_mode', { enabled });
export const setGlobalSpeedMultiplier = (multiplier: number) =>
  invoke<void>('set_global_speed_multiplier', { multiplier });
export const startRecording = () => invoke<void>('start_recording');
export const stopRecording = () => invoke<void>('stop_recording');
export const toggleRecordMouseRelative = (relative: boolean) =>
  invoke<void>('toggle_record_mouse_relative', { relative });
export const toggleRecordMouseMovement = (enabled: boolean) =>
  invoke<void>('toggle_record_mouse_movement', { enabled });

// ─── Settings navigation ────────────────────────────────────────────────────
export const openSettings = () => invoke<void>('open_settings');
export const closeSettings = () => invoke<void>('close_settings');

// ─── View ───────────────────────────────────────────────────────────────────
/** Resets Chromium page zoom to 100% (see `reset_zoom` — Ctrl/Cmd+0 can't
 * rely on the browser's own accelerator in this CEF runtime). */
export const resetZoom = () => invoke<void>('reset_zoom');

// ─── Hotkeys ────────────────────────────────────────────────────────────────
export const startComboCapture = (action: HotkeyActionDto) =>
  invoke<void>('start_combo_capture', { action });
export const startPendingComboCapture = () => invoke<void>('start_pending_combo_capture');
export const comboCaptureEvent = (code: string, modifiers: number) =>
  invoke<void>('combo_capture_event', { code, modifiers });
export const cancelComboCapture = () => invoke<void>('cancel_combo_capture');
export const setPendingMacroIdx = (index: number | null) =>
  invoke<void>('set_pending_macro_idx', { index });
export const addMacroHotkey = () => invoke<void>('add_macro_hotkey');
export const removeHotkeyBinding = (index: number) => invoke<void>('remove_hotkey_binding', { index });
export const clearNamedHotkey = (action: HotkeyActionDto) => invoke<void>('clear_named_hotkey', { action });
export const resetHotkeyToDefault = (action: HotkeyActionDto) =>
  invoke<void>('reset_hotkey_to_default', { action });

// ─── TCP / IPC server ───────────────────────────────────────────────────────
export const setIpcPortText = (text: string) => invoke<void>('set_ipc_port_text', { text });
export const startIpcServer = () => invoke<void>('start_ipc_server');
export const stopIpcServer = () => invoke<void>('stop_ipc_server');
export const setIpcAutoStart = (enabled: boolean) => invoke<void>('set_ipc_auto_start', { enabled });

// ─── System tray ────────────────────────────────────────────────────────────
export const setCloseToTray = (enabled: boolean) => invoke<void>('set_close_to_tray', { enabled });

// ─── Updates ────────────────────────────────────────────────────────────────
export const checkForUpdates = () => invoke<void>('check_for_updates');
export const applyUpdate = () => invoke<void>('apply_update');

// ─── Open App picker ────────────────────────────────────────────────────────
export const listInstalledApps = () => invoke<AppEntryDto[]>('list_installed_apps');
