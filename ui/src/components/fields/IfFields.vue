<script setup lang="ts">
import { computed } from 'vue';
import { ValueBlock } from 'blockstitch';
import { InstructionList } from 'blockstitch';
import { bodyBasePath, fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'If' }>;
  // 'head' renders into InstructionRow.vue's `.wrap-head-line`; 'body' is
  // this component's own `.wrap-mouth` - see the big comment on
  // .instruction-row-wrap in style.css for why a wrap block is a stack of
  // independently-shaped bars around hollow mice, not one filled row.
  part: 'head' | 'body';
}>();

const bodyPath = computed(() => bodyBasePath(props.path, 0));
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">if</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
    <span class="instruction-label">then</span>
  </template>
  <div v-else class="wrap-mouth">
    <InstructionList :strand-id="strandId" :base-path="bodyPath" :instructions="instruction.body" />
  </div>
</template>
