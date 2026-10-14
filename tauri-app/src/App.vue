<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue';
import { useStorage, onClickOutside } from '@vueuse/core';
import { useI18n } from 'vue-i18n';

// UI icons imported from lucide-vue
import { 
  Mic, Wifi, RadioTower, Globe, ChevronDown, CheckCircle2, Settings, 
  Link, Unlink, RefreshCw, ActivitySquare as MonitoringIcon, X, Minus, 
  VolumeX, Volume2, Headphones, QrCode as QrCodeIcon, Loader2 
} from '@lucide/vue';

// Composables managing server connection, audio, theme, window, and system tray
import { appWindow, command, onEvent, openUrl } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';
import { useServer, type ConnectionMode } from './features/connection/composables/useServer';
import { useAudio } from './features/audio/composables/useAudio';
import { useTheme, saveUiPrefs } from './features/theme/composables/useTheme';
import { useWindow } from './features/window/composables/useWindow';
import { useTray } from './features/tray/useTray';
import { useWindowEffects } from './features/window/composables/useWindowEffects';
import { useWindowTransition } from './features/window/composables/useWindowTransition';
import { applyPlatformClasses, isMacOS } from './shared/lib/os';
import { useAppMenu, type AppMenuState } from './features/window/composables/useAppMenu';

// UI components for connection flows, onboarding, and layouts
import ConnectionErrorDialog from './features/connection/components/ConnectionErrorDialog.vue';
import QrCodeDialog from './features/connection/components/QrCodeDialog.vue';
import AudioRing from './features/audio/components/AudioRing.vue';
import MonitoringPanel from './features/audio/components/MonitoringPanel.vue';
import OnboardingWizard from './features/onboarding/components/OnboardingWizard.vue';
import PocketLayout from './features/pocket/components/PocketLayout.vue';
import CloseConfirmDialog from './features/window/components/CloseConfirmDialog.vue';
import UdpWarningDialog from './features/connection/components/UdpWarningDialog.vue';
import MonitoringWarningDialog from './features/audio/components/MonitoringWarningDialog.vue';

// Raw asset content and animation utilities
import appIconSvg from './shared/assets/app_icon.svg?raw';
import { animate, spring } from 'animejs';

applyPlatformClasses();

// macOS drags its windows by the titlebar without a grab affordance, so the
// header keeps the regular pointer there while Windows and Linux keep the grab
// cursor.
const dragSurfaceClass = isMacOS ? 'cursor-default' : 'cursor-grab active:cursor-grabbing';

const { t, locale } = useI18n();

// Initialize shared features
const audio = useAudio();
const server = useServer({ audioLevel: audio.audioLevel, isMuted: audio.isMuted });
const theme = useTheme();
// The CLI/TUI read language and theme color from ui.json.
onMounted(() => saveUiPrefs());

/**
 * Handles custom window dragging. Uses custom Win32 loop on Windows, falls back to Tauri API on other OSs.
 * Prevents dragging when clicking interactive controls.
 */
const startDrag = async (e: MouseEvent) => {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest('button, a, input, select, textarea, [role="button"]')) return;
  try {
    await command('start_window_drag');
  } catch {
    // Non-Windows fallback to Tauri's native startDragging
    try {
      await appWindow.startDragging();
    } catch (err) {
      console.error('Drag failed:', err);
    }
  }
};

// References to HTML elements for animations
const centralBtnRef = ref<HTMLButtonElement | null>(null);

// App wizard and pocket mode settings
const showOnboarding = ref(localStorage.getItem('micyou_onboarding_completed') !== 'true');
// Settings open in their own window; the main window only follows the
// output device chosen there.
const openSettings = async () => {
  try {
    await command('open_settings_window', { title: t('settings.title') });
  } catch (e) {
    console.error('open_settings_window failed:', e);
  }
};
const track = useListeners();
onMounted(() => {
  void track(onEvent('output-device-changed', (payload) => {
    server.outputDevice.value = payload;
  }));
});
const pocketMode = useStorage('micyou_pocket_mode', false);
const windowEffects = useWindowEffects({
  blur: computed(() => theme.uiStyle.value === 'style-glass'),
  // Pocket mode is just the floating bar; the shadow frames the full window.
  shadowRadius: computed(() => (pocketMode.value ? null : 16)),
});
const rootRef = ref<HTMLElement | null>(null);
const transition = useWindowTransition(rootRef, { suspendEffects: windowEffects.suspend });
const win = useWindow({ exit: transition.exit, enter: transition.enter, prepare: windowEffects.apply });

// IP configuration selector panel behavior
const ipMenuRef = ref<HTMLDivElement | null>(null);
onClickOutside(ipMenuRef, () => {
  server.showIpMenu.value = false;
});

// Close IP menu on window blur to maintain clean UI focus
const closeIpMenu = () => {
  server.showIpMenu.value = false;
};
onMounted(() => window.addEventListener('blur', closeIpMenu));
onUnmounted(() => window.removeEventListener('blur', closeIpMenu));

/**
 * Coordinates server stream toggle and trigger scale rebound animations on the central button
 */
const toggleStreaming = async () => {
  await server.toggleStreaming();
  if (centralBtnRef.value) {
    animate(centralBtnRef.value, {
      scale: [1, 0.9, 1.05, 1],
      ease: spring({ mass: 1, stiffness: 80, damping: 10, velocity: 0 }),
    });
  }
};

// Computed state tracking stream and visibility for System Tray syncs
const streamingRef = computed(() => server.isStreaming(server.serverState.value));
const visibilityRef = computed(() => !win.isHidden.value);

// Handing the GUI over to the CLI/TUI closes this app and opens a terminal, so
// it is confirmed first. Shared by the tray and the native app menu.
async function switchToCli() {
  const confirmSwitch = confirm(t('settings.runMode.confirmSwitch'));
  if (!confirmSwitch) return;
  try {
    await command('switch_to_cli');
    await win.exitApp();
  } catch (e) {
    console.error('switch_to_cli failed:', e);
    alert(`${t('settings.runMode.switchFailed')}: ${e}`);
  }
}

async function switchToTui() {
  const confirmSwitch = confirm(t('settings.runMode.confirmSwitchTui'));
  if (!confirmSwitch) return;
  try {
    await command('switch_to_tui');
    await win.exitApp();
  } catch (e) {
    console.error('switch_to_tui failed:', e);
    alert(`${t('settings.runMode.switchFailed')}: ${e}`);
  }
}

// Initialize system tray integration
useTray(
  {
    onShow: async () => {
      // A minimized window is still "shown"; the tray click brings it back.
      if (win.isHidden.value || await appWindow.isMinimized()) {
        await win.showMainWindow();
      } else {
        await win.hideMainWindow();
      }
    },
    onToggleStream: () => toggleStreaming(),
    onExit: () => win.exitApp(),
    onSwitchCli: () => switchToCli(),
    onSwitchTui: () => switchToTui(),
  },
  visibilityRef,
  streamingRef,
);

// Settings live in their own window, so the macOS menu cannot call into them
// directly. A requested section ('about', or 'sponsors' for the sponsors
// dialog) goes through shared storage: an open settings window picks it up
// from the storage event, a new one reads it on mount.
const SETTINGS_REQUEST_KEY = 'micyou_settings_request';

const openSettingsAt = async (section?: string) => {
  if (section) localStorage.setItem(SETTINGS_REQUEST_KEY, section);
  await openSettings();
};

// The settings window follows the stored language through the storage event,
// which never fires in the window that wrote it, so the locale is set here too.
const setLanguage = (code: string) => {
  localStorage.setItem('micyou_language', code);
  locale.value = code;
  saveUiPrefs(code);
};

// Native app menu: the frontend owns its labels, state and actions.
const appMenuState = computed<AppMenuState>(() => ({
  windowVisible: !win.isHidden.value,
  isStreaming: streamingRef.value,
  isMuted: audio.isMuted.value,
  isMonitoring: audio.isMonitoringEnabled.value,
  pocketMode: pocketMode.value,
  connectionMode: server.connectionMode.value,
  isAutoBind: server.isAutoBind.value,
  selectedIp: server.selectedIp.value,
  interfaces: server.networkInterfaces.value.map((i) => ({ ip: i.ip, name: i.interface_name })),
  language: locale.value,
}));

// Help menu targets: the project site hosts the documentation, the upstream
// repository hosts the issue tracker and the sponsorship programme.
const WEBSITE_URL = 'https://micyou.top/';
const DOCS_URL = 'https://micyou.top/en/docs/quick-start';
const GITHUB_URL = 'https://github.com/LanRhyme/MicYou';
const ISSUES_URL = 'https://github.com/LanRhyme/MicYou/issues/new';

const openExternal = (url: string) => {
  void openUrl(url).catch((e) => console.error('openUrl failed:', e));
};

useAppMenu(
  {
    onAbout: () => openSettingsAt('about'),
    onSettings: () => openSettings(),
    onToggleStream: () => toggleStreaming(),
    onToggleWindow: async () => {
      if (win.isHidden.value) {
        await win.showMainWindow();
      } else {
        await win.hideMainWindow();
      }
    },
    onToggleMute: () => audio.toggleMute(),
    onToggleMonitoring: () => audio.toggleMonitoringEnabled(),
    onMode: (mode) => {
      server.connectionMode.value = mode as ConnectionMode;
    },
    onInterface: (ip, auto) => {
      server.selectIp(auto ? '' : ip, auto);
    },
    onTogglePocket: () => {
      pocketMode.value = !pocketMode.value;
    },
    onSwitchCli: () => switchToCli(),
    onSwitchTui: () => switchToTui(),
    onLanguage: (code) => setLanguage(code),
    onWebsite: () => openExternal(WEBSITE_URL),
    onDocs: () => openExternal(DOCS_URL),
    onGithub: () => openExternal(GITHUB_URL),
    onIssues: () => openExternal(ISSUES_URL),
    onSponsors: () => openSettingsAt('sponsors'),
    onOpenLogDir: async () => {
      try {
        await command('open_log_dir');
      } catch (e) {
        console.error('open_log_dir failed:', e);
      }
    },
    onExportLog: async () => {
      try {
        await command('export_log');
      } catch (e) {
        console.error('export_log failed:', e);
      }
    },
  },
  appMenuState,
);

// Starting minimized to the tray hides the window right away.
onMounted(() => void win.launch());

// In-window dialogs pocket mode has to make room for (see the watch below).
const pocketModalOpen = computed(
  () =>
    showOnboarding.value ||
    win.showCloseConfirm.value ||
    server.showErrorDialog.value ||
    server.showQrDialog.value ||
    server.showDeviceSelector.value ||
    server.showIpSwitchConfirm.value ||
    audio.showUdpWarning.value ||
    audio.showMonitoringWarning.value,
);

// Window sizing. Full mode is a fixed 800x600 window; pocket mode follows its
// content, including the expanded menus, so no transparent area is left over.
const FULL_SIZE = { width: 800, height: 600 };
const POCKET_PADDING = 12; // p-1.5 on both sides
const POCKET_MIN_WIDTH = 240;

const pocketContentRef = ref<HTMLElement | null>(null);
let pocketObserver: ResizeObserver | null = null;
let pocketRaf = 0;

// Linux keeps the window resizable with min = max (GTK3 adds 48px to
// non-resizable windows on Wayland), so every resize moves the bounds too.
async function applyFullSize() {
  try {
    await appWindow.setFixedSize(FULL_SIZE.width, FULL_SIZE.height);
  } catch (e) {
    console.error('Failed to resize window:', e);
  }
}

async function resizePocketToContent() {
  const el = pocketContentRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  try {
    await appWindow.setFixedSize(
      Math.max(Math.ceil(rect.width + POCKET_PADDING), POCKET_MIN_WIDTH),
      Math.ceil(rect.height + POCKET_PADDING),
    );
  } catch (e) {
    console.error('Failed to resize pocket window:', e);
  }
}

// The content box is w-max/h-auto, so resizing the window never feeds back
// into the observed size.
function startPocketObserver() {
  stopPocketObserver();
  const el = pocketContentRef.value;
  if (!el) return;
  pocketObserver = new ResizeObserver(() => {
    // A dialog has the window at full size; tracking the content then would
    // shrink it back underneath the dialog.
    if (pocketModalOpen.value) return;
    if (pocketRaf) cancelAnimationFrame(pocketRaf);
    pocketRaf = requestAnimationFrame(() => void resizePocketToContent());
  });
  pocketObserver.observe(el);
}

function stopPocketObserver() {
  pocketObserver?.disconnect();
  pocketObserver = null;
  if (pocketRaf) cancelAnimationFrame(pocketRaf);
  pocketRaf = 0;
}

// The native window controls sit at a different height per layout, and the
// resize above makes AppKit restore them, so the layout is re-applied after it.
async function applyWindowControlLayout(mode: 'full' | 'pocket') {
  if (!isMacOS) return;
  try {
    await command('apply_macos_window_layout', { mode });
  } catch (e) {
    console.error('apply_macos_window_layout failed:', e);
  }
}

async function enterPocketLayout() {
  await nextTick();
  startPocketObserver();
  await resizePocketToContent();
}

watch(
  pocketMode,
  async (isPocket, wasPocket) => {
    if (isPocket) {
      if (pocketModalOpen.value) await applyFullSize();
      else await enterPocketLayout();
    } else {
      stopPocketObserver();
      if (wasPocket) await applyFullSize();
    }
    await applyWindowControlLayout(isPocket ? 'pocket' : 'full');
  },
  { immediate: true },
);

// 主窗口内的模态(关闭确认、各类警告、设备/IP 选择器、引导向导)都是窗口内的覆盖层，
// 在袖珍模式那条窄窗口里会被裁得只剩一条，因此它们打开时把窗口临时还原成 800x600，
// 关闭后再收回袖珍尺寸。新增主窗口内模态时，必须把它的开关加进 pocketModalOpen。
watch(pocketModalOpen, async (open) => {
  if (!pocketMode.value) return;
  if (open) {
    stopPocketObserver();
    await applyFullSize();
  } else {
    await enterPocketLayout();
  }
});

// Dock 徽标：串流时显示已连接客户端数（Wi-Fi/USB 模式下即为 1 台设备），
// 其余时间清除。
const dockBadge = computed(() => {
  if (!isMacOS || server.serverState.value !== 'streaming') return null;
  return String(Math.max(server.webClientCount.value, 1));
});

watch(
  dockBadge,
  async (label) => {
    try {
      await command('set_dock_badge', { label });
    } catch (e) {
      console.error('set_dock_badge failed:', e);
    }
  },
  { immediate: true },
);

// Central action button hover animations
const onCentralBtnHover = () => {
  if (centralBtnRef.value) {
    animate(centralBtnRef.value, {
      scale: 1.08,
      duration: 400,
      ease: 'outExpo',
    });
  }
};

const onCentralBtnLeave = () => {
  if (centralBtnRef.value) {
    animate(centralBtnRef.value, {
      scale: 1,
      duration: 500,
      ease: 'outExpo',
    });
  }
};

onUnmounted(() => {
  stopPocketObserver();
});
</script>

<template>
  <OnboardingWizard :visible="showOnboarding" @complete="showOnboarding = false" />
  <div ref="rootRef" class="relative w-full h-screen overflow-hidden overscroll-none text-foreground bg-transparent origin-center">
    <!-- Rounded window background; pocket mode draws only its own bar and menus -->
    <div v-if="!pocketMode" data-blur-region class="absolute inset-0 -z-10 rounded-2xl bg-surface-container" />

    <!-- Pocket Mode -->
    <div v-if="pocketMode" class="pocket-root p-1.5">
      <div ref="pocketContentRef" class="w-max" :class="dragSurfaceClass" @mousedown="startDrag">
        <PocketLayout
          :serverState="server.serverState.value"
          :connectionMode="server.connectionMode.value"
          :serverPort="server.serverPort.value"
          :webPort="server.webPort.value"
          :displayIp="server.displayIp.value"
          :isAutoBind="server.isAutoBind.value"
          :selectedIp="server.selectedIp.value"
          :networkInterfaces="server.networkInterfaces.value"
          :isMuted="audio.isMuted.value"
          :isMonitoringEnabled="audio.isMonitoringEnabled.value"
          :isMacOS="isMacOS"
          @toggleStream="toggleStreaming"
          @selectIp="(ip, auto) => server.selectIp(ip, auto)"
          @updateMode="m => server.connectionMode.value = m"
          @updatePort="p => server.serverPort.value = p"
          @updateWebPort="p => server.webPort.value = p"
          @toggleMute="audio.toggleMute"
          @toggleMonitoringEnabled="audio.toggleMonitoringEnabled"
          @openSettings="openSettings"
          @minimize="win.minimizeWindow"
          @close="win.requestClose"
        />
      </div>
    </div>

    <!-- Full Mode -->
    <div v-else class="absolute inset-0 flex flex-col p-3 gap-3">
      <!-- Header Section -->
      <div class="haze-surface rounded-2xl flex justify-between items-center px-4 py-2 shrink-0 relative z-30" :class="dragSurfaceClass" @mousedown="startDrag">
        <div class="flex items-center gap-3">
          <!-- macOS uses the native window controls; keep their footprint clear -->
          <div v-if="isMacOS" class="macos-titlebar-spacer mr-1 shrink-0" />
          <div class="w-8 h-8 text-primary pointer-events-none [&>svg]:w-full [&>svg]:h-full" v-html="appIconSvg"></div>
          <div class="flex flex-col pointer-events-none select-none">
            <span class="text-sm font-extrabold text-primary">MicYou Desktop</span>
            <span class="text-[11px] text-on-surface-variant font-medium">Server</span>
          </div>
        </div>

        <div class="flex items-center gap-4">
          <!-- Network Selector -->
          <div class="relative" ref="ipMenuRef">
            <div
              class="flex items-center bg-surface-variant/30 hover:bg-surface-variant/50 transition-all duration-300 hover:-translate-y-0.5 active:scale-95 hover:shadow-xs px-3 py-1.5 rounded-lg cursor-pointer border border-white/5"
              role="button"
              @click="server.showIpMenu.value = !server.showIpMenu.value"
            >
              <Globe class="w-3.5 h-3.5 text-primary mr-2 pointer-events-none" />
              <span class="text-xs font-medium mr-1 select-none pointer-events-none">{{ server.displayIp.value }}</span>
              <ChevronDown class="w-4 h-4 text-on-surface-variant/60 pointer-events-none transition-transform" :class="{ 'rotate-180': server.showIpMenu.value }" />
            </div>

            <Transition
              enter-active-class="transition ease-out duration-150"
              enter-from-class="opacity-0 scale-95 -translate-y-1"
              enter-to-class="opacity-100 scale-100 translate-y-0"
              leave-active-class="transition ease-in duration-100"
              leave-from-class="opacity-100 scale-100 translate-y-0"
              leave-to-class="opacity-0 scale-95 -translate-y-1"
            >
              <div
                v-if="server.showIpMenu.value"
                class="absolute right-0 top-full mt-1 w-64 bg-surface border border-outline/20 rounded-xl shadow-xl z-50 overflow-hidden"
              >
                <div class="max-h-64 overflow-y-auto overflow-x-hidden no-scrollbar">
                  <button
                    class="w-full flex items-center gap-3 px-3 py-2.5 hover:bg-surface-variant/50 transition-colors text-left"
                    @click="server.selectIp('', true)"
                  >
                    <div class="flex-1 min-w-0">
                      <div class="text-xs font-medium text-foreground">{{ t('app.ipSelector.allInterfaces') }}</div>
                      <div class="text-[10px] text-on-surface-variant mt-0.5">{{ t('app.ipSelector.allInterfacesDesc') }}</div>
                    </div>
                    <CheckCircle2 v-if="server.isAutoBind.value" class="w-4 h-4 text-primary shrink-0" />
                  </button>

                  <button
                    v-for="iface in server.networkInterfaces.value"
                    :key="iface.ip"
                    class="w-full flex items-center gap-3 px-3 py-2.5 hover:bg-surface-variant/50 transition-colors text-left"
                    @click="server.selectIp(iface.ip, false)"
                  >
                    <div class="flex-1 min-w-0">
                      <div class="text-xs font-medium text-foreground truncate" :title="iface.ip">{{ iface.ip }}</div>
                      <div class="text-[10px] text-on-surface-variant mt-0.5 truncate">{{ iface.interface_name }}</div>
                    </div>
                    <CheckCircle2 v-if="!server.isAutoBind.value && server.selectedIp.value === iface.ip" class="w-4 h-4 text-primary shrink-0" />
                  </button>
                </div>
              </div>
            </Transition>
          </div>

          <!-- Window Controls (non-macOS: right) -->
          <div v-if="!isMacOS" class="flex items-center gap-1 ml-1">
            <button @click="win.minimizeWindow()" class="w-7 h-7 flex items-center justify-center rounded-full hover:bg-white/10 transition-colors">
              <Minus class="w-4 h-4 text-on-surface" />
            </button>
            <button @click="win.requestClose()" class="w-7 h-7 flex items-center justify-center rounded-full hover:bg-error/20 hover:text-error transition-colors">
              <X class="w-4 h-4 text-on-surface" />
            </button>
          </div>
        </div>
      </div>

      <!-- Main Content -->
      <div class="relative flex flex-1 gap-3 min-h-0">
        <!-- Left Panel -->
        <div class="flex flex-col gap-3 transition-all duration-300" :class="audio.showMonitoringPanel.value ? 'w-[28%]' : 'w-[38%]'">
          <!-- Mode Card -->
          <div class="haze-surface rounded-2xl p-3 flex flex-col gap-2">
            <span class="text-xs text-on-surface-variant font-medium">{{ $t('app.connectionMode') }}</span>
            <div class="flex gap-1.5">
              <button
                @click="server.connectionMode.value = 'wifi'"
                class="flex-1 flex flex-col items-center justify-center py-2 rounded-xl transition-all duration-300 hover:-translate-y-0.5 active:scale-95"
                :class="server.connectionMode.value === 'wifi' ? 'bg-primary text-on-primary shadow-[0_0_16px_0] shadow-primary/30' : 'bg-surface-variant/40 text-on-surface-variant hover:bg-surface-variant/70 hover:shadow-md'"
              >
                <Wifi class="w-4 h-4 mb-1" />
                <span class="text-[10px] font-medium">Wi-Fi</span>
              </button>
              <button
                @click="server.connectionMode.value = 'usb'"
                class="flex-1 flex flex-col items-center justify-center py-2 rounded-xl transition-all duration-300 hover:-translate-y-0.5 active:scale-95"
                :class="server.connectionMode.value === 'usb' ? 'bg-primary text-on-primary shadow-[0_0_16px_0] shadow-primary/30' : 'bg-surface-variant/40 text-on-surface-variant hover:bg-surface-variant/70 hover:shadow-md'"
              >
                <Mic class="w-4 h-4 mb-1" />
                <span class="text-[10px] font-medium">USB</span>
              </button>
              <button
                @click="server.connectionMode.value = 'web'"
                class="flex-1 flex flex-col items-center justify-center py-2 rounded-xl transition-all duration-300 hover:-translate-y-0.5 active:scale-95"
                :class="server.connectionMode.value === 'web' ? 'bg-primary text-on-primary shadow-[0_0_16px_0] shadow-primary/30' : 'bg-surface-variant/40 text-on-surface-variant hover:bg-surface-variant/70 hover:shadow-md'"
              >
                <Globe class="w-4 h-4 mb-1" />
                <span class="text-[10px] font-medium">Web</span>
              </button>
            </div>
          </div>

          <!-- Port Card -->
          <div v-if="server.connectionMode.value !== 'web'" class="haze-surface rounded-2xl p-3 flex flex-col gap-2">
            <span class="text-xs text-on-surface-variant font-medium">{{ $t('app.port') }}</span>
            <input
              v-model="server.serverPort.value"
              type="number"
              max="65534"
              class="w-full bg-surface-variant/40 hover:bg-surface-variant/60 border border-white/5 rounded-xl px-3 py-2 text-sm text-foreground focus:outline-hidden focus:ring-2 focus:ring-primary/50 focus:bg-surface-variant/60 transition-all duration-300"
            />
          </div>

          <!-- Web QR Card -->
          <div v-else class="haze-surface rounded-2xl p-3 flex flex-col items-center justify-center gap-2">
            <span class="text-xs text-on-surface-variant font-medium self-start">{{ $t('app.port') }}</span>
            <div v-if="server.serverState.value === 'idle'" class="w-full">
                <input v-model="server.webPort.value" type="number"
                    class="w-full bg-surface-variant/40 hover:bg-surface-variant/60 border border-white/5 rounded-xl px-3 py-2 text-sm text-foreground focus:outline-hidden focus:ring-2 focus:ring-primary/50 focus:bg-surface-variant/60 transition-all duration-300" />
            </div>
            <button v-if="server.serverState.value !== 'idle'" @click="server.showQrDialog.value = true"
                class="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl bg-primary/10 text-primary text-sm font-medium hover:bg-primary/20 active:scale-[0.98] transition-all">
              <QrCodeIcon class="w-4 h-4" />
              <span>{{ server.qrDataUrl.value ? $t('app.web.scanToConnect') : $t('app.status.connectingDesc', { port: server.webPort.value }) }}</span>
            </button>
            <span v-if="server.serverState.value !== 'idle' && server.webClientCount.value > 0" class="text-xs text-primary font-medium">
              {{ $t('app.web.clientsConnected', { count: server.webClientCount.value }) }}
            </span>
          </div>

          <!-- Status Card -->
          <div class="haze-surface rounded-2xl p-4 flex-1 flex flex-col items-center justify-center text-center gap-3 group transition-all duration-300 hover:shadow-md hover:-translate-y-0.5">
            <div class="w-12 h-12 rounded-full flex items-center justify-center transition-all duration-500 group-hover:scale-110"
                 :class="server.serverState.value === 'streaming' ? 'bg-primary/20 text-primary' : (server.serverState.value === 'starting' ? 'bg-secondary/20 text-secondary' : (server.serverState.value === 'connecting' ? 'bg-tertiary/20 text-tertiary' : 'bg-surface-variant/50 text-on-surface-variant'))">
              <CheckCircle2 v-if="server.serverState.value === 'streaming'" class="w-6 h-6 animate-pulse" />
              <Loader2 v-else-if="server.serverState.value === 'starting'" class="w-6 h-6 animate-spin" />
              <RadioTower v-else class="w-6 h-6 transition-transform duration-500 group-hover:rotate-12" :class="{ 'animate-spin-slow': server.serverState.value === 'connecting' }" />
            </div>
            <div>
              <h3 class="text-sm font-bold">{{ server.serverState.value === 'streaming' ? $t('app.status.streaming') : (server.serverState.value === 'connecting' ? $t('app.status.connecting') : (server.serverState.value === 'starting' ? $t('app.status.starting') : $t('app.status.ready'))) }}</h3>
              <p class="text-xs text-on-surface-variant mt-1 max-w-[200px] mx-auto">
                {{ server.statusDescription.value }}
              </p>
            </div>
          </div>
        </div>

        <!-- Center Panel -->
        <div class="haze-surface rounded-2xl flex flex-col items-center justify-center relative overflow-hidden group transition-all duration-300" :class="audio.showMonitoringPanel.value ? 'w-[44%]' : 'w-[62%]'">
          <!-- Central Visualizer & Action Button -->
          <div class="relative w-64 h-64 flex items-center justify-center absolute-center">
            <AudioRing v-if="server.serverState.value === 'streaming'" :level="audio.audioLevel.value">
              <!-- Central Button When Streaming -->
              <div class="relative flex items-center justify-center">
                <div class="absolute inset-0 bg-error/30 rounded-full blur-md glow-breathe"></div>
                <button ref="centralBtnRef" @click="toggleStreaming" @mouseenter="onCentralBtnHover" @mouseleave="onCentralBtnLeave" class="relative z-10 w-[72px] h-[72px] rounded-full bg-error flex items-center justify-center shadow-[0_0_16px_0] shadow-error/20 transition-all duration-300 group-hover:bg-error/90 border border-white/10 hover:shadow-[0_0_24px_4px] hover:shadow-error/30">
                  <Unlink class="w-7 h-7 text-on-error" stroke-width="2.5" />
                </button>
              </div>
            </AudioRing>

            <div v-else class="relative w-full h-full flex items-center justify-center">
              <button ref="centralBtnRef" @click="toggleStreaming" @mouseenter="onCentralBtnHover" @mouseleave="onCentralBtnLeave" class="relative z-10 w-16 h-16 rounded-full flex items-center justify-center shadow-[0_0_24px_2px] transition-all duration-300 hover:-translate-y-1 active:scale-95 border border-white/10 hover:shadow-[0_0_32px_6px] hover:shadow-primary/40"
                      :class="server.serverState.value === 'starting' ? 'bg-secondary shadow-secondary/30 text-on-secondary' : (server.serverState.value === 'connecting' ? 'bg-tertiary shadow-tertiary/30 text-on-tertiary' : 'bg-primary shadow-primary/30 text-on-primary')">
                <RefreshCw v-if="server.serverState.value === 'connecting'" class="w-7 h-7 animate-spin-slow" stroke-width="2.5" />
                <Loader2 v-else-if="server.serverState.value === 'starting'" class="w-7 h-7 animate-spin" stroke-width="2.5" />
                <Link v-else class="w-7 h-7" stroke-width="2.5" />
              </button>
            </div>
          </div>
        </div>

        <!-- Right Panel (Monitoring) -->
        <Transition name="panel">
          <div v-if="audio.showMonitoringPanel.value" class="w-[28%] min-w-0">
            <MonitoringPanel :serverState="server.serverState.value" :audioLevel="audio.audioLevel.value" :metrics="audio.audioMetrics.value" />
          </div>
        </Transition>
      </div>

      <!-- Bottom Bar -->
      <div class="haze-surface rounded-2xl p-2 flex justify-between items-center shrink-0">
        <div class="flex items-center px-3">
          <div class="w-2 h-2 rounded-full mr-2" :class="server.serverState.value === 'streaming' ? 'bg-primary dot-pulse shadow-[0_0_8px_hsl(var(--primary))]' : (server.serverState.value === 'starting' ? 'bg-secondary animate-pulse shadow-[0_0_8px_hsl(var(--secondary))]' : (server.serverState.value === 'connecting' ? 'bg-tertiary animate-pulse shadow-[0_0_8px_hsl(var(--tertiary))]' : 'bg-on-surface-variant'))"></div>
          <span class="text-xs font-bold uppercase tracking-wider text-on-surface-variant transition-colors duration-300">{{ server.serverState.value === 'streaming' ? $t('app.status.stateStreaming') : (server.serverState.value === 'connecting' ? $t('app.status.stateConnecting') : (server.serverState.value === 'starting' ? $t('app.status.stateStarting') : $t('app.status.stateIdle'))) }}</span>
        </div>

        <div class="flex items-center gap-2 pr-1">
          <button
            @click="audio.toggleMute()"
            class="w-10 h-10 rounded-full flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90"
            :class="audio.isMuted.value ? 'bg-error/20 text-error hover:bg-error/30' : 'bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant'"
            :title="audio.isMuted.value ? $t('app.status.unmute') : $t('app.status.mute')"
          >
            <VolumeX v-if="audio.isMuted.value" class="w-4 h-4" />
            <Volume2 v-else class="w-4 h-4" />
          </button>

          <button
            @click="audio.toggleMonitoringEnabled()"
            class="w-10 h-10 rounded-full flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90"
            :class="audio.isMonitoringEnabled.value ? 'bg-primary/20 text-primary hover:bg-primary/30 ring-2 ring-primary/40' : 'bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant'"
            :title="audio.isMonitoringEnabled.value ? $t('app.status.disableEarback') : $t('app.status.enableEarback')"
          >
            <Headphones class="w-4 h-4" />
          </button>

          <button @click="audio.showMonitoringPanel.value = !audio.showMonitoringPanel.value" class="w-10 h-10 rounded-full flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90" :class="audio.showMonitoringPanel.value ? 'bg-primary/20 text-primary hover:bg-primary/30' : 'bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant'" :title="$t('app.monitoring.title')">
            <MonitoringIcon class="w-4 h-4" />
          </button>

          <button @click="openSettings" class="w-10 h-10 rounded-full bg-surface-variant/40 hover:bg-surface-variant flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90">
            <Settings class="w-4 h-4 text-on-surface-variant" />
          </button>
        </div>
      </div>
    </div>



    <UdpWarningDialog
      :show="audio.showUdpWarning.value"
      :port="Number(server.serverPort.value) + 1"
      @close="audio.showUdpWarning.value = false"
    />

    <MonitoringWarningDialog
      :show="audio.showMonitoringWarning.value"
      @confirm="audio.handleMonitoringWarningConfirm"
      @cancel="audio.handleMonitoringWarningCancel"
      @update:show="val => audio.showMonitoringWarning.value = val"
    />

    <CloseConfirmDialog v-model:show="win.showCloseConfirm.value" @select="win.handleCloseSelect" />

    <ConnectionErrorDialog
      :show="server.showErrorDialog.value"
      :details="server.errorDetails.value"
      @dismiss="server.showErrorDialog.value = false"
      @retry="server.showErrorDialog.value = false; toggleStreaming()"
    />

    <QrCodeDialog
      :show="server.showQrDialog.value"
      :qr-data-url="server.qrDataUrl.value"
      :web-url="server.webUrl.value"
      :client-count="server.webClientCount.value"
      @dismiss="server.showQrDialog.value = false"
    />

    <!-- IP Switch Confirmation Dialog -->
    <Transition
      enter-active-class="transition ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div v-if="server.showIpSwitchConfirm.value" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs">
        <div class="bg-surface rounded-2xl shadow-2xl border border-outline/10 p-6 w-80">
          <h3 class="text-sm font-bold text-foreground mb-2">{{ t('app.ipSelector.switchConfirmTitle') }}</h3>
          <p class="text-xs text-on-surface-variant mb-5">{{ t('app.ipSelector.switchConfirmMessage') }}</p>
          <div class="flex justify-end gap-2">
            <button
              class="px-4 py-2 text-xs font-medium text-on-surface-variant hover:bg-surface-variant/50 rounded-lg transition-colors"
              @click="server.showIpSwitchConfirm.value = false"
            >
              {{ t('app.ipSelector.cancel') }}
            </button>
            <button
              class="px-4 py-2 text-xs font-medium text-on-primary bg-primary hover:bg-primary/90 rounded-lg transition-colors"
              @click="server.confirmIpSwitch()"
            >
              {{ t('app.ipSelector.continue') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- USB Device Selector Dialog -->
    <Transition
      enter-active-class="transition ease-out duration-200"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div v-if="server.showDeviceSelector.value" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs">
        <div class="bg-surface rounded-2xl shadow-2xl border border-outline/10 p-6 w-96 max-h-[80vh] flex flex-col">
          <h3 class="text-sm font-bold text-foreground mb-2">{{ t('app.deviceSelector.title') }}</h3>
          <p class="text-xs text-on-surface-variant mb-4">{{ t('app.deviceSelector.desc') }}</p>

          <div class="flex-1 overflow-y-auto space-y-2 mb-4">
            <button
              v-for="device in server.adbDevices.value"
              :key="device.serial"
              class="w-full text-left px-4 py-3 rounded-xl bg-surface-variant/30 hover:bg-surface-variant/60 border border-outline/10 hover:border-primary/30 transition-all duration-200 group"
              @click="server.selectAdbDevice(device.serial)"
            >
              <div class="flex items-center justify-between">
                <div class="flex-1 min-w-0">
                  <div class="text-sm font-medium text-foreground truncate">
                    {{ device.description || device.serial }}
                  </div>
                  <div class="text-xs text-on-surface-variant mt-0.5">
                    {{ device.serial }}
                  </div>
                </div>
                <div class="ml-3 shrink-0">
                  <div class="w-2 h-2 rounded-full bg-success animate-pulse"></div>
                </div>
              </div>
            </button>
          </div>

          <div class="flex justify-end">
            <button
              class="px-4 py-2 text-xs font-medium text-on-surface-variant hover:bg-surface-variant/50 rounded-lg transition-colors"
              @click="server.cancelDeviceSelection()"
            >
              {{ t('app.deviceSelector.cancel') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>
