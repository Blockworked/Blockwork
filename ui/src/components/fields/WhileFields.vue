<script setup lang="ts">
import { computed } from 'vue';
import { ValueBlock } from 'blockstitch';
import { InstructionList } from 'blockstitch';
import { bodyBasePath, fieldLocation } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'While' }>;
  // See IfFields.vue's comment on 'head'/'body' - same wrap-block split.
  part: 'head' | 'body';
}>();

const bodyPath = computed(() => bodyBasePath(props.path, 0));
</script>

<template>
  <template v-if="part === 'head'">
    <span class="instruction-label">while</span>
    <ValueBlock :location="fieldLocation(strandId, path, 'Condition')" :value="instruction.condition" />
  </template>
  <div v-else class="wrap-mouth">
    <InstructionList :strand-id="strandId" :base-path="bodyPath" :instructions="instruction.body" />
  </div>
</template>
