// Payloads exchanged with the Rust backend. Field names follow the serde
// output of the matching Rust types (most are camelCase, a few are not).

// ---- Server and connection ----

export interface ServerPrefs {
  port: number;
  webPort: number;
  mode: string;
  bindAddress: string;
  autoBind: boolean;
  outputDevice: string;
  muteSync: boolean;
}

/** Platform the phone reported when it connected; `unknown` for clients that predate the field. */
export type ClientPlatform = 'android' | 'ios' | 'unknown';

export interface StreamingStatus {
  isServerRunning: boolean;
  isConnected: boolean;
  isMuted: boolean;
  platform: ClientPlatform;
}

export interface DeviceInfo {
  name: string;
  ip: string;
  latency: number;
  platform: ClientPlatform;
}

export interface NetworkInfo {
  ips: string[];
  port: number;
}

export interface NetworkInterfaceInfo {
  ip: string;
  interface_name: string;
}

export interface AdbDevice {
  serial: string;
  state: string;
  description: string;
}

export type UsbModeResult =
  | { type: 'Success' }
  | { type: 'NoDevices' }
  | { type: 'MultipleDevices'; devices: AdbDevice[] };

export interface ModeStatus {
  mode: 'gui' | 'cli' | 'tui' | 'none';
  pid: number | null;
  running: boolean;
}

// ---- Audio ----

export interface EqualizerConfig {
  enabled: boolean;
  preAmp: number;
  gains: number[];
}

export interface AudioDspSettings {
  gain: number;
  nsEnabled: boolean;
  nsType: string;
  nsIntensity: number;
  dereverbEnabled: boolean;
  dereverbLevel: number;
  agcEnabled: boolean;
  agcTarget: number;
  agcAttack: number;
  agcDecay: number;
  vadEnabled: boolean;
  vadThreshold: number;
  aecEnabled: boolean;
  outputBufferMs: number;
  processingChain: string[];
  equalizer: EqualizerConfig;
}

export interface AudioMetrics {
  bitrate: number;
  sampleRate: number;
  latencyMs: number;
  networkLatencyMs: number;
  packetLossRate: number;
  jitterMs: number;
  bufferDurationMs: number;
}

export interface SpectrumPayload {
  raw: number[];
  processed: number[];
}

export interface AecStatus {
  available: boolean;
  enabled: boolean;
  reason?: string | null;
}

export interface VBCableResult {
  success: boolean;
  error_type?: string | null;
  message?: string | null;
}

export interface BlackHoleStatus {
  installed: boolean;
  switch_audio_source: boolean;
  device_name: string | null;
}

export interface BlackHoleResult {
  success: boolean;
  message?: string | null;
}

export interface PipeWireStatus {
  available: boolean;
  setup: boolean;
  device_exists: boolean;
  install_command?: string;
  distro?: string;
}

// ---- Theme ----

export interface SystemAccentColor {
  hex: string;
  source: string;
  supported: boolean;
}

export interface InstalledTheme {
  css: string;
  controlsThemeColor: boolean;
}

export interface ThemeColors {
  primary: string;
  secondary: string;
  tertiary: string;
  surface: string;
  surfaceVariant: string;
  onSurface: string;
  error: string;
}

// ---- Plugins ----

export interface PluginDependency {
  id: string;
  version?: string;
  optional?: boolean;
}

export interface PluginUpdate {
  id: string;
  currentVersion: string;
  latestVersion: string;
  updateUrl: string;
}

export interface PluginPreview {
  id: string;
  name: string;
  version: string;
  author?: string | null;
  description?: string | null;
  runtime: string;
  kind: string;
  capabilities: string[];
  license?: string | null;
  homepage?: string | null;
}

export interface PluginView {
  id: string;
  name: string;
  version: string;
  author?: string | null;
  description?: string | null;
  runtime: string; // native | wasm
  kind: string; // dsp | utility | ui | bridge
  platforms: string[];
  capabilities: string[];
  ui?: {
    route: string;
    label?: string;
    entry?: string | null;
    panels?: Array<{ id: string; label: string; entry: string; sidebar?: boolean }>;
  } | null;
  enabled: boolean;
  loaded: boolean;
  dspNode: boolean;
  error?: string | null;
  nameI18n?: Record<string, string>;
  descriptionI18n?: Record<string, string>;
  dependencies?: PluginDependency[];
  configSchema?: {
    fields: Array<{
      key: string;
      fieldType: string;
      label?: string | null;
      description?: string | null;
      default?: unknown;
      min?: number;
      max?: number;
      step?: number;
      options?: Array<{ value: string; label?: string | null }>;
    }>;
  };
}

export interface PluginSyncStatus {
  deviceConnected: boolean;
  transportReady: boolean;
}

export interface PluginDownloadProgress {
  id: string;
  downloaded: number;
  total: number;
  done: boolean;
}

// ---- App ----

export interface UpdateCheckResult {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  releaseNotes?: string | null;
  isMirror: boolean;
  cdkExpiredTime?: number | null;
}

// ---- Window and tray ----

export interface TrayMenuStrings {
  tooltip: string;
  show: string;
  hide: string;
  start: string;
  stop: string;
  exit: string;
  switchCli: string;
  switchTui: string;
}

export interface TrayState {
  windowVisible: boolean;
  isStreaming: boolean;
}

/** Native macOS menu entries the frontend may request by name. */
export type PredefinedName =
  | 'services'
  | 'hide'
  | 'hideOthers'
  | 'showAll'
  | 'quit'
  | 'undo'
  | 'redo'
  | 'cut'
  | 'copy'
  | 'paste'
  | 'selectAll'
  | 'minimize'
  | 'maximize'
  | 'fullscreen'
  | 'closeWindow';

/**
 * One node of the macOS app menu descriptor.
 *
 * The whole menu is sent on every change (never as a patch), so the backend
 * stays stateless: the frontend owns the labels, the checked state and the
 * behaviour, and only the native menu object lives in Rust.
 */
export type MenuNode =
  | { kind: 'item'; id: string; label: string; enabled?: boolean; accelerator?: string }
  | { kind: 'check'; id: string; label: string; checked: boolean; enabled?: boolean }
  | { kind: 'separator' }
  | {
      kind: 'submenu';
      label: string;
      enabled?: boolean;
      role?: 'window' | 'help';
      items: MenuNode[];
    }
  | { kind: 'predefined'; name: PredefinedName; label?: string };

export interface BlurRect {
  x: number;
  y: number;
  width: number;
  height: number;
  radius: number;
}
