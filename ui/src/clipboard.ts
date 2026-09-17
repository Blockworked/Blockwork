// In-memory copy/paste clipboard (not the OS clipboard).
import type { InstrPath, InstructionDto, StrandDto } from './types';
import { resolveInstructionAt, resolveInstructionList } from './types';

let copied: InstructionDto[] | null = null;

export function copyBlock(strand: StrandDto, path: InstrPath): void {
  const ins = resolveInstructionAt(strand, path);
  if (ins) copied = [ins];
}

export function copyAll(strand: StrandDto, path: InstrPath): void {
  if (path.length === 0) return;
  const list = resolveInstructionList(strand, path.slice(0, -1));
  copied = list.slice(path[path.length - 1].index);
}

export function hasClipboard(): boolean {
  return copied !== null && copied.length > 0;
}

export function clipboardContents(): InstructionDto[] | null {
  return copied;
}
