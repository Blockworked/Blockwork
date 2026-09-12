<script setup lang="ts">
import { computed } from 'vue';
import { AppDropdown, AutosizeInput, PaletteNumberField } from 'blockstitch';
import { state } from '../../../store';
import { sortedListNames, textValue } from '../../../types';
import type { InstructionDto } from '../../../types';

type ListCommand = Extract<InstructionDto, { type: 'AddToList' | 'DeleteOfList' | 'DeleteAllOfList' | 'ShiftList' | 'InsertIntoList' | 'ReplaceItemOfList' | 'ReverseList' }>;
defineProps<{ instruction: ListCommand }>();
const listNames = computed(() => sortedListNames(state.current_macro));
function setText(target: any, value: string) { target.value = textValue(value); }
</script>

<template>
  <template v-if="instruction.type === 'AddToList'"><span class="instruction-label">add</span><AutosizeInput :model-value="instruction.value.kind === 'Text' ? instruction.value.value : ''" :min-chars="4" @update:model-value="v => setText(instruction, v)" /><span class="instruction-label">to</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /></template>
  <template v-else-if="instruction.type === 'DeleteOfList'"><span class="instruction-label">delete</span><PaletteNumberField v-model="instruction.index" /><span class="instruction-label">of</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /></template>
  <template v-else-if="instruction.type === 'DeleteAllOfList'"><span class="instruction-label">delete all of</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /></template>
  <template v-else-if="instruction.type === 'ShiftList'"><span class="instruction-label">shift</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /><span class="instruction-label">by</span><PaletteNumberField v-model="instruction.amount" /></template>
  <template v-else-if="instruction.type === 'InsertIntoList'"><span class="instruction-label">insert</span><AutosizeInput :model-value="instruction.value.kind === 'Text' ? instruction.value.value : ''" :min-chars="4" @update:model-value="v => setText(instruction, v)" /><span class="instruction-label">at</span><PaletteNumberField v-model="instruction.index" /><span class="instruction-label">of</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /></template>
  <template v-else-if="instruction.type === 'ReplaceItemOfList'"><span class="instruction-label">replace item</span><PaletteNumberField v-model="instruction.index" /><span class="instruction-label">of</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /><span class="instruction-label">with</span><AutosizeInput :model-value="instruction.value.kind === 'Text' ? instruction.value.value : ''" :min-chars="4" @update:model-value="v => setText(instruction, v)" /></template>
  <template v-else><span class="instruction-label">reverse</span><AppDropdown :options="listNames" v-model="instruction.name" placeholder="list" class-name="dd-compact" /></template>
</template>
