<script setup lang="ts">
// Sidebar prefab for one user-defined `ReturnsValue`/`ReturnsBool` custom block —
// same role as PaletteValueBlock.vue, but keyed by `BlockDef` instead of a
// fixed `ValueKind` string (see blockDefs.ts's header comment for why this
// can't just be another entry in that registry-driven component).
import { computed } from 'vue';
import type { BlockDefDto } from '../types';
import { blockInputNames } from '../types';
import { paletteCallArgs } from '../blockDefs';
import { beginValuePaletteDrag, paletteEvalPreview, PaletteNumberField } from 'blockstitch';
import { openMyBlockMenu } from '../contextMenu';

const props = defineProps<{ def: BlockDefDto }>();

const inputNames = computed(() => blockInputNames(props.def));
const kind = computed(() => `Call:${props.def.id}` as const);

const preview = computed(() => (paletteEvalPreview.value?.kind === kind.value ? paletteEvalPreview.value : null));

function onPointerDown(e: PointerEvent) {
  if ((e.target as Element | null)?.closest?.('input')) return;
  beginValuePaletteDrag(e, kind.value, e.currentTarget as HTMLElement);
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  openMyBlockMenu(e, props.def.id);
}
</script>

<template>
  <span
    class="value-block palette-prefab"
    :class="def.shape === 'ReturnsBool' ? 'value-card-shape-bool' : 'value-card-shape'"
    @pointerdown="onPointerDown"
    @contextmenu="onContextMenu"
  >
    <template v-for="piece in def.pieces" :key="piece.kind === 'Label' ? piece.text : piece.name">
      <span v-if="piece.kind === 'Label'" class="value-op">{{ piece.text }}</span>
      <!-- Boolean inputs have no editable palette leaf — see
           PaletteCallBlock.vue's matching case. -->
      <span v-else-if="piece.value_type === 'Bool'" class="value-block value-hex-blank">
        <span class="value-op value-hex-blank-spacer">&nbsp;</span>
      </span>
      <PaletteNumberField
        v-else
        :model-value="paletteCallArgs[def.id]?.[inputNames.indexOf(piece.name)] ?? { kind: 'Number', value: 0 }"
        @update:model-value="v => { if (paletteCallArgs[def.id]) paletteCallArgs[def.id][inputNames.indexOf(piece.name)] = v; }"
      />
    </template>
    <span v-if="preview" class="value-eval-tooltip" :class="{ 'value-eval-tooltip-error': preview.error }">{{ preview.text }}</span>
  </span>
</template>
