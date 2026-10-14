import { invoke } from '@tauri-apps/api/core';
import type {
  AecStatus,
  AudioDspSettings,
  BlackHoleResult,
  BlackHoleStatus,
  BlurRect,
  InstalledTheme,
  MenuNode,
  ModeStatus,
  NetworkInfo,
  NetworkInterfaceInfo,
  PipeWireStatus,
  PluginPreview,
  PluginSyncStatus,
  PluginUpdate,
  PluginView,
  ServerPrefs,
  StreamingStatus,
  SystemAccentColor,
  TrayMenuStrings,
  TrayState,
  UpdateCheckResult,
  UsbModeResult,
  VBCableResult,
} from './types';

type NoArgs = Record<string, never>;

/**
 * Every backend command the frontend calls: arguments (camelCase, Tauri maps
 * them to the Rust snake_case parameters) and the resolved value.
 */
export interface Commands {
  // Server lifecycle
  start_server: {
    args: { port: number; mode: string; bindAddress: string | null; outputDevice: string | null };
    result: string;
  };
  stop_server: { args: NoArgs; result: string };
  get_streaming_status: { args: NoArgs; result: StreamingStatus };
  set_mute_state: { args: { isMuted: boolean }; result: void };
  set_monitoring: { args: { enabled: boolean }; result: void };
  set_spectrum_streaming: { args: { enabled: boolean }; result: void };
  exit_app: { args: NoArgs; result: void };

  // Audio devices and DSP
  get_audio_devices: { args: NoArgs; result: string[] };
  get_audio_settings: { args: NoArgs; result: Partial<AudioDspSettings> };
  get_aec_status: { args: NoArgs; result: AecStatus };
  update_audio_settings: { args: { settings: AudioDspSettings }; result: string };
  get_server_prefs: { args: NoArgs; result: ServerPrefs };
  save_server_prefs: { args: { prefs: ServerPrefs }; result: string };
  check_pipewire: { args: NoArgs; result: PipeWireStatus };
  check_vbcable: { args: NoArgs; result: boolean };
  install_vbcable: { args: NoArgs; result: VBCableResult };
  check_blackhole: { args: NoArgs; result: BlackHoleStatus };
  set_blackhole_as_input: { args: NoArgs; result: BlackHoleResult };
  restore_input_device: { args: NoArgs; result: BlackHoleResult };

  // Network
  enable_usb_mode: { args: { port: number; deviceSerial: string | null }; result: UsbModeResult };
  get_network_info: { args: NoArgs; result: NetworkInfo };
  get_network_interfaces: { args: NoArgs; result: NetworkInterfaceInfo[] };
  allow_firewall: { args: NoArgs; result: void };

  // Run mode (GUI / CLI / TUI) and shared prefs
  get_mode_status: { args: NoArgs; result: ModeStatus };
  switch_to_cli: { args: NoArgs; result: void };
  switch_to_tui: { args: NoArgs; result: void };
  save_ui_prefs: { args: { language: string; themeColor: string }; result: void };
  save_theme_colors: {
    args: {
      primary: string;
      secondary: string;
      tertiary: string;
      surface: string;
      surfaceVariant: string;
      onSurface: string;
      error: string;
    };
    result: void;
  };

  // Theme
  get_system_accent_color: { args: NoArgs; result: SystemAccentColor };
  install_theme: { args: { themeId: string; manifestJson: string; css: string }; result: void };
  list_installed_themes: { args: NoArgs; result: string[] };
  get_installed_theme: { args: { themeId: string }; result: InstalledTheme };
  remove_installed_theme: { args: { themeId: string }; result: void };

  // About and logs
  get_app_version: { args: NoArgs; result: string };
  check_app_update: { args: { cdk: string | null }; result: UpdateCheckResult };
  get_sponsors: { args: NoArgs; result: string };
  export_log: { args: NoArgs; result: void };
  get_log_path: { args: NoArgs; result: string };
  get_log_content: { args: NoArgs; result: string };
  open_log_dir: { args: NoArgs; result: string };

  // Plugins
  list_plugins: { args: NoArgs; result: PluginView[] };
  set_plugin_enabled: { args: { id: string; enabled: boolean }; result: void };
  uninstall_plugin: { args: { id: string }; result: void };
  get_plugin_config: { args: { id: string }; result: Record<string, unknown> };
  set_plugin_config: { args: { id: string; key: string; value: unknown }; result: void };
  get_plugin_logs: { args: { id: string }; result: string[] };
  get_plugin_sync_status: { args: NoArgs; result: PluginSyncStatus };
  get_plugin_panel_icons: { args: { id: string }; result: Record<string, string> };
  get_plugin_panel: { args: { pluginId: string; panelId: string }; result: string };
  get_app_locale: { args: NoArgs; result: string };
  open_plugins_dir: { args: NoArgs; result: string };
  plugin_trigger: {
    args: { pluginId: string; action: string; payload: string | null };
    result: void;
  };
  preview_plugin_zip: { args: { zipPath: string }; result: PluginPreview };
  preview_plugin_from_url: { args: { manifestUrl: string }; result: PluginPreview };
  install_plugin_from_url: { args: { id: string; zipUrl: string }; result: string };
  cancel_plugin_download: { args: { id: string }; result: void };
  check_plugin_updates: { args: NoArgs; result: PluginUpdate[] };
  update_plugin: { args: { id: string }; result: string };
  import_plugin: { args: { source: string }; result: string };

  // Windows and tray
  set_tray_strings: { args: { strings: TrayMenuStrings }; result: void };
  set_tray_state: { args: { state: TrayState }; result: void };
  start_window_drag: { args: NoArgs; result: void };
  set_window_blur: { args: { regions: BlurRect[] }; result: boolean };
  set_window_shadow: { args: { radius: number | null }; result: boolean };
  open_settings_window: { args: { title: string }; result: void };
  show_main_window: { args: NoArgs; result: void };
  minimize_main_window: { args: NoArgs; result: void };
  hide_main_window: { args: NoArgs; result: void };

  // macOS chrome: the app menu, the native window controls and the Dock badge.
  // The backend ignores all three on other platforms.
  set_app_menu: { args: { menu: MenuNode[] }; result: void };
  apply_macos_window_layout: { args: { mode: 'full' | 'pocket' }; result: void };
  set_dock_badge: { args: { label: string | null }; result: void };
}

export type CommandName = keyof Commands;

type ArgsParam<K extends CommandName> = Commands[K]['args'] extends NoArgs
  ? []
  : [args: Commands[K]['args']];

/** Calls a backend command; rejects with the error string the command returned. */
export function command<K extends CommandName>(
  name: K,
  ...args: ArgsParam<K>
): Promise<Commands[K]['result']> {
  return invoke<Commands[K]['result']>(name, args[0]);
}
