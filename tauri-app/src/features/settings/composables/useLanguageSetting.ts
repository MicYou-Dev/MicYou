import { ref, watch } from 'vue';
import { isLocale, setLocale } from '@/i18n';
import { saveUiPrefs } from '@/features/theme/composables/useTheme';

const stored = localStorage.getItem('micyou_language');
const currentLanguage = ref(isLocale(stored) ? stored : 'system');

watch(currentLanguage, (language) => {
  localStorage.setItem('micyou_language', language);
  setLocale(language);
  // Share the language with the CLI via ui.json
  saveUiPrefs(language);
});

// The macOS app menu lives in the main window, so a language picked there has to
// reach this one. `storage` never fires in the window that wrote the value, which
// is what makes this the only place the two windows can meet.
window.addEventListener('storage', (event) => {
  if (event.key !== 'micyou_language') return;
  const next = event.newValue;
  if (next && isLocale(next) && next !== currentLanguage.value) currentLanguage.value = next;
});

/** The language setting: "system" or a locale key. */
export function useLanguageSetting() {
  return {
    currentLanguage,
    saveUiPrefs: () => saveUiPrefs(currentLanguage.value),
  };
}
