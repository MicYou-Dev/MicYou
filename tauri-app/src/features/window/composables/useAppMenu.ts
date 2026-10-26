import { onMounted, onBeforeUnmount, watch, type Ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { command, onEvent, type MenuNode, type UnlistenFn } from '@/platform';

// The wire types live in the platform layer; re-exported so callers can build a
// descriptor without importing from two places.
export type { MenuNode, PredefinedName } from '@/platform';

export interface AppMenuState {
  windowVisible: boolean;
  isStreaming: boolean;
  isMuted: boolean;
  isMonitoring: boolean;
  pocketMode: boolean;
  /**
   * The bind address is a per-run choice, so the submenu needs the mode, the
   * auto flag, the current pick and the list to draw from.
   */
  connectionMode: string;
  isAutoBind: boolean;
  selectedIp: string;
  /** Backing data for the network-interface submenu; empty while unavailable. */
  interfaces: Array<{ ip: string; name: string }>;
  /** Effective language code, used to tick the language submenu. */
  language: string;
}

export interface AppMenuCallbacks {
  onAbout: () => void | Promise<void>;
  onSettings: () => void | Promise<void>;
  onToggleStream: () => void | Promise<void>;
  onToggleWindow: () => void | Promise<void>;
  onToggleMute: () => void | Promise<void>;
  onToggleMonitoring: () => void | Promise<void>;
  onMode: (mode: string) => void | Promise<void>;
  /** `ip` is empty when `auto` is set, matching the window's selectIp("", true). */
  onInterface: (ip: string, auto: boolean) => void | Promise<void>;
  onTogglePocket: () => void | Promise<void>;
  onSwitchCli: () => void | Promise<void>;
  onSwitchTui: () => void | Promise<void>;
  onLanguage: (code: string) => void | Promise<void>;
  onWebsite: () => void | Promise<void>;
  onDocs: () => void | Promise<void>;
  onGithub: () => void | Promise<void>;
  onIssues: () => void | Promise<void>;
  onSponsors: () => void | Promise<void>;
  onOpenLogDir: () => void | Promise<void>;
  onExportLog: () => void | Promise<void>;
}

/**
 * The backend only forwards ids carrying this prefix, so every clickable item
 * has to use it or its click is dropped silently.
 */
export const APP_MENU_ID_PREFIX = 'menu:';

export const MENU_ID_ABOUT = `${APP_MENU_ID_PREFIX}about`;
export const MENU_ID_SETTINGS = `${APP_MENU_ID_PREFIX}settings`;
export const MENU_ID_TOGGLE_STREAM = `${APP_MENU_ID_PREFIX}toggle_stream`;
export const MENU_ID_TOGGLE_WINDOW = `${APP_MENU_ID_PREFIX}toggle_window`;
export const MENU_ID_MUTE = `${APP_MENU_ID_PREFIX}mute`;
export const MENU_ID_MONITORING = `${APP_MENU_ID_PREFIX}monitoring`;
export const MENU_ID_POCKET = `${APP_MENU_ID_PREFIX}pocket`;
export const MENU_ID_SWITCH_CLI = `${APP_MENU_ID_PREFIX}switch_cli`;
export const MENU_ID_SWITCH_TUI = `${APP_MENU_ID_PREFIX}switch_tui`;
export const MENU_ID_WEBSITE = `${APP_MENU_ID_PREFIX}website`;
export const MENU_ID_DOCS = `${APP_MENU_ID_PREFIX}docs`;
export const MENU_ID_GITHUB = `${APP_MENU_ID_PREFIX}github`;
export const MENU_ID_ISSUES = `${APP_MENU_ID_PREFIX}issues`;
export const MENU_ID_SPONSORS = `${APP_MENU_ID_PREFIX}sponsors`;
export const MENU_ID_LOG_DIR = `${APP_MENU_ID_PREFIX}log_dir`;
export const MENU_ID_EXPORT_LOG = `${APP_MENU_ID_PREFIX}export_log`;

/** Language entries carry the code in the id, so they need a prefix match. */
export const MENU_ID_LANG_PREFIX = `${APP_MENU_ID_PREFIX}lang:`;

export const menuLangId = (code: string) => `${MENU_ID_LANG_PREFIX}${code}`;

/** Connection-mode entries carry the mode in the id. */
export const MENU_ID_MODE_PREFIX = `${APP_MENU_ID_PREFIX}mode:`;

export const menuModeId = (mode: string) => `${MENU_ID_MODE_PREFIX}${mode}`;

/** Interface entries carry the bind address in the id; "auto" means every interface. */
export const MENU_ID_IFACE_PREFIX = `${APP_MENU_ID_PREFIX}iface:`;

export const menuIfaceId = (ip: string) => `${MENU_ID_IFACE_PREFIX}${ip}`;

export const MENU_ID_IFACE_AUTO = `${MENU_ID_IFACE_PREFIX}auto`;

/**
 * Language self-names: every language reads the same in any locale, so these are
 * shown verbatim — the same choice the settings dialog makes for its selector.
 */
export const APP_MENU_LANGUAGES: ReadonlyArray<{ code: string; label: string }> = [
  { code: 'zh', label: '简体中文' },
  { code: 'en', label: 'English' },
  { code: 'cat', label: '喵喵语 (´,,•ω•,,)' },
  { code: 'zh-hk', label: '粤语' },
  { code: 'zh-tw', label: '繁體中文（台灣）' },
  { code: 'zh-ss', label: '中国人（坚硬）' },
  { code: 'lzh', label: '文言' },
];

/**
 * The connection modes are product names rather than prose, so they read the same
 * in every locale — the same call the mode chips in the window make.
 */
export const APP_MENU_CONNECTION_MODES: ReadonlyArray<{ mode: string; label: string }> = [
  { mode: 'wifi', label: 'Wi-Fi' },
  { mode: 'usb', label: 'USB' },
  { mode: 'web', label: 'Web' },
];

/**
 * Full screen is only offered outside pocket mode, where it would fight with the
 * bar sizing. The entry is dropped instead of greyed out because the descriptor
 * only understands `enabled` on regular items.
 */
function fullscreenItems(t: (key: string) => string, state: AppMenuState): MenuNode[] {
  if (state.pocketMode) return [];
  return [
    { kind: 'separator' },
    { kind: 'predefined', name: 'fullscreen', label: t('menu.fullscreen') },
  ];
}

/**
 * "All Interfaces" is always offered; the per-interface entries only appear once
 * the backend has reported them, and the separator goes with them so an empty
 * list never leaves a stray line behind.
 */
function interfaceItems(t: (key: string) => string, state: AppMenuState): MenuNode[] {
  const auto = {
    kind: 'check' as const,
    id: MENU_ID_IFACE_AUTO,
    label: t('app.ipSelector.allInterfaces'),
    checked: state.isAutoBind,
  };
  if (state.interfaces.length === 0) return [auto];
  return [
    auto,
    { kind: 'separator' },
    ...state.interfaces.map((iface) => ({
      kind: 'check' as const,
      id: menuIfaceId(iface.ip),
      label: `${iface.ip} (${iface.name})`,
      checked: !state.isAutoBind && state.selectedIp === iface.ip,
    })),
  ];
}

/**
 * Builds the macOS app menu from the current locale and app state.
 *
 * The Edit and Window entries stay `predefined` on purpose: they keep their
 * native selectors, which is what makes the standard shortcuts (⌘C/⌘V/⌘X/⌘A/⌘Z)
 * reach the text fields. They still carry a `label` because the backend falls
 * back to a hardcoded English title when none is given.
 */
export function appMenuFromI18n(t: (key: string) => string, state: AppMenuState): MenuNode[] {
  return [
    {
      kind: 'submenu',
      label: 'MicYou',
      items: [
        { kind: 'item', id: MENU_ID_ABOUT, label: t('menu.about') },
        { kind: 'separator' },
        {
          kind: 'item',
          id: MENU_ID_SETTINGS,
          label: t('menu.settings'),
          accelerator: 'CmdOrCtrl+,',
        },
        { kind: 'separator' },
        { kind: 'predefined', name: 'services', label: t('menu.services') },
        { kind: 'separator' },
        { kind: 'predefined', name: 'hide', label: t('menu.hide') },
        { kind: 'predefined', name: 'hideOthers', label: t('menu.hideOthers') },
        { kind: 'predefined', name: 'showAll', label: t('menu.showAll') },
        { kind: 'separator' },
        { kind: 'predefined', name: 'quit', label: t('menu.quit') },
      ],
    },
    {
      kind: 'submenu',
      label: t('menu.service'),
      items: [
        {
          kind: 'item',
          id: MENU_ID_TOGGLE_STREAM,
          label: state.isStreaming ? t('tray.stop') : t('tray.start'),
          accelerator: 'CmdOrCtrl+Shift+S',
        },
        {
          kind: 'item',
          id: MENU_ID_TOGGLE_WINDOW,
          label: state.windowVisible ? t('tray.hide') : t('tray.show'),
        },
        { kind: 'separator' },
        {
          kind: 'submenu',
          label: t('menu.connectionMode'),
          items: APP_MENU_CONNECTION_MODES.map(({ mode, label }) => ({
            kind: 'check' as const,
            id: menuModeId(mode),
            label,
            checked: state.connectionMode === mode,
          })),
        },
        {
          kind: 'submenu',
          label: t('menu.networkInterface'),
          // USB goes through adb reverse, so the bind address is ignored there.
          enabled: state.connectionMode !== 'usb',
          items: interfaceItems(t, state),
        },
        { kind: 'separator' },
        {
          kind: 'check',
          id: MENU_ID_MUTE,
          label: t('menu.mute'),
          checked: state.isMuted,
        },
        {
          kind: 'check',
          id: MENU_ID_MONITORING,
          label: t('menu.monitoring'),
          checked: state.isMonitoring,
        },
      ],
    },
    {
      kind: 'submenu',
      label: t('menu.view'),
      items: [
        {
          kind: 'check',
          id: MENU_ID_POCKET,
          label: t('menu.pocketMode'),
          checked: state.pocketMode,
        },
        { kind: 'separator' },
        { kind: 'item', id: MENU_ID_SWITCH_CLI, label: t('tray.switchCli') },
        { kind: 'item', id: MENU_ID_SWITCH_TUI, label: t('tray.switchTui') },
        { kind: 'separator' },
        {
          kind: 'submenu',
          label: t('menu.language'),
          items: APP_MENU_LANGUAGES.map(({ code, label }) => ({
            kind: 'check' as const,
            id: menuLangId(code),
            label,
            checked: state.language === code,
          })),
        },
        ...fullscreenItems(t, state),
      ],
    },
    {
      kind: 'submenu',
      label: t('menu.edit'),
      items: [
        { kind: 'predefined', name: 'undo', label: t('menu.undo') },
        { kind: 'predefined', name: 'redo', label: t('menu.redo') },
        { kind: 'separator' },
        { kind: 'predefined', name: 'cut', label: t('menu.cut') },
        { kind: 'predefined', name: 'copy', label: t('menu.copy') },
        { kind: 'predefined', name: 'paste', label: t('menu.paste') },
        { kind: 'predefined', name: 'selectAll', label: t('menu.selectAll') },
      ],
    },
    {
      kind: 'submenu',
      label: t('menu.window'),
      role: 'window',
      items: [
        { kind: 'predefined', name: 'minimize', label: t('menu.minimize') },
        { kind: 'predefined', name: 'maximize', label: t('menu.zoom') },
        { kind: 'separator' },
        { kind: 'predefined', name: 'closeWindow', label: t('menu.closeWindow') },
      ],
    },
    {
      kind: 'submenu',
      label: t('menu.help'),
      role: 'help',
      items: [
        { kind: 'item', id: MENU_ID_WEBSITE, label: t('menu.website') },
        { kind: 'item', id: MENU_ID_DOCS, label: t('menu.docs') },
        { kind: 'item', id: MENU_ID_GITHUB, label: t('menu.github') },
        { kind: 'item', id: MENU_ID_ISSUES, label: t('menu.issues') },
        { kind: 'separator' },
        { kind: 'item', id: MENU_ID_SPONSORS, label: t('menu.sponsors') },
        { kind: 'separator' },
        { kind: 'item', id: MENU_ID_LOG_DIR, label: t('menu.openLogDir') },
        { kind: 'item', id: MENU_ID_EXPORT_LOG, label: t('menu.exportLog') },
      ],
    },
  ];
}

/**
 * Returns a description of the first node that cannot be expressed by the
 * backend (missing id prefix or empty label), or null when the menu is fine.
 * Such nodes would render as dead menu entries, so the menu is not pushed at all.
 */
export function findInvalidMenuNode(nodes: MenuNode[], path: string[] = []): string | null {
  for (const [index, node] of nodes.entries()) {
    const here = [...path, `${node.kind}[${index}]`];
    if (node.kind === 'separator') continue;

    if (node.kind === 'predefined') {
      // Without a label the backend falls back to its built-in English title
      // ("&Copy", "Toggle Full Screen", …), so one is mandatory here. The wire
      // type leaves it optional because the Rust side accepts an omitted label.
      if (!node.label?.trim()) {
        return `${here.join(' › ')}: predefined entry needs a label`;
      }
      continue;
    }

    if (!node.label.trim()) {
      return `${here.join(' › ')}: empty label`;
    }
    if (path.length === 0 && node.kind !== 'submenu') {
      // The menu bar holds submenus only; anything else makes the backend reject
      // the whole menu instead of skipping that one entry.
      return `${here.join(' › ')}: the menu bar holds submenus only`;
    }
    if (node.kind === 'item' || node.kind === 'check') {
      if (!node.id.startsWith(APP_MENU_ID_PREFIX)) {
        return `${here.join(' › ')}: id "${node.id}" is missing the "${APP_MENU_ID_PREFIX}" prefix`;
      }
    } else {
      const nested = findInvalidMenuNode(node.items, here);
      if (nested) return nested;
    }
  }
  return null;
}

export function useAppMenu(callbacks: AppMenuCallbacks, state: Ref<AppMenuState>) {
  const { t, locale } = useI18n();
  let unlisten: UnlistenFn | null = null;
  let lastPushedMenu: string | null = null;

  async function push() {
    const menu = appMenuFromI18n(t, state.value);
    const invalid = findInvalidMenuNode(menu);
    if (invalid) {
      console.error('Refusing to push an invalid app menu:', invalid);
      return;
    }

    const key = JSON.stringify(menu);
    if (key === lastPushedMenu) return;
    lastPushedMenu = key;
    try {
      await command('set_app_menu', { menu });
    } catch (e) {
      lastPushedMenu = null;
      console.error('set_app_menu failed:', e);
    }
  }

  onMounted(async () => {
    unlisten = await onEvent('app-menu-action', (id) => {
      if (id.startsWith(MENU_ID_LANG_PREFIX)) {
        void callbacks.onLanguage(id.slice(MENU_ID_LANG_PREFIX.length));
      } else if (id.startsWith(MENU_ID_MODE_PREFIX)) {
        void callbacks.onMode(id.slice(MENU_ID_MODE_PREFIX.length));
      } else if (id.startsWith(MENU_ID_IFACE_PREFIX)) {
        const iface = id.slice(MENU_ID_IFACE_PREFIX.length);
        const auto = iface === 'auto';
        void callbacks.onInterface(auto ? '' : iface, auto);
      } else
        switch (id) {
          case MENU_ID_ABOUT:
            void callbacks.onAbout();
            break;
          case MENU_ID_SETTINGS:
            void callbacks.onSettings();
            break;
          case MENU_ID_TOGGLE_STREAM:
            void callbacks.onToggleStream();
            break;
          case MENU_ID_TOGGLE_WINDOW:
            void callbacks.onToggleWindow();
            break;
          case MENU_ID_MUTE:
            void callbacks.onToggleMute();
            break;
          case MENU_ID_MONITORING:
            void callbacks.onToggleMonitoring();
            break;
          case MENU_ID_POCKET:
            void callbacks.onTogglePocket();
            break;
          case MENU_ID_SWITCH_CLI:
            void callbacks.onSwitchCli();
            break;
          case MENU_ID_SWITCH_TUI:
            void callbacks.onSwitchTui();
            break;
          case MENU_ID_WEBSITE:
            void callbacks.onWebsite();
            break;
          case MENU_ID_DOCS:
            void callbacks.onDocs();
            break;
          case MENU_ID_GITHUB:
            void callbacks.onGithub();
            break;
          case MENU_ID_ISSUES:
            void callbacks.onIssues();
            break;
          case MENU_ID_SPONSORS:
            void callbacks.onSponsors();
            break;
          case MENU_ID_LOG_DIR:
            void callbacks.onOpenLogDir();
            break;
          case MENU_ID_EXPORT_LOG:
            void callbacks.onExportLog();
            break;
          default:
            console.warn('Unknown app-menu-action id:', id);
        }
      // muda flips a check item's own state before it dispatches the click, so an
      // action that ends up changing nothing (a cancelled warning, re-picking the
      // active language) would leave the native tick lying. Clearing the dedupe key
      // and re-pushing from the authoritative frontend state puts it back.
      lastPushedMenu = null;
      void push();
    });

    await push();
  });

  watch([state, locale], () => {
    void push();
  });

  onBeforeUnmount(() => {
    if (unlisten) unlisten();
  });

  return {
    push,
  };
}
