<script setup lang="ts">
// A callback parameter in a definition header. Dragging it out creates the
// stack-shaped RunBranch block that invokes this call site's matching mouth.
import { beginPaletteDrag } from 'blockstitch';
import { GitBranch } from 'lucide-vue-next';

const props = defineProps<{ blockId: string; name: string; color: string; editable?: boolean }>();

function onPointerDown(e: PointerEvent) {
  if (props.editable) return;
  const el = e.currentTarget as HTMLElement;
  beginPaletteDrag(e, 'RunBranch', el.cloneNode(true) as HTMLElement, `${props.blockId}:${props.name}`);
}
</script>

<template>
  <div
    class="instruction-row blockwork-branch-header-piece"
    :style="{ '--blockwork-custom-block-color': color }"
    :title="editable ? undefined : 'Drag into this definition to run this branch'"
    @pointerdown="onPointerDown"
  >
    <div class="instruction-shape">
      <GitBranch class="instruction-type-icon" />
      <div class="instruction-content"><slot><span class="instruction-label">{{ name }}</span></slot></div>
    </div>
  </div>
</template>
