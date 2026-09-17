<script setup lang="ts">
// Renders a `CallBlock` instruction's prototype dynamically from its
// `BlockDef` (labels as plain text, one `ValueBlock` per declared input) -
// there's no fixed shape to hardcode, unlike every other instruction's
// *Fields component, since it depends on whichever block this call names.
import { computed } from 'vue';
import { state } from '../../store';
import { fieldLocation, findBlockDef } from '../../types';
import { ValueBlock } from 'blockstitch';
import type { InstrPath, InstructionDto, ValueDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'CallBlock' }> }>();

const def = computed(() => findBlockDef(state.current_macro, props.instruction.block_id));

// Pairs each prototype piece with the arg index it addresses (labels get
// -1 - nothing to look up in `args`) and the blank fallback that index
// should show if `args` came up short (matches blockDefs.ts's blankArgFor -
// a boolean input falls back to an empty hexagon, not a number). Mirrors
// ValueBlock.vue's `callPieces`.
const pieces = computed(() => {
  if (!def.value) return [];
  let argIndex = 0;
  return def.value.pieces.map(piece =>
    piece.kind === 'Input'
      ? { piece, argIndex: argIndex++, fallback: (piece.value_type === 'Bool' ? { kind: 'Bool' } : { kind: 'Number', value: 0 }) as ValueDto }
      : { piece, argIndex: -1, fallback: { kind: 'Number', value: 0 } as ValueDto },
  );
});
</script>

<template>
  <span v-if="!def" class="instruction-label">(deleted block)</span>
  <template v-else v-for="(item, i) in pieces" :key="i">
    <span v-if="item.piece.kind === 'Label'" class="instruction-label">{{ item.piece.text }}</span>
    <ValueBlock
      v-else
      :location="fieldLocation(strandId, path, `CallArg:${item.argIndex}`)"
      :value="instruction.args[item.argIndex] ?? item.fallback"
    />
  </template>
</template>
