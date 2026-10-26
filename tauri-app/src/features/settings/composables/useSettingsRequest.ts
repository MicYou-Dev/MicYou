import { onMounted, onUnmounted, ref } from 'vue';

/**
 * Requests the macOS app menu leaves in shared storage (see `openSettingsAt` in
 * App.vue): a settings section to show, or `sponsors` for that dialog. The menu
 * lives in the main window while settings have their own, so the two cannot call
 * each other directly. A window that is already open picks the request up from
 * the `storage` event; one created for it reads the key on mount.
 */
const SETTINGS_REQUEST_KEY = 'micyou_settings_request';

/** Set when the app menu asked for the sponsors dialog; AboutSection consumes it. */
export const sponsorsRequested = ref(false);

/** Shows `section`, or the about section when the sponsors dialog was asked for. */
export function useSettingsRequest(showSection: (section: string) => void) {
  function apply() {
    const request = localStorage.getItem(SETTINGS_REQUEST_KEY);
    if (!request) return;
    localStorage.removeItem(SETTINGS_REQUEST_KEY);
    if (request === 'sponsors') {
      sponsorsRequested.value = true;
      showSection('about');
      return;
    }
    showSection(request);
  }

  function onStorage(event: StorageEvent) {
    if (event.key === SETTINGS_REQUEST_KEY) apply();
  }

  onMounted(() => {
    apply();
    window.addEventListener('storage', onStorage);
  });
  onUnmounted(() => window.removeEventListener('storage', onStorage));
}
