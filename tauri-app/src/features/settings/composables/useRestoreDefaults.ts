import { ref } from 'vue';
import { i18n } from '@/i18n';
import {
  DEFAULT_THEME,
  resetThemeToDefaults,
  sharedColorMode,
} from '@/features/theme/composables/useTheme';
import { useAppPrefs } from './useAppPrefs';
import { useAudioSettings } from './useAudioSettings';
import { useLanguageSetting } from './useLanguageSetting';

export type RestoreTarget = 'settings' | 'theme';

// In-app confirm + result dialogs, no native confirm()/alert().
const confirmTarget = ref<RestoreTarget | null>(null);
const result = ref<{ title: string; message: string } | null>(null);

const t = (key: string) => i18n.global.t(key);

async function restoreSettings() {
  const { resetAppPrefs } = useAppPrefs();
  const { resetAudioSettings } = useAudioSettings();
  try {
    const autostartReset = await resetAppPrefs();
    await resetAudioSettings();
    result.value = {
      title: t('settings.restoreDefaults.title'),
      message: autostartReset
        ? t('settings.restoreDefaults.success')
        : t('settings.restoreDefaults.autostartFailed'),
    };
  } catch (e) {
    console.error('Failed to restore default settings:', e);
    result.value = {
      title: t('settings.restoreDefaults.title'),
      message: t('settings.restoreDefaults.failed'),
    };
  }
}

async function restoreTheme() {
  const colorMode = sharedColorMode();
  const { saveUiPrefs } = useLanguageSetting();
  try {
    colorMode.value = DEFAULT_THEME.colorMode as typeof colorMode.value;
    // True when removing an installed theme package failed in the backend
    // (the frontend is already reset), so report a partial success.
    const themeRemoveFailed = await resetThemeToDefaults();
    // ui.json stores the theme preset; write it after the reset.
    saveUiPrefs();
    result.value = {
      title: t('settings.restoreTheme.title'),
      message: themeRemoveFailed
        ? t('settings.restoreTheme.partialSuccess')
        : t('settings.restoreTheme.success'),
    };
  } catch (e) {
    console.error('Failed to restore default theme:', e);
    result.value = {
      title: t('settings.restoreTheme.title'),
      message: t('settings.restoreTheme.failed'),
    };
  }
}

export function useRestoreDefaults() {
  return {
    confirmTarget,
    result,
    request: (target: RestoreTarget) => {
      confirmTarget.value = target;
    },
    async confirm() {
      const target = confirmTarget.value;
      confirmTarget.value = null;
      if (target === 'settings') await restoreSettings();
      else if (target === 'theme') await restoreTheme();
    },
  };
}
