import { nextTick, reactive, ref, watch } from 'vue';
import { command, emitToWindow } from '@/platform';
import { useAecStatus } from '@/features/audio/composables/useAecStatus';

// AEC availability comes from the backend, so the GUI, the CLI and the TUI share
// one answer instead of each deciding what the platform supports. The singleton
// owns the `aec-status-changed` listener for the life of the app, which is why
// this module no longer tracks the event itself.
const {
  aecSupported: isAecSupported,
  aecAvailable: aecRuntimeAvailable,
  aecReason,
} = useAecStatus();

// Single source of truth for audio / DSP defaults: both the initial state and
// "restore defaults" use it, so a new field only has to be added here.
const DEFAULT_AUDIO_SETTINGS = () => ({
  audioDevice: 'auto',
  gain: 0,
  aecEnabled: false,
  nsEnabled: false,
  nsType: 'PureVox',
  nsIntensity: 100,
  dereverbEnabled: false,
  dereverbLevel: 50,
  agcEnabled: false,
  agcTarget: 16000,
  agcAttack: 50,
  agcDecay: 50,
  vadEnabled: false,
  vadThreshold: -40,
  outputBufferMs: 300,
  processingChain: ['AEC', 'NoiseReduction', 'Dereverb', 'Equalizer', 'Amplifier', 'AGC', 'VAD'],
  equalizer: {
    enabled: false,
    preAmp: 0,
    gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
});

// One settings window per webview, so module state is the window's state.
const settings = reactive(DEFAULT_AUDIO_SETTINGS());
const suppressAutosave = ref(false);

/** The main window follows the output device chosen here. */
function notifyOutputDevice(device: string) {
  void emitToWindow('main', 'output-device-changed', device).catch((e) =>
    console.warn('Failed to notify the main window of the output device:', e),
  );
}

/** AEC is pinned first where it is supported and dropped elsewhere. */
function normalizeAec() {
  const chain = settings.processingChain ?? [];
  settings.processingChain = isAecSupported.value
    ? ['AEC', ...chain.filter((i) => i !== 'AEC')]
    : chain.filter((i) => i !== 'AEC');
  if (!isAecSupported.value) settings.aecEnabled = false;
}

async function loadSettings() {
  // Loading only mirrors what is already saved, so it must not autosave:
  // between the two reads the device would briefly be "auto" and the main
  // window would follow it.
  suppressAutosave.value = true;
  try {
    await readSavedSettings();
    // Let the deep watcher flush while still suppressed, so it is a no-op.
    await nextTick();
  } finally {
    suppressAutosave.value = false;
  }
}

async function readSavedSettings() {
  // DSP settings live in the shared settings.json and the output device in
  // server.json, both also used by the CLI and TUI.
  try {
    Object.assign(settings, await command('get_audio_settings'));
  } catch (error) {
    console.error('get_audio_settings failed:', error);
  }
  try {
    const { outputDevice } = await command('get_server_prefs');
    settings.audioDevice = outputDevice && outputDevice !== 'default' ? outputDevice : 'auto';
  } catch (error) {
    console.error('get_server_prefs failed:', error);
  }

  if (!['PureVox', 'RNNoise', 'Speexdsp'].includes(settings.nsType)) {
    settings.nsType = 'PureVox';
  }
  normalizeAec();
}

async function syncSettingsToBackend() {
  try {
    await command('update_audio_settings', {
      settings: {
        gain: settings.gain,
        aecEnabled: isAecSupported.value ? settings.aecEnabled : false,
        nsEnabled: settings.nsEnabled,
        nsType: settings.nsType,
        nsIntensity: settings.nsIntensity,
        dereverbEnabled: settings.dereverbEnabled,
        dereverbLevel: settings.dereverbLevel,
        agcEnabled: settings.agcEnabled,
        agcTarget: settings.agcTarget,
        agcAttack: settings.agcAttack,
        agcDecay: settings.agcDecay,
        vadEnabled: settings.vadEnabled,
        vadThreshold: settings.vadThreshold,
        outputBufferMs: settings.outputBufferMs,
        processingChain: isAecSupported.value
          ? settings.processingChain
          : settings.processingChain.filter((i) => i !== 'AEC'),
        equalizer: settings.equalizer,
      },
    });
  } catch (e) {
    console.error('Failed to sync DSP settings to backend:', e);
  }
}

function saveSettings() {
  // The main window owns server.json and persists the output device.
  notifyOutputDevice(settings.audioDevice);
  void syncSettingsToBackend();
}

// Autosave: any change persists and syncs to the backend. Suppressed while
// restoring defaults so the batched reset produces exactly one save.
watch(
  settings,
  () => {
    if (suppressAutosave.value) return;
    saveSettings();
  },
  { deep: true },
);

async function resetAudioSettings() {
  suppressAutosave.value = true;
  try {
    const defaults = DEFAULT_AUDIO_SETTINGS();
    // Drop keys that are no longer defaults (renamed or removed fields), then overwrite.
    Object.keys(settings).forEach((k) => {
      if (!(k in defaults)) delete (settings as Record<string, unknown>)[k];
    });
    Object.assign(settings, defaults);
    normalizeAec();
    // Let the deep watcher flush while still suppressed, so it is a no-op.
    await nextTick();
  } finally {
    suppressAutosave.value = false;
  }
  saveSettings();
}

export function useAudioSettings() {
  return {
    settings,
    isAecSupported,
    aecRuntimeAvailable,
    aecReason,
    loadSettings,
    syncSettingsToBackend,
    resetAudioSettings,
  };
}
