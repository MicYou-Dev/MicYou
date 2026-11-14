import { ref, computed, onMounted, watch, type Ref } from 'vue';
import { useStorage } from '@vueuse/core';
import { useI18n } from 'vue-i18n';
import QRCode from 'qrcode';
import { analyzeError, generateErrorDetails, type ConnectionErrorDetails } from '../utils/connectionError';
import { muteSyncEnabled } from './useMuteSync';
import { isMacOS } from '@/shared/lib/os';
import { useListeners } from '@/shared/lib/listeners';
import {
  command,
  notify,
  onEvent,
  type AdbDevice,
  type ClientPlatform,
  type NetworkInfo,
  type NetworkInterfaceInfo,
} from '@/platform';

// Connection modes supported by the application
export type ConnectionMode = 'wifi' | 'usb' | 'web';

// Backend server streaming states
export type ServerState = 'idle' | 'starting' | 'connecting' | 'streaming';

const aecFailureNotificationKeys: Record<string, string> = {
  inference_failed: 'app.notify.aecDisabledInferenceFailed',
  model_load_failed: 'app.notify.aecDisabledModelLoadFailed',
  permission_denied: 'app.notify.aecDisabledPermissionDenied',
  pipewire_unavailable: 'app.notify.aecDisabledPipeWireUnavailable',
  reference_lost: 'app.notify.aecDisabledReferenceLost',
  unsupported_os: 'app.notify.aecDisabledUnsupportedOs',
  virtual_source_missing: 'app.notify.aecDisabledVirtualSourceMissing',
};

function aecFailureNotificationKey(reason?: string | null) {
  return (reason && aecFailureNotificationKeys[reason]) || 'app.notify.aecDisabled';
}

/**
 * Composable for managing the connection server, IP configurations, modes, and device scanning
 */
export function useServer(options?: { audioLevel?: Ref<number>; isMuted?: Ref<boolean> }) {
  const { t } = useI18n();

  // Current server connection state
  const serverState = ref<ServerState>('idle');
  
  // Selected connection mode persisted in storage
  const connectionMode = useStorage<ConnectionMode>('micyou_connectionMode', 'wifi');
  
  // Port for streaming under Wi-Fi or USB modes
  const serverPort = useStorage<number>('micyou_serverPort', 8554);
  
  // Port for Web RTC / HTTPS server stream
  const webPort = useStorage<number>('micyou_webPort', 8443);

  // Preserve advanced origins configured in server.json when saving GUI prefs.
  let webAllowedOrigins: string[] = [];
  
  // Number of clients currently connected to the Web interface
  const webClientCount = ref(0);
  
  // Complete HTTPS URL for the web stream
  const webUrl = ref('');
  
  // Data URI base64 of the QR code for easy connection scanning
  const qrDataUrl = ref('');
  
  // Flag indicating if desktop OS notification alerts are enabled
  const notificationsEnabled = useStorage<boolean>('micyou_notifications', true);
  
  // Cache of primary network information returned by backend
  const networkInfo = ref<NetworkInfo | null>(null);
  
  // Currently selected bind IP (if isAutoBind is false)
  const selectedIp = ref<string>('0.0.0.0');
  
  // List of active network adapters and interfaces detected by the OS
  const networkInterfaces = ref<NetworkInterfaceInfo[]>([]);
  
  // UI triggers for menus, confirm popups and dialogs
  const showIpMenu = ref(false);
  const showIpSwitchConfirm = ref(false);
  const pendingIp = ref('');
  const pendingAutoSelect = ref(false);
  
  // Whether to listen on all interfaces (auto-bind) or a selected static IP interface
  const isAutoBind = ref(true);
  
  // USB mode target select overlay triggers
  const showDeviceSelector = ref(false);
  const adbDevices = ref<AdbDevice[]>([]);
  const pendingUsbPort = ref<number>(0);
  
  // Error handling triggers
  const showErrorDialog = ref(false);
  const errorDetails = ref<ConnectionErrorDetails | null>(null);
  
  // Selected audio output device target (e.g. system default or virtual sound card)
  const outputDevice = ref<string>('');
  const showQrDialog = ref(false);

  // Active configurations when the server is running
  const activeConnectionMode = ref<ConnectionMode | null>(null);
  const activePort = ref<number | null>(null);

  // Platform of the connected phone, reported in its Connect message
  const clientPlatform = ref<ClientPlatform>('unknown');

  // Computes the display representation of the active bind IP address
  const displayIp = computed(() => {
    if (isAutoBind.value) {
      return networkInterfaces.value.length > 0 ? networkInterfaces.value[0].ip : '...';
    }
    return selectedIp.value;
  });

  // Dynamic text description showing the status of the connection
  const statusDescription = computed(() => {
    const mode = activeConnectionMode.value || connectionMode.value;
    const port = activePort.value || (mode === 'web' ? webPort.value : serverPort.value);

    if (serverState.value === 'streaming') {
      if (mode === 'web') {
        return t('app.web.clientsConnected', { count: webClientCount.value });
      }
      if (clientPlatform.value === 'ios') return t('app.status.streamingDescIos');
      if (clientPlatform.value === 'android') return t('app.status.streamingDescAndroid');
      return t('app.status.streamingDesc');
    }
    if (serverState.value === 'connecting') {
      return t('app.status.connectingDesc', { port: port });
    }
    if (serverState.value === 'starting') return t('app.status.startingDesc');
    return t('app.status.readyDesc');
  });

  /**
   * Checks if the server is in any active (non-idle) states
   */
  function isStreaming(v: ServerState) {
    return v === 'streaming' || v === 'connecting' || v === 'starting';
  }

  /**
   * Generates a base64 QR code image from a given target URL
   */
  async function generateQrCode(url: string) {
    try {
      qrDataUrl.value = await QRCode.toDataURL(url, {
        width: 200,
        margin: 1,
        color: { dark: '#000000', light: '#ffffff' }
      });
    } catch (e) {
      console.error('QR generation failed:', e);
      qrDataUrl.value = '';
    }
  }

  function ensureStreamingState() {
    if (serverState.value === 'streaming') return;
    serverState.value = 'streaming';
    if (notificationsEnabled.value) {
      void notify(t('app.notify.connected'));
    }
  }

  function resetToIdle() {
    serverState.value = 'idle';
    activeConnectionMode.value = null;
    activePort.value = null;
    if (options?.audioLevel) options.audioLevel.value = 0;
  }

  async function stopQuietly() {
    try {
      await command('stop_server');
    } catch (e) {
      console.warn('Failed to stop server cleanly:', e);
    }
  }

  function showStartError(message: string, mode: ConnectionMode, port: number) {
    const type = analyzeError(message);
    errorDetails.value = generateErrorDetails(type, message, mode, port, selectedIp.value, t);
    showErrorDialog.value = true;
  }

  function errorMessage(e: unknown): string {
    if (typeof e === 'string') return e;
    return (e as Error | undefined)?.message ?? String(e);
  }

  function portFor(mode: ConnectionMode): number {
    return Number(mode === 'web' ? webPort.value : serverPort.value);
  }

  // USB mode goes through adb reverse which forwards to 127.0.0.1, so the
  // server must listen on all interfaces regardless of the selected IP.
  function bindAddressFor(mode: ConnectionMode): string | null {
    return mode === 'usb' || isAutoBind.value ? null : selectedIp.value;
  }

  function hostForUrl(ip: string): string {
    return ip.includes(':') ? `[${ip}]` : ip;
  }

  /**
   * Starts the server in `mode` and finishes the mode-specific setup.
   * `deviceSerial` picks the adb device when several are attached. Errors
   * stop the server again and open the connection error dialog.
   */
  async function startServer(mode: ConnectionMode, port: number, deviceSerial: string | null = null) {
    serverState.value = 'starting';
    activeConnectionMode.value = mode;
    activePort.value = port;
    try {
      await command('start_server', {
        port,
        mode,
        bindAddress: bindAddressFor(mode),
        outputDevice: outputDevice.value && outputDevice.value !== 'auto' && outputDevice.value !== 'default'
          ? outputDevice.value
          : null,
      });
      // Auto-switch to BlackHole input on macOS for seamless virtual audio loopback
      if (isMacOS) {
        try { await command('set_blackhole_as_input'); } catch { /* best-effort, ignore */ }
      }
      if (mode === 'usb') {
        const result = await command('enable_usb_mode', { port, deviceSerial });
        if (result.type === 'MultipleDevices') {
          await stopQuietly();
          resetToIdle();
          adbDevices.value = result.devices;
          pendingUsbPort.value = port;
          showDeviceSelector.value = true;
          return;
        }
        if (result.type === 'NoDevices') {
          throw new Error('No USB devices found. Connect an Android device with USB debugging enabled, or an iPhone or iPad.');
        }
      }
      if (mode === 'web') {
        const ip = isAutoBind.value ? networkInfo.value?.ips[0] : selectedIp.value;
        webUrl.value = `https://${hostForUrl(ip ?? 'localhost')}:${port}`;
        void generateQrCode(webUrl.value);
      }
      const status = await command('get_streaming_status').catch(() => null);
      if (status?.isConnected) {
        clientPlatform.value = status.platform;
        ensureStreamingState();
      } else if (serverState.value === 'starting') {
        serverState.value = 'connecting';
      }
    } catch (e) {
      console.error(e);
      await stopQuietly();
      resetToIdle();
      showStartError(errorMessage(e), mode, port);
    }
  }

  /**
   * Toggles the server state between started and stopped
   */
  const toggleStreaming = async () => {
    // PipeWire setup and audio initialization can take a moment. Do not let a
    // second click (or an auto-start racing with a manual click) stop the
    // server that is still starting.
    if (serverState.value === 'starting') {
      console.warn('[Server] Ignoring toggle while server startup is in progress');
      return;
    }

    if (serverState.value !== 'idle') {
      await stopQuietly();
      resetToIdle();
      // Restore original input device on macOS when using BlackHole virtual audio
      if (isMacOS) {
        try { await command('restore_input_device'); } catch { /* best-effort, ignore */ }
      }
      return;
    }

    const mode = connectionMode.value;
    await startServer(mode, portFor(mode));
  };

  /**
   * Sets bind IP target or prompts user if they try to switch while server is active
   */
  const selectIp = (ip: string, autoSelect: boolean) => {
    showIpMenu.value = false;
    if (autoSelect && isAutoBind.value) return;
    if (!autoSelect && !isAutoBind.value && selectedIp.value === ip) return;
    if (serverState.value === 'streaming' || serverState.value === 'connecting') {
      pendingIp.value = ip;
      pendingAutoSelect.value = autoSelect;
      showIpSwitchConfirm.value = true;
    } else {
      applyIpSelection(ip, autoSelect);
    }
  };

  /**
   * Sets active bind variables directly
   */
  const applyIpSelection = (ip: string, autoSelect: boolean) => {
    isAutoBind.value = autoSelect;
    selectedIp.value = autoSelect ? '0.0.0.0' : ip;
  };

  /**
   * Switches IP and restarts the server with the new bind target
   */
  const confirmIpSwitch = async () => {
    applyIpSelection(pendingIp.value, pendingAutoSelect.value);
    showIpSwitchConfirm.value = false;
    if (serverState.value !== 'streaming' && serverState.value !== 'connecting') return;
    await stopQuietly();
    resetToIdle();
    const mode = connectionMode.value;
    await startServer(mode, portFor(mode));
  };

  /**
   * Targets a specific discovered USB/ADB device
   */
  const selectAdbDevice = async (serial: string) => {
    showDeviceSelector.value = false;
    await startServer('usb', pendingUsbPort.value, serial);
  };

  /**
   * Cancels ongoing ADB device selection flow
   */
  const cancelDeviceSelection = () => {
    showDeviceSelector.value = false;
    adbDevices.value = [];
    pendingUsbPort.value = 0;
  };

  const track = useListeners();

  // ---- Shared server prefs (server.json, also read/written by the CLI) ----
  async function loadServerPrefs() {
    try {
      const prefs = await command('get_server_prefs');
      if (!prefs) return;
      if (prefs.port) serverPort.value = prefs.port;
      if (prefs.webPort) webPort.value = prefs.webPort;
      webAllowedOrigins = prefs.webAllowedOrigins ?? [];
      if (prefs.mode && ['wifi', 'usb', 'web'].includes(prefs.mode)) {
        connectionMode.value = prefs.mode as ConnectionMode;
      }
      if (prefs.autoBind !== undefined) isAutoBind.value = prefs.autoBind;
      if (prefs.bindAddress && prefs.bindAddress !== '0.0.0.0') {
        selectedIp.value = prefs.bindAddress;
      }
      if (prefs.outputDevice) outputDevice.value = prefs.outputDevice;
      if (prefs.muteSync !== undefined) muteSyncEnabled.value = prefs.muteSync;
    } catch (e) {
      console.error('Failed to load server prefs:', e);
    }
  }

  let prefsSaveTimer: ReturnType<typeof setTimeout> | null = null;
  function persistServerPrefs() {
    if (prefsSaveTimer) clearTimeout(prefsSaveTimer);
    prefsSaveTimer = setTimeout(() => {
      void command('save_server_prefs', {
        prefs: {
          port: Number(serverPort.value),
          webPort: Number(webPort.value),
          webAllowedOrigins,
          mode: connectionMode.value,
          bindAddress: isAutoBind.value ? '0.0.0.0' : selectedIp.value,
          autoBind: isAutoBind.value,
          outputDevice: outputDevice.value || '',
          muteSync: muteSyncEnabled.value,
        },
      }).catch((e) => console.error('Failed to save server prefs:', e));
    }, 500);
  }
  watch(
    [connectionMode, serverPort, webPort, isAutoBind, selectedIp, outputDevice, muteSyncEnabled],
    persistServerPrefs,
  );

  /** A sign of life from the phone confirms a start that is still waiting. */
  function healPendingStart() {
    if (serverState.value === 'connecting' || serverState.value === 'starting') {
      ensureStreamingState();
    }
  }

  function onDeviceDisconnected() {
    if (serverState.value !== 'streaming' && serverState.value !== 'connecting') return;
    const wasStreaming = serverState.value === 'streaming';
    const mode = activeConnectionMode.value || connectionMode.value;
    if (mode === 'usb') {
      // The adb reverse tunnel is gone with the cable: stop instead of waiting.
      resetToIdle();
      if (options?.isMuted) options.isMuted.value = false;
      void stopQuietly();
      if (wasStreaming && notificationsEnabled.value) {
        void notify(t('app.notify.usbDisconnected'));
      }
      return;
    }
    serverState.value = 'connecting';
    if (options?.audioLevel) options.audioLevel.value = 0;
    if (wasStreaming && notificationsEnabled.value) {
      void notify(t('app.notify.disconnected'));
    }
  }

  // Registered before the initial status query so no transition that
  // happens while the window loads is missed.
  function listenToServer() {
    return Promise.all([
      track(onEvent('device-connected', (info) => {
        clientPlatform.value = info.platform;
        ensureStreamingState();
      })),
      track(onEvent('device-disconnected', onDeviceDisconnected)),
      // Stops triggered elsewhere (tray, plugins, another window)
      track(onEvent('server-stopped', () => {
        resetToIdle();
        if (options?.isMuted) options.isMuted.value = false;
      })),
      // Metrics only flow while a TCP session is up
      track(onEvent('audio-metrics', healPendingStart)),
      track(onEvent('audio-level', (payload) => {
        if (payload > 0) healPendingStart();
      })),
      track(onEvent('web-client-count', (payload) => {
        webClientCount.value = payload;
        if (payload > 0) {
          healPendingStart();
        } else if (
          (activeConnectionMode.value ?? connectionMode.value) === 'web' &&
          serverState.value === 'streaming'
        ) {
          serverState.value = 'connecting';
        }
      })),
      track(onEvent('aec-status-changed', (payload) => {
        if (!payload.available && notificationsEnabled.value) {
          void notify(t(aecFailureNotificationKey(payload.reason)));
        }
      })),
    ]);
  }

  onMounted(async () => {
    try {
      await listenToServer();
    } catch (e) {
      console.error('Failed to listen for server events:', e);
    }

    await loadServerPrefs();

    try {
      networkInfo.value = await command('get_network_info');
      // Keep a bind address saved in server.json; only fill in a manual
      // selection that has no concrete address yet.
      const firstIp = networkInfo.value?.ips[0];
      if (firstIp && !isAutoBind.value && selectedIp.value === '0.0.0.0') {
        selectedIp.value = firstIp;
      }
    } catch (e) {
      console.error('Failed to get network info:', e);
    }

    try {
      networkInterfaces.value = await command('get_network_interfaces');
    } catch (e) {
      console.error('Failed to get network interfaces:', e);
    }

    // Sync with a server that is already running (window reload, GUI reopened)
    try {
      const status = await command('get_streaming_status');
      if (status.isServerRunning) {
        serverState.value = status.isConnected ? 'streaming' : 'connecting';
        clientPlatform.value = status.platform;
        activeConnectionMode.value = connectionMode.value;
        activePort.value = portFor(connectionMode.value);
        if (options?.isMuted) {
          options.isMuted.value = status.isMuted;
        }
      }
    } catch (e) {
      console.error('Failed to get initial server status:', e);
    }

    if (localStorage.getItem('micyou_auto_stream') === 'true' && serverState.value === 'idle') {
      void toggleStreaming();
    }
  });

  return {
    serverState,
    connectionMode,
    serverPort,
    webPort,
    webClientCount,
    webUrl,
    qrDataUrl,
    networkInfo,
    selectedIp,
    networkInterfaces,
    showIpMenu,
    isAutoBind,
    displayIp,
    statusDescription,
    showDeviceSelector,
    adbDevices,
    pendingUsbPort,
    showErrorDialog,
    errorDetails,
    outputDevice,
    showQrDialog,
    notificationsEnabled,
    showIpSwitchConfirm,
    pendingIp,
    pendingAutoSelect,
    isStreaming,
    toggleStreaming,
    selectIp,
    applyIpSelection,
    confirmIpSwitch,
    selectAdbDevice,
    cancelDeviceSelection,
  };
}
