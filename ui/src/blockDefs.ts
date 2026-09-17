// Ephemeral, per-macro palette state for "My Blocks" prefabs - the
// live-editable arg list a CallBlock/Call prefab carries in the sidebar.
// Unlike paletteState.ts's fixed operator registry, custom blocks are
// per-macro and dynamic, so this stays synced with `block_defs` (keyed by
// block id, resized when inputs change) rather than a fixed Record.
import { reactive, watch } from 'vue';
import { state } from './store';
import { numberValue, blockInputPieces, findBlockDef, newId } from './types';
import type { BlockPieceDto, InstructionDto, ValueDto } from './types';

export const paletteCallArgs = reactive<Record<string, ValueDto[]>>({});

/** A fresh blank value for an input slot, matching its declared type - a
 * boolean input defaults to an empty hexagon (`{ kind: 'Bool' }`, same as a
 * built-in boolean slot), everything else to a plain `0`. */
function blankArgFor(piece: Extract<BlockPieceDto, { kind: 'Input' }> | undefined): ValueDto {
  return piece?.value_type === 'Bool' ? { kind: 'Bool' } : numberValue(0);
}

watch(
  () => state.current_macro?.block_defs,
  defs => {
    const list = defs ?? [];
    const ids = new Set(list.map(d => d.id));
    for (const id of Object.keys(paletteCallArgs)) {
      if (!ids.has(id)) delete paletteCallArgs[id];
    }
    for (const def of list) {
      const inputs = blockInputPieces(def);
      const existing = paletteCallArgs[def.id] ?? [];
      paletteCallArgs[def.id] = inputs.map((piece, i) => existing[i] ?? blankArgFor(piece));
    }
  },
  { immediate: true, deep: true },
);

function currentArgs(blockId: string): ValueDto[] {
  const def = findBlockDef(state.current_macro, blockId);
  const inputs = def ? blockInputPieces(def) : null;
  const count = inputs ? inputs.length : (paletteCallArgs[blockId]?.length ?? 0);
  const existing = paletteCallArgs[blockId] ?? [];
  return Array.from({ length: count }, (_, i) => existing[i] ?? blankArgFor(inputs?.[i]));
}

/** The `ValueDto` a "My Blocks" reporter prefab represents - mirrors
 * paletteState.ts's `paletteValueFor`, but for dynamic-arity `Call:<blockId>` kinds. */
export function paletteCallValueFor(blockId: string): ValueDto {
  return { kind: 'Call', block_id: blockId, args: currentArgs(blockId), branches: [], saved: numberValue(0) };
}

/** The `InstructionDto` a "My Blocks" command prefab represents -
 * counterpart to `paletteCallValueFor`, for per-block-id `CallBlock`s. */
export function paletteCallInstructionFor(blockId: string): InstructionDto {
  return { id: newId(), type: 'CallBlock', block_id: blockId, args: currentArgs(blockId) };
}
