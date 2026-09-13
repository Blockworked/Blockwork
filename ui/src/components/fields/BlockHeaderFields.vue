<script setup lang="ts">
// The hat row for a custom block's own body strand - renders its prototype
// (labels as plain text, one draggable input oval per declared parameter).
// Dragging an oval reuses PaletteValueBlock's existing `Var:`-style prefab
// drag machinery wholesale (see `ValueKind`'s `Param:${string}` case in
// types.ts/paletteState.ts) with kind `Param:<name>` - the only new part is
// *where* it renders (here, not the sidebar), since a param reporter is
// only meaningful within its own block's body. `dragKind` additionally packs
// this block's own id in (`Param:<blockId>:<name>`, parsed by
// types.ts's parseParamKind) purely so blockstitchSetup.ts's
// `createFloatingValue` can record it if this drag ends up parked as a
// floating value with no strand of its own to trace back to - see
// `paramIsBool` there.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { Blocks } from 'lucide-vue-next';
import { state } from '../../store';
import { blockBranchPieces, findBlockDef } from '../../types';
import { PaletteValueBlock } from 'blockstitch';
import BranchHeaderPiece from './BranchHeaderPiece.vue';
import type { InstrPath, InstructionDto, ValueKind } from '../../types';

const props = defineProps<{ strandId: string; path: InstrPath; instruction: Extract<InstructionDto, { type: 'BlockHeader' }> }>();

const def = computed(() => findBlockDef(state.current_macro, props.instruction.block_id));
const titlePieces = computed(() => {
  const pieces = def.value?.pieces ?? [];
  const firstBranch = pieces.findIndex(piece => piece.kind === 'Branch');
  if (firstBranch < 0) return pieces;
  // Inputs always belong to the caller's head, even if the user added or
  // moved them after a branch while editing the prototype. Labels after the
  // first branch belong to the bars between mouths instead.
  return pieces.filter((piece, index) => index < firstBranch || piece.kind === 'Input');
});
const branches = computed(() => def.value ? blockBranchPieces(def.value) : []);
// The ordinary boolean reporter derives the side angle from its height. The
// definition header embeds a reporter preview directly, so measure it here
// and provide the same value to the shared shape styles.
const callerPreviewEl = ref<HTMLElement | null>(null);
const callerPreviewHeight = ref(25);
let callerPreviewObserver: ResizeObserver | undefined;

onMounted(() => {
  callerPreviewObserver = new ResizeObserver(() => {
    const element = callerPreviewEl.value;
    if (element) callerPreviewHeight.value = element.offsetHeight;
  });
  if (callerPreviewEl.value) callerPreviewObserver.observe(callerPreviewEl.value);
});

onBeforeUnmount(() => callerPreviewObserver?.disconnect());
function separatorAfter(index: number): string | null {
  const pieces = def.value?.pieces ?? [];
  let seen = -1;
  for (let i = 0; i < pieces.length; i++) {
    if (pieces[i].kind !== 'Branch') continue;
    seen++;
    if (seen === index) {
      const label = pieces.slice(i + 1).find(piece => piece.kind === 'Label');
      return label?.kind === 'Label' ? label.text : null;
    }
  }
  return null;
}

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
  <div v-else-if="branches.length" class="block-header-prototype block-header-prototype-branched">
    <div
      ref="callerPreviewEl"
      class="block-header-caller-shell"
      :class="def.shape === 'ReturnsBool' ? 'value-block value-card-shape-bool blockwork-custom-value-block blockwork-branch-reporter' : undefined"
      :style="def.shape === 'ReturnsBool'
        ? { '--blockwork-custom-block-color': def.color, '--blockstitch-bh': `${callerPreviewHeight}px` }
        : undefined"
    >
      <div
        class="instruction-row instruction-row-wrap blockwork-custom-block block-header-caller-preview"
        :style="{ '--blockwork-custom-block-color': def.color }"
      >
        <div class="wrap-head-line">
          <Blocks class="instruction-type-icon-inline" />
          <template v-for="(piece, i) in titlePieces" :key="i">
            <span v-if="piece.kind === 'Label'" class="instruction-label block-header-label">{{ piece.text }}</span>
            <PaletteValueBlock
              v-else-if="piece.kind === 'Input'"
              class="blockwork-custom-value-block"
              :style="{ '--blockwork-custom-block-color': def.color }"
              :kind="paramKind(piece.name)"
              :drag-kind="paramDragKind(piece.name)"
              :bool-override="piece.value_type === 'Bool'"
            />
          </template>
        </div>
        <template v-for="(branch, i) in branches" :key="branch.id">
          <div v-if="i > 0" class="wrap-mid-bar">
            <span v-if="separatorAfter(i - 1)" class="instruction-label">{{ separatorAfter(i - 1) }}</span>
          </div>
          <div class="wrap-mouth block-header-branch-mouth">
            <BranchHeaderPiece :block-id="instruction.block_id" :name="branch.name" :color="def.color" />
          </div>
        </template>
        <div class="wrap-foot-bar" />
      </div>
    </div>
  </div>
  <div v-else class="block-header-prototype">
    <div class="block-header-title">
      <template v-for="(piece, i) in titlePieces" :key="i">
        <span v-if="piece.kind === 'Label'" class="instruction-label block-header-label">{{ piece.text }}</span>
        <PaletteValueBlock
          v-else-if="piece.kind === 'Input'"
          class="blockwork-custom-value-block"
          :style="{ '--blockwork-custom-block-color': def.color }"
          :kind="paramKind(piece.name)"
          :drag-kind="paramDragKind(piece.name)"
          :bool-override="piece.value_type === 'Bool'"
        />
      </template>
    </div>
  </div>
</template>
