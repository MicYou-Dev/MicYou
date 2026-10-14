import { ref } from 'vue';
import { useStorage } from '@vueuse/core';
import { autostart } from '@/platform';

// Window and startup preferences. The main window reads the same storage
// keys and follows changes through storage events.
const closeBehavior = useStorage<'ask' | 'hide' | 'exit' | null>('micyou_remember_close_action', null);
const startMinimized = useStorage('micyou_start_minimized', false);
const notificationsEnabled = useStorage('micyou_notifications', true);
const autoStream = useStorage('micyou_auto_stream', false);
const pocketMode = useStorage('micyou_pocket_mode', false);
const autostartEnabled = ref(false);

async function refreshAutostart() {
  try {
    autostartEnabled.value = await autostart.isEnabled();
  } catch (e) {
    console.error('Failed to read autostart state:', e);
  }
}

async function setAutostart(enabled: boolean) {
  try {
    await (enabled ? autostart.enable() : autostart.disable());
    autostartEnabled.value = enabled;
  } catch (e) {
    console.error('Failed to toggle autostart:', e);
  }
}

/** Resets the preferences; resolves false when autostart could not be turned off. */
async function resetAppPrefs(): Promise<boolean> {
  closeBehavior.value = null;
  startMinimized.value = false;
  notificationsEnabled.value = true;
  autoStream.value = false;
  pocketMode.value = false;
  if (!autostartEnabled.value) return true;
  try {
    await autostart.disable();
    autostartEnabled.value = false;
    return true;
  } catch (e) {
    console.error('Failed to disable autostart on reset:', e);
    return false;
  }
}

export function useAppPrefs() {
  return {
    closeBehavior,
    startMinimized,
    notificationsEnabled,
    autoStream,
    pocketMode,
    autostartEnabled,
    refreshAutostart,
    setAutostart,
    resetAppPrefs,
  };
}
