<script setup lang="ts">
// Sidebar prefab for one user-defined `Normal`/`Ending`-shaped custom block -
// same role as PaletteInstructionBlock.vue, but keyed by `BlockDef` instead
// of a fixed `InstructionType` (a block's shape is dynamic/per-macro, so
// there's no static FIELD_COMPONENTS entry to look up). Editable in place
// via blockDefs.ts's `paletteCallArgs`, same "this is genuinely what lands
// on the canvas" spirit as every other prefab.
import { computed } from 'vue';
import type { BlockDefDto } from '../types';
import { blockBranchPieces, blockInputNames } from '../types';
import { paletteCallArgs } from '../blockDefs';
import { beginPaletteDrag, PaletteNumberField } from 'blockstitch';
import { openMyBlockMenu } from '../contextMenu';
import { Blocks } from 'lucide-vue-next';

const props = defineProps<{ def: BlockDefDto; previewOnly?: boolean }>();

const inputNames = computed(() => blockInputNames(props.def));
const branches = computed(() => blockBranchPieces(props.def));
const headPieces = computed(() => {
  const firstBranch = props.def.pieces.findIndex(piece => piece.kind === 'Branch');
  if (firstBranch < 0) return props.def.pieces;
  return props.def.pieces.filter((piece, index) => index < firstBranch || piece.kind === 'Input');
});
function separatorAfter(index: number): string | null {
  let seen = -1;
  for (let i = 0; i < props.def.pieces.length; i++) {
    if (props.def.pieces[i].kind !== 'Branch') continue;
    seen++;
    if (seen === index) {
      const label = props.def.pieces.slice(i + 1).find(piece => piece.kind === 'Label');
      return label?.kind === 'Label' ? label.text : null;
    }
  }
  return null;
}

function onPointerDown(e: PointerEvent) {
  if (props.previewOnly) return;
  const target = e.target as Element | null;
  if (target?.closest?.('input, select, textarea, button')) return;
  const el = e.currentTarget as HTMLElement;
  beginPaletteDrag(e, 'CallBlock', el.cloneNode(true) as HTMLElement, props.def.id);
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  openMyBlockMenu(e, props.def.id);
}
</script>

<template>
  <div
    v-if="branches.length"
    class="instruction-row instruction-row-wrap palette-prefab blockwork-custom-block"
    :class="{ 'blockwork-wrap-ending': def.shape === 'Ending' }"
    :style="{ '--blockwork-custom-block-color': def.color }"
    @pointerdown="onPointerDown"
    @contextmenu="onContextMenu"
  >
    <div class="wrap-head-line">
      <Blocks class="instruction-type-icon-inline" />
      <template v-for="piece in headPieces" :key="piece.id">
        <span v-if="piece.kind === 'Label'" class="instruction-label">{{ piece.text }}</span>
        <span v-else-if="piece.kind === 'Input' && piece.value_type === 'Bool'" class="value-block value-hex-blank"><span class="value-op value-hex-blank-spacer">&nbsp;</span></span>
        <PaletteNumberField
          v-else-if="piece.kind === 'Input'"
          :model-value="paletteCallArgs[def.id]?.[inputNames.indexOf(piece.name)] ?? { kind: 'Number', value: 0 }"
          @update:model-value="v => { if (paletteCallArgs[def.id]) paletteCallArgs[def.id][inputNames.indexOf(piece.name)] = v; }"
        />
      </template>
    </div>
    <template v-for="(branch, i) in branches" :key="branch.id">
      <div v-if="i > 0" class="wrap-mid-bar"><span class="instruction-label">{{ separatorAfter(i - 1) }}</span></div>
      <div class="wrap-mouth" />
    </template>
    <div class="wrap-foot-bar" />
  </div>
  <div
    v-else
    class="instruction-row palette-prefab blockwork-custom-block"
    :class="{ 'instruction-row-cap': def.shape === 'Ending' }"
    :style="{ '--blockwork-custom-block-color': def.color }"
    @pointerdown="onPointerDown"
    @contextmenu="onContextMenu"
  >
    <div class="instruction-shape">
      <Blocks class="instruction-type-icon" />
      <div class="instruction-content">
        <template v-for="piece in def.pieces" :key="piece.kind === 'Label' ? piece.text : piece.name">
          <span v-if="piece.kind === 'Label'" class="instruction-label">{{ piece.text }}</span>
          <!-- Boolean inputs have no editable palette leaf - a static blank
               hexagon placeholder, same as a built-in operator's bool arg
               (see paletteState.ts's paletteValueFor) and a real unfilled
               boolean slot. -->
          <span v-else-if="piece.kind === 'Input' && piece.value_type === 'Bool'" class="value-block value-hex-blank">
            <span class="value-op value-hex-blank-spacer">&nbsp;</span>
          </span>
          <PaletteNumberField
            v-else-if="piece.kind === 'Input'"
            :model-value="paletteCallArgs[def.id]?.[inputNames.indexOf(piece.name)] ?? { kind: 'Number', value: 0 }"
            @update:model-value="v => { if (paletteCallArgs[def.id]) paletteCallArgs[def.id][inputNames.indexOf(piece.name)] = v; }"
          />
          <span v-else class="instruction-label">{{ piece.name }}</span>
        </template>
      </div>
    </div>
  </div>
</template>
