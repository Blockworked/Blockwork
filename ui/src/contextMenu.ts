// Right-click context menu state.
import { reactive } from 'vue';
import { registerOpen, unregisterOpen, clientToCanvas } from 'blockstitch';
import type { InstrPath, ValueDto } from './types';

export type ContextMenuType = 'block' | 'canvas' | 'variable' | 'myBlock' | 'paletteInstruction' | 'paletteValue' | 'value';

interface ContextMenuState {
  open: boolean;
  x: number;
  y: number;
  type: ContextMenuType;
  strandId: string;
  path: InstrPath;
  canvasX: number;
  canvasY: number;
  variableName: string;
  blockId: string;
  paletteInstructionType: string;
  paletteVariantId: string | undefined;
  paletteValueKind: string;
  valueNode: ValueDto | null;
}

export const contextMenu = reactive<ContextMenuState>({
  open: false,
  x: 0,
  y: 0,
  type: 'block',
  strandId: '',
  path: [],
  canvasX: 0,
  canvasY: 0,
  variableName: '',
  blockId: '',
  paletteInstructionType: '',
  paletteVariantId: undefined,
  paletteValueKind: '',
  valueNode: null,
});

function openAt(e: MouseEvent) {
  registerOpen(closeContextMenu);
  contextMenu.open = true;
  contextMenu.x = e.clientX;
  contextMenu.y = e.clientY;
}

export function openBlockMenu(e: MouseEvent, strandId: string, path: InstrPath): void {
  openAt(e);
  contextMenu.type = 'block';
  contextMenu.strandId = strandId;
  contextMenu.path = path;
}

export function openCanvasMenu(e: MouseEvent): void {
  openAt(e);
  contextMenu.type = 'canvas';
  const [cx, cy] = clientToCanvas(e.clientX, e.clientY);
  contextMenu.canvasX = cx;
  contextMenu.canvasY = cy;
}

export function openVariableMenu(e: MouseEvent, name: string): void {
  openAt(e);
  contextMenu.type = 'variable';
  contextMenu.variableName = name;
}

export function openMyBlockMenu(e: MouseEvent, blockId: string): void {
  openAt(e);
  contextMenu.type = 'myBlock';
  contextMenu.blockId = blockId;
}

export function openPaletteInstructionMenu(e: MouseEvent, type: string, variantId?: string): void {
  openAt(e);
  contextMenu.type = 'paletteInstruction';
  contextMenu.paletteInstructionType = type;
  contextMenu.paletteVariantId = variantId;
}

export function openPaletteValueMenu(e: MouseEvent, kind: string): void {
  openAt(e);
  contextMenu.type = 'paletteValue';
  contextMenu.paletteValueKind = kind;
}

// Var/Param reporters placed on canvas have no Details to show (they're not
// a fixed block with fixed behavior) - silently declining to open here, same
// as before this menu existed, is better than opening one with nothing in it.
export function openValueMenu(e: MouseEvent, value: ValueDto): void {
  if (value.kind !== 'Op' && value.kind !== 'Call' && value.kind !== 'Number' && value.kind !== 'Text') return;
  openAt(e);
  contextMenu.type = 'value';
  contextMenu.valueNode = value;
}

export function closeContextMenu(): void {
  if (!contextMenu.open) return;
  contextMenu.open = false;
  unregisterOpen(closeContextMenu);
}
