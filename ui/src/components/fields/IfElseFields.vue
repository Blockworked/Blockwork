<script setup lang="ts">
import { computed } from 'vue';
import { ValueBlock } from 'blockstitch';
import { InstructionList } from 'blockstitch';
import { bodyBasePath, fieldLocation, isCapType } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'IfElse' }>;
  // See IfFields.vue's header comment - rendered separately for the wrap
  // block's head line vs. its mouth (nested body).
  part: 'head' | 'body';
}>();

const thenPath = computed(() => bodyBasePath(props.path, 0));
const elsePath = computed(() => bodyBasePath(props.path, 1));

// The mid ("else") bar's top notch receives the bottom bump of the then-arm's
// last row. A cap-type row there (Return/EscapeLoop/ContinueLoop) has no
// bump, so the notch would otherwise carve an unfilled hole - see
// `.wrap-bar-flat-notch` in blockstitch's theme CSS. InstructionRow.vue
// handles this the same way for the foot bar, generically via getSlots(); the
// mid bar can't go through that path since it's this component, not
// InstructionRow.vue, that renders it.
const midNotchFlat = computed(() => {
  const body = props.instruction.then_body;
  const last = body[body.length - 1];
  return !!last && isCapType(last.type);
});
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
    <span class="instruction-label">then</span>
  </template>
  <template v-else>
    <div class="wrap-mouth">
      <InstructionList :strand-id="strandId" :base-path="thenPath" :instructions="instruction.then_body" />
    </div>
    <div class="wrap-mid-bar" :class="{ 'wrap-bar-flat-notch': midNotchFlat }"><span class="instruction-label">else</span></div>
    <div class="wrap-mouth">
      <InstructionList :strand-id="strandId" :base-path="elsePath" :instructions="instruction.else_body" />
    </div>
  </template>
</template>
