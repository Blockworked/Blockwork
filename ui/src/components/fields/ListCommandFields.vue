<script setup lang="ts">
import { computed } from 'vue';
import { AppDropdown, ValueBlock } from 'blockstitch';
import { state } from '../../store';
import { editInstruction } from '../../tauri';
import { fieldLocation, sortedListNames } from '../../types';
import type { InstrPath, InstructionDto } from '../../types';

type ListCommand = Extract<InstructionDto, { type: 'AddToList' | 'DeleteOfList' | 'DeleteAllOfList' | 'ShiftList' | 'InsertIntoList' | 'ReplaceItemOfList' | 'ReverseList' }>;
const props = defineProps<{ strandId: string; path: InstrPath; instruction: ListCommand }>();
const listNames = computed(() => sortedListNames(state.current_macro));

function setName(name: string) { editInstruction(props.strandId, props.path, { ...props.instruction, name } as InstructionDto); }
</script>

<template>
  <template v-if="instruction.type === 'AddToList'">
    <span class="instruction-label">add</span><ValueBlock :location="fieldLocation(strandId, path, 'AddToListValue')" :value="instruction.value" />
    <span class="instruction-label">to</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
  </template>
  <template v-else-if="instruction.type === 'DeleteOfList'">
    <span class="instruction-label">delete</span><ValueBlock :location="fieldLocation(strandId, path, 'DeleteOfListIndex')" :value="instruction.index" />
    <span class="instruction-label">of</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
  </template>
  <template v-else-if="instruction.type === 'DeleteAllOfList'">
    <span class="instruction-label">delete all of</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
  </template>
  <template v-else-if="instruction.type === 'ShiftList'">
    <span class="instruction-label">shift</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
    <span class="instruction-label">by</span><ValueBlock :location="fieldLocation(strandId, path, 'ShiftListAmount')" :value="instruction.amount" />
  </template>
  <template v-else-if="instruction.type === 'InsertIntoList'">
    <span class="instruction-label">insert</span><ValueBlock :location="fieldLocation(strandId, path, 'InsertIntoListValue')" :value="instruction.value" />
    <span class="instruction-label">at</span><ValueBlock :location="fieldLocation(strandId, path, 'InsertIntoListIndex')" :value="instruction.index" />
    <span class="instruction-label">of</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
  </template>
  <template v-else-if="instruction.type === 'ReplaceItemOfList'">
    <span class="instruction-label">replace item</span><ValueBlock :location="fieldLocation(strandId, path, 'ReplaceItemOfListIndex')" :value="instruction.index" />
    <span class="instruction-label">of</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
    <span class="instruction-label">with</span><ValueBlock :location="fieldLocation(strandId, path, 'ReplaceItemOfListValue')" :value="instruction.value" />
  </template>
  <template v-else>
    <span class="instruction-label">reverse</span><AppDropdown :options="listNames" :model-value="instruction.name" placeholder="list" class-name="dd-compact" @update:model-value="setName" />
  </template>
</template>
