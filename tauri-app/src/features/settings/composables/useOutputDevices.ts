import { computed, ref } from 'vue';
import {
  command,
  onEvent,
  openUrl,
  type BlackHoleStatus,
  type PipeWireStatus,
  type UnlistenFn,
} from '@/platform';
import { isLinux, isMacOS, isWindows } from '@/shared/lib/os';
import { useAudioSettings } from './useAudioSettings';

export const supportedDistros = [
  { id: 'arch', name: 'Arch / Manjaro', command: 'sudo pacman -S pipewire pipewire-pulse' },
  { id: 'debian', name: 'Debian / Ubuntu', command: 'sudo apt install pipewire pipewire-pulse' },
  { id: 'fedora', name: 'Fedora', command: 'sudo dnf install pipewire pipewire-pulseaudio' },
  { id: 'opensuse', name: 'openSUSE', command: 'sudo zypper install pipewire pipewire-pulseaudio' },
];

const audioDevices = ref<string[]>([]);
const vbcableDetected = ref(false);
const vbcableInstalling = ref(false);
const vbcableInstallProgress = ref('');
const blackholeStatus = ref<BlackHoleStatus>({
  installed: false,
  switch_audio_source: false,
  device_name: null,
});
const pipewireStatus = ref<PipeWireStatus>({
  available: false,
  setup: false,
  device_exists: false,
});
const selectedDistro = ref('debian');

const hasVBCable = computed(() =>
  vbcableDetected.value ||
  audioDevices.value.some((d) => {
    const lower = d.toLowerCase();
    return lower.includes('cable') || lower.includes('vb-audio');
  }),
);
const hasBlackHole = computed(() =>
  blackholeStatus.value.installed ||
  audioDevices.value.some((d) => d.toLowerCase().includes('blackhole')),
);

const currentPipewireInstallCommand = computed(() => {
  const match = supportedDistros.find((d) => d.id === selectedDistro.value);
  if (match) return match.command;
  return pipewireStatus.value.install_command || 'sudo apt install pipewire pipewire-pulse';
});

async function checkVBCableStatus() {
  if (!isWindows) return;
  try {
    vbcableDetected.value = await command('check_vbcable');
  } catch (e) {
    console.error('Failed to check VB-CABLE status:', e);
  }
}

async function checkBlackHoleStatus() {
  if (!isMacOS) return;
  try {
    blackholeStatus.value = await command('check_blackhole');
  } catch (e) {
    console.error('Failed to check BlackHole status:', e);
  }
}

async function checkPipeWireStatus() {
  if (!isLinux) return;
  try {
    pipewireStatus.value = await command('check_pipewire');
    const distro = pipewireStatus.value.distro;
    if (distro && supportedDistros.some((d) => d.id === distro)) {
      selectedDistro.value = distro;
    }
  } catch (e) {
    console.error('Failed to check PipeWire status:', e);
  }
}

async function fetchDevices() {
  try {
    audioDevices.value = await command('get_audio_devices');
  } catch (e) {
    console.error('Failed to fetch audio devices', e);
  }
  void checkVBCableStatus();
  void checkBlackHoleStatus();
  void checkPipeWireStatus();
}

async function installVBCable() {
  vbcableInstalling.value = true;
  vbcableInstallProgress.value = '';
  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await onEvent('vbcable-install-progress', (payload) => {
      vbcableInstallProgress.value = payload;
    });
    const result = await command('install_vbcable');
    if (result.success) {
      await checkVBCableStatus();
      audioDevices.value = await command('get_audio_devices');
    }
  } catch (e) {
    console.error('VB-CABLE install failed:', e);
  } finally {
    unlisten?.();
    vbcableInstalling.value = false;
    vbcableInstallProgress.value = '';
  }
}

export function useOutputDevices() {
  const { settings } = useAudioSettings();

  // "auto" routes to the platform's virtual device when one is present.
  const isVirtualDeviceSelected = computed(() => {
    if (!settings.audioDevice || settings.audioDevice === 'auto') {
      if (isWindows) return hasVBCable.value;
      if (isMacOS) return hasBlackHole.value;
      if (isLinux) return pipewireStatus.value.available;
      return false;
    }
    const dev = settings.audioDevice.toLowerCase();
    return (
      dev.includes('cable') ||
      dev.includes('vb-audio') ||
      dev.includes('blackhole') ||
      dev.includes('micyou')
    );
  });

  const isAutoSelected = computed(() => !settings.audioDevice || settings.audioDevice === 'auto');

  return {
    audioDevices,
    hasVBCable,
    hasBlackHole,
    blackholeStatus,
    pipewireStatus,
    selectedDistro,
    currentPipewireInstallCommand,
    vbcableInstalling,
    vbcableInstallProgress,
    isVirtualDeviceSelected,
    isExplicitPhysicalDevice: computed(() => !isAutoSelected.value && !isVirtualDeviceSelected.value),
    isAutoFallbackToPhysical: computed(() => isAutoSelected.value && !isVirtualDeviceSelected.value),
    fetchDevices,
    installVBCable,
    openVBCableDownload: () => openUrl('https://vb-audio.com/Cable/'),
    openBlackHoleDownload: () => openUrl('https://github.com/ExistentialAudio/BlackHole'),
  };
}
