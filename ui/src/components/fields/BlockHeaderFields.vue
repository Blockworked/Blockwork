<script setup lang="ts">
// The hat row for a custom block's own body strand — renders its prototype
// (labels as plain text, one draggable input oval per declared parameter).
// Dragging an oval reuses PaletteValueBlock's existing `Var:`-style prefab
// drag machinery wholesale (see `ValueKind`'s `Param:${string}` case in
// types.ts/paletteState.ts) with kind `Param:<name>` — the only new part is
// *where* it renders (here, not the sidebar), since a param reporter is
// only meaningful within its own block's body. `dragKind` additionally packs
// this block's own id in (`Param:<blockId>:<name>`, parsed by
// types.ts's parseParamKind) purely so blockstitchSetup.ts's
// `createFloatingValue` can record it if this drag ends up parked as a
// floating value with no strand of its own to trace back to — see
// `paramIsBool` there.
import { computed } from 'vue';
import { Blocks } from 'lucide-vue-next';
import { state } from '../../store';
import { findBlockDef } from '../../types';
import { PaletteValueBlock } from 'blockstitch';
import type { InstrPath, InstructionDto, ValueKind } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'BlockHeader' }> }>();

const def = computed(() => findBlockDef(state.current_macro, props.instruction.block_id));

function paramKind(name: string): ValueKind {
  return `Param:${name}`;
}

function paramDragKind(name: string): string {
  return `Param:${props.instruction.block_id}:${name}`;
}
</script>

<template>
  <Blocks class="custom-block-header-icon" />
  <template v-if="!def">
    <span class="instruction-label">(deleted block)</span>
  </template>
  <template v-else v-for="(piece, i) in def.pieces" :key="i">
    <span v-if="piece.kind === 'Label'" class="instruction-label block-header-label">{{ piece.text }}</span>
    <PaletteValueBlock
      v-else
      class="blockwork-custom-value-block"
      :style="{ '--blockwork-custom-block-color': def.color }"
      :kind="paramKind(piece.name)"
      :drag-kind="paramDragKind(piece.name)"
      :bool-override="piece.value_type === 'Bool'"
    />
  </template>
</template>
