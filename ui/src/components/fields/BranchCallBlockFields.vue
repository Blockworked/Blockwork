<script setup lang="ts">
// Dynamic C-block face for a custom call with branch parameters. Labels can
// appear between branch pieces in the prototype; the corresponding label is
// shown on the mid bar between the two mouths (and more mouths follow in
// declaration order).
import { computed } from 'vue';
import { InstructionList, ValueBlock } from 'blockstitch';
import { blockBranchPieces, bodyBasePath, fieldLocation, findBlockDef } from '../../types';
import { state } from '../../store';
import type { InstrPath, InstructionDto, ValueDto } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'BranchCallBlock' }>; part: 'head' | 'body' }>();
const def = computed(() => findBlockDef(state.current_macro, props.instruction.block_id));
const headPieces = computed(() => {
  if (!def.value) return [];
  let argIndex = 0;
  const firstBranch = def.value.pieces.findIndex(piece => piece.kind === 'Branch');
  return def.value.pieces.flatMap((piece, index) => {
    if (piece.kind === 'Branch') return [];
    if (piece.kind === 'Label' && firstBranch >= 0 && index > firstBranch) return [];
    return [piece.kind === 'Input'
      ? { piece, argIndex: argIndex++, fallback: (piece.value_type === 'Bool' ? { kind: 'Bool' } : { kind: 'Number', value: 0 }) as ValueDto }
      : { piece, argIndex: -1, fallback: { kind: 'Number', value: 0 } as ValueDto }];
  });
});
const branches = computed(() => def.value ? blockBranchPieces(def.value) : []);
function betweenLabel(index: number): string | null {
  const pieces = def.value?.pieces ?? [];
  let seen = -1;
  for (let i = 0; i < pieces.length; i++) {
    if (pieces[i].kind !== 'Branch') continue;
    seen++;
    if (seen === index) {
      const next = pieces.slice(i + 1).find(p => p.kind === 'Label');
      return next?.kind === 'Label' ? next.text : null;
    }
  }
  return null;
}
</script>

<template>
  <template v-if="part === 'head'">
    <span v-if="!def" class="instruction-label">(deleted block)</span>
    <template v-else v-for="(item, i) in headPieces" :key="i">
      <span v-if="item.piece.kind === 'Label'" class="instruction-label">{{ item.piece.text }}</span>
      <ValueBlock v-else :location="fieldLocation(strandId, path, `CallArg:${item.argIndex}`)" :value="instruction.args[item.argIndex] ?? item.fallback" />
    </template>
  </template>
  <template v-else>
    <template v-for="(branch, i) in branches" :key="branch.id">
      <div v-if="i > 0" class="wrap-mid-bar"><span class="instruction-label">{{ betweenLabel(i - 1) }}</span></div>
      <div class="wrap-mouth"><InstructionList :strand-id="strandId" :base-path="bodyBasePath(path, i)" :instructions="instruction.branches[i] ?? []" /></div>
    </template>
  </template>
</template>
