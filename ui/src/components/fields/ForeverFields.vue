<script setup lang="ts">
import { computed } from 'vue';
import { InstructionList } from 'blockstitch';
import { bodyBasePath } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

const props = defineProps<{
  strandId: string;
  path: InstrPath;
  instruction: Extract<InstructionDto, { type: 'Forever' }>;
  // See IfFields.vue's comment on 'head'/'body' - same wrap-block split.
  part: 'head' | 'body';
}>();

const bodyPath = computed(() => bodyBasePath(props.path, 0));
</script>

<template>
  <span v-if="part === 'head'" class="instruction-label">forever</span>
  <div v-else class="wrap-mouth">
    <InstructionList :strand-id="strandId" :base-path="bodyPath" :instructions="instruction.body" />
  </div>
</template>
