<script setup lang="ts">
// "Choose an App" popup, shared by the OpenApp and CloseApp instructions
// (OpenAppFields.vue/PaletteOpenAppFields.vue and their CloseApp
// counterparts) - a searchable grid of every installed app the backend's
// `list_installed_apps` can find (see src-tauri/src/installed_apps.rs).
// Teleported to <body> like the other dialogs (MacroSettingsDialog.vue, etc.),
// but wider, since it's a grid rather than a form.
import { computed, onMounted, ref } from 'vue';
import { Search, AppWindow } from 'lucide-vue-next';
import { listInstalledApps } from '../tauri';
import type { AppEntryDto } from '../types';

const props = withDefaults(defineProps<{ title?: string }>(), { title: 'Choose an App' });
const emit = defineEmits<{ select: [app: AppEntryDto]; close: [] }>();

const loading = ref(true);
const apps = ref<AppEntryDto[]>([]);
const search = ref('');
const searchInput = ref<HTMLInputElement | null>(null);

onMounted(async () => {
  try {
    apps.value = await listInstalledApps();
  } finally {
    loading.value = false;
  }
  searchInput.value?.focus();
});

const filteredApps = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return apps.value;
  return apps.value.filter(a => a.name.toLowerCase().includes(q));
});

function choose(app: AppEntryDto) {
  emit('select', app);
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-overlay" @pointerdown.self="emit('close')">
      <div class="modal-panel app-selector-panel">
        <h2 class="modal-title">{{ props.title }}</h2>
        <div class="app-selector-search">
          <Search :size="15" />
          <input
            ref="searchInput"
            v-model="search"
            type="text"
            placeholder="Search apps…"
            autocomplete="off"
            spellcheck="false"
            @keydown.esc="emit('close')"
          >
        </div>
        <p v-if="loading" class="app-selector-empty">Loading installed apps…</p>
        <p v-else-if="filteredApps.length === 0" class="app-selector-empty">No apps found.</p>
        <div v-else class="app-selector-grid">
          <button
            v-for="app in filteredApps"
            :key="app.command"
            type="button"
            class="app-selector-item"
            :title="app.name"
            @click="choose(app)"
          >
            <img v-if="app.icon" :src="app.icon" class="app-selector-item-icon" alt="">
            <AppWindow v-else :size="24" class="app-selector-item-icon-fallback" />
            <span class="app-selector-item-name">{{ app.name }}</span>
          </button>
        </div>
        <div class="modal-actions">
          <button type="button" @click="emit('close')">Cancel</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
