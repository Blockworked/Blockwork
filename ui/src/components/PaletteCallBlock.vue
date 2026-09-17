<script setup lang="ts">
// Sidebar prefab for one user-defined `Normal`/`Ending`-shaped custom block -
// same role as PaletteInstructionBlock.vue, but keyed by `BlockDef` instead
// of a fixed `InstructionType` (a block's shape is dynamic/per-macro, so
// there's no static FIELD_COMPONENTS entry to look up). Editable in place
// via blockDefs.ts's `paletteCallArgs`, same "this is genuinely what lands
// on the canvas" spirit as every other prefab.
import { computed } from 'vue';
import type { BlockDefDto } from '../types';
import { blockInputNames } from '../types';
import { paletteCallArgs } from '../blockDefs';
import { beginPaletteDrag, PaletteNumberField } from 'blockstitch';
import { openMyBlockMenu } from '../contextMenu';
import { Blocks } from 'lucide-vue-next';

const props = defineProps<{ def: BlockDefDto }>();

const inputNames = computed(() => blockInputNames(props.def));

function onPointerDown(e: PointerEvent) {
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
          <span v-else-if="piece.value_type === 'Bool'" class="value-block value-hex-blank">
            <span class="value-op value-hex-blank-spacer">&nbsp;</span>
          </span>
          <PaletteNumberField
            v-else
            :model-value="paletteCallArgs[def.id]?.[inputNames.indexOf(piece.name)] ?? { kind: 'Number', value: 0 }"
            @update:model-value="v => { if (paletteCallArgs[def.id]) paletteCallArgs[def.id][inputNames.indexOf(piece.name)] = v; }"
          />
        </template>
      </div>
    </div>
  </div>
</template>
