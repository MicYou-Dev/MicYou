import { computed, ref, watch, watchEffect } from 'vue';
import { useColorMode, useStorage } from '@vueuse/core';
import { command, type SystemAccentColor } from '@/platform';
import type { HslColor } from '../types';
import type { ThemeMode } from '../types';

export type { HslColor } from '../types';

export const BUILTIN_THEMES: Record<string, HslColor> = {
  'theme-blue': { h: 215, s: 35, l: 55 },
  'theme-green': { h: 150, s: 30, l: 50 },
  'theme-rose': { h: 350, s: 40, l: 60 },
  'theme-purple': { h: 270, s: 30, l: 60 },
  'theme-orange': { h: 25, s: 40, l: 55 },
  'theme-amber': { h: 40, s: 40, l: 50 },
  'theme-teal': { h: 175, s: 30, l: 45 },
  'theme-cyan': { h: 190, s: 40, l: 45 },
};

const DEFAULT_SYSTEM_COLOR = '#5b7cfa';
const THEME_CLASSES = Object.keys(BUILTIN_THEMES).concat('theme-custom', 'theme-system');
const THEME_COLOR_CLASSES = Object.keys(BUILTIN_THEMES).map((name) => name.replace('theme-', 'theme-color-')).concat('theme-color-custom', 'theme-color-system');
const STYLE_CLASSES = ['style-default', 'style-glass'];

// Single source of truth for theme default values. `useStorage` init and
// resetThemeToDefaults() both reference this — change a default here once.
// `colorMode` is the light/dark preference applied by useTheme().
export const DEFAULT_THEME = {
  colorMode: 'auto',
  themeMode: 'system' as ThemeMode,
  themeColor: 'theme-blue',
  uiStyle: 'style-default',
  customH: 215,
  customS: 35,
  customL: 55,
  customVariant: 'TonalSpot',
  customCss: '',
  customCssEnabled: true,
};

// Versioned keys intentionally avoid silently migrating the previous theme data.
const themeMode = useStorage<ThemeMode>('micyou_theme_v2_mode', DEFAULT_THEME.themeMode);
const themeColor = useStorage<string>('micyou_theme_v2_color', DEFAULT_THEME.themeColor);
const uiStyle = useStorage<string>('micyou_theme_v2_ui_style', DEFAULT_THEME.uiStyle);
const customH = useStorage<number>('micyou_theme_v2_custom_h', DEFAULT_THEME.customH);
const customS = useStorage<number>('micyou_theme_v2_custom_s', DEFAULT_THEME.customS);
const customL = useStorage<number>('micyou_theme_v2_custom_l', DEFAULT_THEME.customL);
const customVariant = useStorage<string>('micyou_theme_v2_variant', DEFAULT_THEME.customVariant);
const customCss = useStorage<string>('micyou_theme_v2_css', DEFAULT_THEME.customCss);
const customCssEnabled = useStorage<boolean>('micyou_theme_v2_css_enabled', DEFAULT_THEME.customCssEnabled);
const installedThemeId = useStorage<string>('micyou_theme_v2_installed_id', '');
const installedThemeCss = useStorage<string>('micyou_theme_v2_installed_css', '');
const installedThemeControlsColor = useStorage<boolean>('micyou_theme_v2_installed_controls_color', true);
const systemAccentHex = useStorage<string>('micyou_theme_v2_system_hex', DEFAULT_SYSTEM_COLOR);
const systemAccentSupported = useStorage<boolean>('micyou_theme_v2_system_supported', false);
const systemAccentSource = useStorage<string>('micyou_theme_v2_system_source', 'fallback');
const systemAccentLoading = ref(false);
const systemAccentInitialized = ref(false);

export function hexToHsl(hex: string): HslColor {
  const normalized = hex.replace('#', '').trim();
  const value = normalized.length === 3
    ? normalized.split('').map((part) => part + part).join('')
    : normalized;
  const red = Number.parseInt(value.slice(0, 2), 16) / 255;
  const green = Number.parseInt(value.slice(2, 4), 16) / 255;
  const blue = Number.parseInt(value.slice(4, 6), 16) / 255;
  if ([red, green, blue].some(Number.isNaN)) return BUILTIN_THEMES['theme-blue'];

  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const delta = max - min;
  let hue = 0;
  const lightness = (max + min) / 2;
  const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));

  if (delta !== 0) {
    if (max === red) hue = 60 * (((green - blue) / delta) % 6);
    else if (max === green) hue = 60 * ((blue - red) / delta + 2);
    else hue = 60 * ((red - green) / delta + 4);
  }

  return {
    h: Math.round((hue + 360) % 360),
    s: Math.round(saturation * 100),
    l: Math.round(lightness * 100),
  };
}

export function generateThemeCSS(baseH: number, baseS: number, baseL: number, variant: string, isDark: boolean): string {
  let priH = baseH, priS = baseS, priL = baseL;
  let secH = baseH, secS = 20, secL = isDark ? 16 : 90;
  let terH = baseH, terS = 20, terL = isDark ? 16 : 90;
  let bgH = baseH, bgS = 15, bgL = isDark ? 8 : 96;
  let surH = baseH, surS = 15, surL = isDark ? 10 : 98;

  switch (variant) {
    case 'Neutral': priS = Math.max(0, baseS - 15); secS = 10; terS = 10; bgS = 5; surS = 5; break;
    case 'Vibrant': priS = Math.min(100, baseS + 20); secS = 30; terS = 35; bgS = 25; surS = 25; break;
    case 'Expressive': secH = (baseH + 45) % 360; terH = (baseH + 90) % 360; surS = 20; break;
    case 'Rainbow': secH = (baseH + 120) % 360; terH = (baseH + 240) % 360; secS = 35; terS = 35; break;
    case 'FruitSalad': secH = (baseH + 60) % 360; terH = (baseH + 150) % 360; priS = Math.min(100, baseS + 10); secS = 30; terS = 30; break;
    case 'Monochrome': priS = 0; secS = 0; terS = 0; bgS = 0; surS = 0; break;
    case 'Fidelity': secS = Math.max(0, baseS - 10); terS = Math.max(0, baseS - 15); surS = Math.max(0, baseS - 20); bgS = Math.max(0, baseS - 25); break;
    case 'Content': secS = Math.max(0, baseS - 5); terS = Math.max(0, baseS - 10); surS = Math.max(0, baseS - 15); bgS = Math.max(0, baseS - 20); break;
  }

  const fgL = isDark ? 85 : 25;
  const onPriL = isDark ? 20 : 92;
  const priContL = isDark ? 25 : 85;
  const onPriContL = isDark ? 85 : 25;
  const onSecL = isDark ? 85 : 25;
  const secContL = isDark ? 16 : 90;
  const onSecContL = isDark ? 85 : 25;
  const surBrightL = isDark ? 14 : 98;
  const surContL = isDark ? 16 : 92;
  const surContLowL = isDark ? 12 : 94;
  const surVarL = isDark ? 22 : 88;
  const onSurVarL = isDark ? 60 : 45;
  const outlineL = isDark ? 20 : 80;

  return `
    --background: ${bgH} ${bgS}% ${bgL}%;
    --foreground: ${surH} ${surS}% ${fgL}%;
    --surface: ${surH} ${surS}% ${surL}%;
    --on-surface: ${surH} ${surS}% ${fgL}%;
    --surface-bright: ${surH} ${surS}% ${surBrightL}%;
    --surface-container: ${surH} ${surS}% ${surContL}%;
    --surface-container-low: ${surH} ${surS}% ${surContLowL}%;
    --surface-variant: ${surH} ${surS}% ${surVarL}%;
    --on-surface-variant: ${surH} ${surS}% ${onSurVarL}%;
    --outline: ${surH} ${surS}% ${outlineL}%;
    --border: ${surH} ${surS}% ${outlineL}%;
    --primary: ${priH} ${priS}% ${isDark ? Math.min(priL + 10, 80) : priL}%;
    --on-primary: ${priH} ${priS}% ${onPriL}%;
    --primary-container: ${priH} ${priS}% ${priContL}%;
    --on-primary-container: ${priH} ${priS}% ${onPriContL}%;
    --secondary: ${secH} ${secS}% ${secL}%;
    --on-secondary: ${secH} ${secS}% ${onSecL}%;
    --secondary-container: ${secH} ${secS}% ${secContL}%;
    --on-secondary-container: ${secH} ${secS}% ${onSecContL}%;
    --tertiary: ${terH} ${terS}% ${terL}%;
    --on-tertiary: ${terH} ${terS}% ${onSecL}%;
    --error: 0 40% ${isDark ? 65 : 55}%;
    --on-error: 0 40% ${isDark ? 20 : 92}%;
  `;
}

function ensureStyle(id: string): HTMLStyleElement {
  let style = document.getElementById(id) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement('style');
    style.id = id;
    document.head.appendChild(style);
  }
  return style;
}

function applyRootClasses() {
  const html = document.documentElement;
  html.classList.remove(...THEME_CLASSES, ...THEME_COLOR_CLASSES, ...STYLE_CLASSES, 'theme-mode-system', 'theme-mode-preset', 'theme-mode-custom', 'system-accent-unavailable');

  const activeColor = themeMode.value === 'system' ? 'theme-system' : themeColor.value;
  html.classList.add(activeColor, `theme-color-${activeColor.replace('theme-', '')}`, `theme-mode-${themeMode.value}`, uiStyle.value);
  if (themeMode.value === 'system' && !systemAccentSupported.value) {
    html.classList.add('system-accent-unavailable');
  }
}

function activeBaseColor(): HslColor {
  if (themeMode.value === 'system') return hexToHsl(systemAccentHex.value || DEFAULT_SYSTEM_COLOR);
  return BUILTIN_THEMES[themeColor.value] || { h: customH.value, s: customS.value, l: customL.value };
}

async function loadSystemAccent() {
  systemAccentLoading.value = true;
  try {
    const result = await command('get_system_accent_color');
    systemAccentHex.value = result.supported && result.hex ? result.hex : DEFAULT_SYSTEM_COLOR;
    systemAccentSupported.value = result.supported;
    systemAccentSource.value = result.source || (result.supported ? 'system' : 'fallback');
  } catch (error) {
    systemAccentHex.value = DEFAULT_SYSTEM_COLOR;
    systemAccentSupported.value = false;
    systemAccentSource.value = 'fallback';
    console.warn('System accent color is unavailable:', error);
  } finally {
    systemAccentLoading.value = false;
  }
}

// The desktop accent can change while the app runs (KDE derives it from the
// wallpaper), so it is re-read whenever the window regains focus.
function refreshSystemAccentOnFocus() {
  if (themeMode.value !== 'system' || systemAccentLoading.value) return;
  void loadSystemAccent();
}

async function initializeSystemAccent() {
  if (systemAccentInitialized.value || systemAccentLoading.value || typeof window === 'undefined') return;
  systemAccentInitialized.value = true;
  window.addEventListener('focus', refreshSystemAccentOnFocus);
  // Picking "system" is the moment the user expects the OS accent, so it is
  // re-read then instead of answering from the value loaded earlier.
  watch(themeMode, (mode) => {
    if (mode === 'system' && !systemAccentLoading.value) void loadSystemAccent();
  });
  await loadSystemAccent();
}

function exportThemeToCli() {
  if (typeof document === 'undefined') return;
  try {
    const read = (name: string): string => {
      const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
      const parts = raw.split(/\s+/).map((part) => Number.parseFloat(part));
      if (parts.length < 3 || parts.some(Number.isNaN)) return '';
      return hslToHex(parts[0], parts[1], parts[2]);
    };
    void command('save_theme_colors', {
      primary: read('--primary') || '#8d8768',
      secondary: read('--secondary') || '#8d8768',
      tertiary: read('--tertiary') || '#8d8768',
      surface: read('--surface') || '#1e1d1a',
      surfaceVariant: read('--surface-variant') || '#2a2824',
      onSurface: read('--on-surface') || '#e7e6e4',
      error: read('--error') || '#d17a7a',
    });
  } catch (error) {
    console.error('export theme colors failed:', error);
  }
}

function hslToHex(h: number, s: number, l: number): string {
  const sn = s / 100;
  const ln = l / 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = sn * Math.min(ln, 1 - ln);
  const f = (n: number) => ln - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  const toHex = (value: number) => Math.round(255 * value).toString(16).padStart(2, '0');
  return `#${toHex(f(0))}${toHex(f(8))}${toHex(f(4))}`;
}

export function activateInstalledTheme(themeId: string, css: string, controlsThemeColor = true) {
  installedThemeId.value = themeId;
  installedThemeCss.value = css;
  installedThemeControlsColor.value = controlsThemeColor;
}

export function clearInstalledTheme() {
  installedThemeId.value = '';
  installedThemeCss.value = '';
  installedThemeControlsColor.value = true;
}

// Reset every theme field to DEFAULT_THEME (single source of truth) and tear
// down any installed theme package. The applyTheme watchEffect recomputes CSS
// + writes theme.json; SettingsWindow calls saveUiPrefs() afterwards to sync
// ui.json. No second copy of defaults lives here.
// Returns true if an installed theme package existed but its backend removal
// failed (frontend is already cleared) — caller surfaces a partial-success.
export async function resetThemeToDefaults(): Promise<boolean> {
  themeMode.value = DEFAULT_THEME.themeMode;
  themeColor.value = DEFAULT_THEME.themeColor;
  uiStyle.value = DEFAULT_THEME.uiStyle;
  customH.value = DEFAULT_THEME.customH;
  customS.value = DEFAULT_THEME.customS;
  customL.value = DEFAULT_THEME.customL;
  customVariant.value = DEFAULT_THEME.customVariant;
  customCss.value = DEFAULT_THEME.customCss;
  customCssEnabled.value = DEFAULT_THEME.customCssEnabled;
  const id = installedThemeId.value;
  if (id) {
    clearInstalledTheme();
    try {
      await command('remove_installed_theme', { themeId: id });
    } catch (e) {
      console.warn('Failed to remove installed theme:', e);
      return true; // installed-theme package removal failed (frontend already cleared)
    }
  }
  return false;
}

/**
 * Shares the effective language and theme color with the CLI/TUI (ui.json).
 * The main window calls it at startup, the settings window on every change.
 */
export function saveUiPrefs(languageSetting = localStorage.getItem('micyou_language') || 'system') {
  const language = languageSetting === 'system'
    ? navigator.language.toLowerCase().startsWith('zh') ? 'zh' : 'en'
    : languageSetting;
  void command('save_ui_prefs', {
    language,
    themeColor: themeMode.value === 'system' ? 'theme-system' : themeColor.value,
  }).catch((e) => console.error('save_ui_prefs failed:', e));
}

// Every window applies the light/dark class itself; the preference is shared
// through localStorage, so a change in the settings window reaches the rest.
// Created once per window, however many components call useTheme().
let colorModeRef: ReturnType<typeof useColorMode> | null = null;
export function sharedColorMode() {
  colorModeRef ??= useColorMode({
    emitAuto: true,
    modes: {
      dark: 'dark',
      light: 'light',
    },
    attribute: 'class',
  });
  return colorModeRef;
}

export function useTheme() {
  void initializeSystemAccent();

  const colorMode = sharedColorMode();

  const systemAccent = computed<SystemAccentColor>(() => ({
    hex: systemAccentHex.value,
    source: systemAccentSource.value,
    supported: systemAccentSupported.value,
  }));

  watchEffect(() => {
    if (typeof document === 'undefined') return;
    applyRootClasses();
    const baseColor = activeBaseColor();
    ensureStyle('micyou-theme-tokens').textContent = `
      :root, :root[class] { ${generateThemeCSS(baseColor.h, baseColor.s, baseColor.l, customVariant.value, false)} }
      :root.dark, html.dark[class] { ${generateThemeCSS(baseColor.h, baseColor.s, baseColor.l, customVariant.value, true)} }
    `;
    ensureStyle('micyou-theme-package-css').textContent = installedThemeCss.value;
    ensureStyle('micyou-user-custom-css').textContent = customCssEnabled.value ? customCss.value : '';
    exportThemeToCli();
  });

  return {
    colorMode,
    themeMode,
    themeColor,
    uiStyle,
    customH,
    customS,
    customL,
    customVariant,
    customCss,
    customCssEnabled,
    installedThemeId,
    installedThemeCss,
    installedThemeControlsColor,
    clearInstalledTheme,
    resetThemeToDefaults,
    systemAccent,
    systemAccentLoading,
  };
}
