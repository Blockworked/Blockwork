import { reactive, ref } from 'vue';
import { emptyState, type StateDto } from './types';
import { getAppVersion, getState, onStateUpdated } from './tauri';

// Reactive backend state snapshot.
export const state = reactive<StateDto>(emptyState());
export const appVersion = ref('');

let initialized = false;

export async function initState(): Promise<void> {
  if (initialized) return;
  initialized = true;

  try {
    appVersion.value = await getAppVersion();
  } catch (e) {
    console.error('Failed to get app version:', e);
  }

  try {
    const s = await getState();
    Object.assign(state, s);
  } catch (e) {
    console.error('Failed to get initial state:', e);
  }

  try {
    await onStateUpdated(s => {
      Object.assign(state, s);
    });
  } catch (e) {
    console.error('Failed to subscribe to state updates:', e);
  }
}
