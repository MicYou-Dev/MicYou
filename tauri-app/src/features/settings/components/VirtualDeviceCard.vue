<script setup lang="ts">
import { ref } from 'vue';
import { Check, Copy, Download, Loader2 } from '@lucide/vue';
import { isLinux, isMacOS } from '@/shared/lib/os';
import { supportedDistros, useOutputDevices } from '../composables/useOutputDevices';

// The platform's virtual microphone: BlackHole on macOS, PipeWire on Linux,
// VB-CABLE on Windows.
const {
  hasVBCable,
  hasBlackHole,
  blackholeStatus,
  pipewireStatus,
  selectedDistro,
  currentPipewireInstallCommand,
  vbcableInstalling,
  vbcableInstallProgress,
  installVBCable,
  openVBCableDownload,
  openBlackHoleDownload,
} = useOutputDevices();

const copiedPipewire = ref(false);

async function copyPipewireCommand() {
  try {
    await navigator.clipboard.writeText(currentPipewireInstallCommand.value);
    copiedPipewire.value = true;
    setTimeout(() => {
      copiedPipewire.value = false;
    }, 2000);
  } catch (e) {
    console.error('Failed to copy pipewire command:', e);
  }
}
</script>

<template>
  <div v-if="isMacOS" class="bg-surface-bright rounded-2xl p-4 space-y-4 shadow-xs">
    <div class="flex items-center justify-between">
      <h4 class="font-bold text-on-surface text-lg">BlackHole</h4>
      <span
        class="text-xs font-medium px-2 py-1 rounded-md"
        :class="hasBlackHole ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'"
      >
        {{ hasBlackHole ? $t('settings.blackhole.installed') : $t('settings.blackhole.notDetected') }}
      </span>
    </div>
    <p class="text-xs text-on-surface-variant">{{ $t('settings.blackhole.desc') }}</p>
    <div v-if="blackholeStatus.switch_audio_source" class="text-xs text-on-surface-variant flex items-center gap-1.5">
      <span class="inline-block w-1.5 h-1.5 rounded-full bg-green-400"></span>
      SwitchAudioSource {{ $t('settings.blackhole.available') }}
    </div>
    <div v-else-if="hasBlackHole" class="text-xs text-orange-400 flex items-center gap-1.5">
      <span class="inline-block w-1.5 h-1.5 rounded-full bg-orange-400"></span>
      {{ $t('settings.blackhole.switchAudioSourceMissing') }}
    </div>
    <div v-if="!hasBlackHole" class="space-y-2">
      <p class="text-xs text-on-surface-variant font-mono bg-surface-container rounded-lg p-3 select-all">
        brew install blackhole-2ch
      </p>
      <button
        @click="openBlackHoleDownload"
        class="w-full py-2 bg-surface-variant hover:bg-surface-variant/80 rounded-xl text-sm font-bold flex items-center justify-center gap-2 transition-colors"
      >
        <Download class="w-4 h-4" /> {{ $t('settings.blackhole.download') }}
      </button>
    </div>
  </div>

  <div v-else-if="isLinux" class="bg-surface-bright rounded-2xl p-4 space-y-4 shadow-xs">
    <div class="flex items-center justify-between">
      <h4 class="font-bold text-on-surface text-lg">PipeWire</h4>
      <span
        class="text-xs font-medium px-2 py-1 rounded-md"
        :class="
          pipewireStatus.available
            ? pipewireStatus.device_exists
              ? 'bg-green-500/20 text-green-400'
              : 'bg-yellow-500/20 text-yellow-400'
            : 'bg-red-500/20 text-red-400'
        "
      >
        {{
          pipewireStatus.available
            ? pipewireStatus.device_exists
              ? $t('settings.pipewire.active')
              : $t('settings.pipewire.available')
            : $t('settings.pipewire.notAvailable')
        }}
      </span>
    </div>
    <p class="text-xs text-on-surface-variant">{{ $t('settings.pipewire.desc') }}</p>
    <div v-if="!pipewireStatus.available" class="space-y-2.5">
      <div class="flex items-center gap-1.5 overflow-x-auto pb-0.5">
        <button
          v-for="distro in supportedDistros"
          :key="distro.id"
          type="button"
          @click="selectedDistro = distro.id"
          class="px-2.5 py-1 text-xs rounded-lg transition-colors font-medium cursor-pointer"
          :class="
            selectedDistro === distro.id
              ? 'bg-primary text-on-primary shadow-xs'
              : 'bg-surface-container text-on-surface-variant hover:bg-surface-variant'
          "
        >
          {{ distro.name }}
        </button>
      </div>
      <div
        class="text-xs text-on-surface-variant font-mono bg-surface-container rounded-lg p-3 select-all flex items-center justify-between gap-2"
      >
        <span class="break-all">{{ currentPipewireInstallCommand }}</span>
        <button
          type="button"
          @click="copyPipewireCommand"
          class="shrink-0 p-1.5 hover:bg-surface-variant rounded-md text-on-surface-variant transition-colors cursor-pointer"
          :title="copiedPipewire ? $t('settings.about.copied') : $t('settings.about.copyLog')"
        >
          <Check v-if="copiedPipewire" class="w-3.5 h-3.5 text-primary" />
          <Copy v-else class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  </div>

  <div v-else class="bg-surface-bright rounded-2xl p-4 space-y-4 shadow-xs">
    <div class="flex items-center justify-between">
      <h4 class="font-bold text-on-surface text-lg">{{ $t('settings.vbcable.title') }}</h4>
      <span
        class="text-xs font-medium px-2 py-1 rounded-md"
        :class="hasVBCable ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'"
      >
        {{ hasVBCable ? $t('settings.vbcable.installed') : $t('settings.vbcable.notDetected') }}
      </span>
    </div>
    <p class="text-xs text-on-surface-variant">{{ $t('settings.vbcable.desc') }}</p>
    <div v-if="!hasVBCable" class="space-y-2">
      <button
        @click="installVBCable"
        :disabled="vbcableInstalling"
        class="w-full py-2 bg-primary disabled:opacity-50 rounded-xl text-sm font-bold text-on-primary flex items-center justify-center gap-2 transition-colors"
      >
        <Loader2 v-if="vbcableInstalling" class="w-4 h-4 animate-spin" />
        <Download v-else class="w-4 h-4" />
        {{
          vbcableInstalling
            ? vbcableInstallProgress || $t('vbcableInstall.installing')
            : $t('vbcableDetect.autoInstall')
        }}
      </button>
      <button
        @click="openVBCableDownload"
        class="w-full py-2 bg-surface-variant hover:bg-surface-variant/80 rounded-xl text-sm font-bold flex items-center justify-center gap-2 transition-colors"
      >
        <Download class="w-4 h-4" /> {{ $t('settings.vbcable.download') }}
      </button>
    </div>
  </div>
</template>
