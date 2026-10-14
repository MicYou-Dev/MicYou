<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { ArrowRight, Ban, ChevronRight } from '@lucide/vue';
import { command, onEvent, type UnlistenFn } from '@/platform';
import MD3Slider from '@/shared/components/ui/slider/MD3Slider.vue';
import AudioChainDialog from '@/features/audio/components/AudioChainDialog.vue';
import { chainStageLabel } from '@/features/audio/chain';
import { usePlugins } from '@/features/plugins/composables/usePlugins';
import { useAudioSettings } from '../../composables/useAudioSettings';
import { useOutputDevices } from '../../composables/useOutputDevices';
import DspCard from '../DspCard.vue';

const { t, locale } = useI18n();
const { settings, isAecSupported, aecRuntimeAvailable, aecReason, loadSettings, syncSettingsToBackend } =
  useAudioSettings();
const { fetchDevices } = useOutputDevices();
const { plugins } = usePlugins();

const showAudioChain = ref(false);
const nsTypes = [
  { id: 'PureVox', label: 'PureVox (AI)' },
  { id: 'RNNoise', label: 'RNNoise' },
  { id: 'Speexdsp', label: 'Speexdsp' },
];

const displayChain = computed(() =>
  isAecSupported.value
    ? settings.processingChain
    : settings.processingChain.filter((i) => i !== 'AEC'),
);

// Plugin:<id> stages show the plugin name rather than a translation key (#347)
const chainItemLabel = (item: string) => chainStageLabel(item, t, plugins.value, locale.value);

// ---- Spectrum monitoring: streams while this section is open ----
const spectrumCanvas = ref<HTMLCanvasElement | null>(null);
const rawSpectrum = ref<number[]>(new Array(64).fill(0));
const processedSpectrum = ref<number[]>(new Array(64).fill(0));
let unlistenSpectrum: UnlistenFn | null = null;
let animationFrameId: number | null = null;
let mounted = false;

function setSpectrumStreaming(enabled: boolean) {
  void command('set_spectrum_streaming', { enabled }).catch((e) =>
    console.error('Failed to set spectrum streaming:', e),
  );
}

function scheduleFrame() {
  if (mounted && animationFrameId === null) {
    animationFrameId = requestAnimationFrame(drawSpectrum);
  }
}

function drawSpectrum() {
  animationFrameId = null;
  if (!mounted) return;
  const canvas = spectrumCanvas.value;
  const ctx = canvas?.getContext('2d');
  if (!canvas || !ctx) {
    scheduleFrame();
    return;
  }

  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  const targetWidth = Math.max(1, Math.round(rect.width * dpr));
  const targetHeight = Math.max(1, Math.round(rect.height * dpr));
  if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
    canvas.width = targetWidth;
    canvas.height = targetHeight;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  const { width, height } = rect;
  ctx.clearRect(0, 0, width, height);

  const raw = rawSpectrum.value;
  const proc = processedSpectrum.value;
  const gap = 2;
  const barWidth = width / raw.length;
  const effectiveBarWidth = barWidth - gap;

  const style = getComputedStyle(document.documentElement);
  const primaryColor = `hsl(${style.getPropertyValue('--primary').trim()})`;
  const variantColor = `hsl(${style.getPropertyValue('--surface-variant').trim()})`;

  for (let i = 0; i < raw.length; i++) {
    const rawH = (raw[i] || 0) * height;
    const procH = (proc[i] || 0) * height;
    if (rawH > 0.5) {
      ctx.fillStyle = variantColor;
      ctx.beginPath();
      ctx.roundRect(i * barWidth + gap / 2, height - rawH, effectiveBarWidth, rawH, 2);
      ctx.fill();
    }
    if (procH > 0.5) {
      ctx.fillStyle = primaryColor;
      ctx.beginPath();
      ctx.roundRect(i * barWidth + gap / 2, height - procH, effectiveBarWidth, procH, 2);
      ctx.fill();
    }
  }

  scheduleFrame();
}

async function startMonitoring() {
  // Refresh devices and settings so changes made elsewhere (e.g. the CLI) show up.
  await fetchDevices();
  await loadSettings();
  await syncSettingsToBackend();
  const unlisten = await onEvent('audio-spectrum', (payload) => {
    rawSpectrum.value = payload.raw;
    processedSpectrum.value = payload.processed;
  });
  if (!mounted) {
    unlisten();
    return;
  }
  unlistenSpectrum = unlisten;
  setSpectrumStreaming(true);
  scheduleFrame();
}

onMounted(() => {
  mounted = true;
  void startMonitoring().catch((e) => console.error('Failed to start audio monitoring:', e));
});

onUnmounted(() => {
  mounted = false;
  unlistenSpectrum?.();
  if (animationFrameId !== null) cancelAnimationFrame(animationFrameId);
  setSpectrumStreaming(false);
});
</script>

<template>
  <div class="space-y-6">
    <!-- Spectrum Analyzer / Real-time Monitoring -->
    <div class="haze-surface p-4 space-y-3">
      <div class="flex justify-between items-center">
        <h4 class="font-bold text-on-surface text-sm">{{ $t('settings.spectrum.title') }}</h4>
        <div class="flex gap-4">
          <div class="flex items-center gap-2">
            <div class="w-3 h-3 rounded-sm bg-surface-variant"></div>
            <span class="text-[10px] text-on-surface-variant">{{ $t('settings.spectrum.raw') }}</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="w-3 h-3 rounded-sm bg-primary"></div>
            <span class="text-[10px] text-on-surface-variant">{{ $t('settings.spectrum.processed') }}</span>
          </div>
        </div>
      </div>
      <div class="w-full h-32 bg-surface-container rounded-xl overflow-hidden relative">
        <canvas ref="spectrumCanvas" class="w-full h-full"></canvas>
      </div>
    </div>

    <!-- Amplifier (Gain) -->
    <div class="bg-surface-bright rounded-2xl p-4 shadow-xs flex items-center gap-4">
      <span class="text-sm font-medium text-on-surface whitespace-nowrap">{{ $t('settings.audioParams.gain') }}</span>
      <MD3Slider :min="-50" :max="50" v-model="settings.gain" />
      <span class="text-xs w-12 text-right">{{ settings.gain > 0 ? '+' : '' }}{{ settings.gain }} dB</span>
    </div>

    <!-- Acoustic Echo Cancellation (AEC) -->
    <DspCard
      v-model="settings.aecEnabled"
      :title="$t('settings.audioParams.aec')"
      :desc="$t('settings.audioParams.aecDesc')"
      :disabled="!isAecSupported || !aecRuntimeAvailable"
    >
      <template #hint>
        <p
          v-if="aecReason === 'permission_denied'"
          class="text-xs text-on-surface-variant mt-1 flex items-center gap-1"
        >
          <Ban class="w-3 h-3 shrink-0" />
          {{ $t('settings.audioParams.aecPermissionDenied') }}
        </p>
        <p v-else-if="!isAecSupported" class="text-xs text-on-surface-variant mt-1 flex items-center gap-1">
          <Ban class="w-3 h-3 shrink-0" />
          {{ $t('settings.audioParams.aecUnavailable') }}
        </p>
        <p v-else-if="!aecRuntimeAvailable" class="text-xs text-on-surface-variant mt-1 flex items-center gap-1">
          <Ban class="w-3 h-3 shrink-0" />
          {{ $t('settings.audioParams.aecRuntimeUnavailable') }}
        </p>
      </template>
    </DspCard>

    <!-- Noise Suppression -->
    <DspCard v-model="settings.nsEnabled" :title="$t('settings.audioParams.noiseSuppression')">
      <div class="space-y-4">
        <div class="flex gap-2">
          <button
            v-for="type in nsTypes"
            :key="type.id"
            @click="settings.nsType = type.id"
            class="px-3 py-1 rounded-full text-xs font-medium transition-colors"
            :class="settings.nsType === type.id ? 'bg-primary text-on-primary' : 'bg-surface-container text-on-surface'"
          >
            {{ type.label }}
          </button>
        </div>
        <div class="flex items-center gap-4">
          <span class="text-xs text-on-surface-variant whitespace-nowrap">{{ $t('settings.audioParams.intensity') }}</span>
          <MD3Slider :min="0" :max="100" v-model="settings.nsIntensity" />
          <span class="text-xs w-8 text-right">{{ settings.nsIntensity }}%</span>
        </div>
      </div>
    </DspCard>

    <!-- Dereverb -->
    <DspCard v-model="settings.dereverbEnabled" :title="$t('settings.audioParams.dereverb')">
      <div class="flex items-center gap-4">
        <span class="text-xs text-on-surface-variant whitespace-nowrap">{{ $t('settings.audioParams.level') }}</span>
        <MD3Slider :min="0" :max="100" v-model="settings.dereverbLevel" />
        <span class="text-xs w-8 text-right">{{ settings.dereverbLevel }}%</span>
      </div>
    </DspCard>

    <!-- Auto Gain Control -->
    <DspCard v-model="settings.agcEnabled" :title="$t('settings.audioParams.agc')">
      <div class="space-y-4">
        <div class="flex items-center gap-4">
          <span class="text-xs text-on-surface-variant w-20">{{ $t('settings.audioParams.target') }}</span>
          <MD3Slider :min="0" :max="32767" v-model="settings.agcTarget" />
          <span class="text-xs w-10 text-right">{{ settings.agcTarget }}</span>
        </div>
        <div class="flex items-center gap-4">
          <span class="text-xs text-on-surface-variant w-20">{{ $t('settings.audioParams.attack') }}</span>
          <MD3Slider :min="1" :max="100" v-model="settings.agcAttack" />
          <span class="text-xs w-10 text-right">{{ (settings.agcAttack / 1000).toFixed(3) }}</span>
        </div>
        <div class="flex items-center gap-4">
          <span class="text-xs text-on-surface-variant w-20">{{ $t('settings.audioParams.decay') }}</span>
          <MD3Slider :min="1" :max="100" v-model="settings.agcDecay" />
          <span class="text-xs w-10 text-right">{{ (settings.agcDecay / 10000).toFixed(4) }}</span>
        </div>
      </div>
    </DspCard>

    <!-- Voice Activity Detection -->
    <DspCard v-model="settings.vadEnabled" :title="$t('settings.audioParams.vad')">
      <div class="flex items-center gap-4">
        <span class="text-xs text-on-surface-variant whitespace-nowrap">{{ $t('settings.audioParams.threshold') }}</span>
        <MD3Slider :min="-100" :max="0" v-model="settings.vadThreshold" />
        <span class="text-xs w-12 text-right">{{ settings.vadThreshold }} dB</span>
      </div>
    </DspCard>

    <!-- Audio Processing Chain -->
    <div
      @click="showAudioChain = true"
      class="bg-surface-bright rounded-2xl p-4 shadow-xs space-y-3 cursor-pointer hover:bg-surface-variant transition-colors group"
    >
      <div class="flex items-center justify-between">
        <div>
          <h4 class="font-bold text-on-surface">{{ $t('settings.audioChain.title') }}</h4>
          <p class="text-xs text-on-surface-variant mt-0.5">{{ $t('settings.audioChain.descPopup') }}</p>
        </div>
        <div
          class="w-8 h-8 rounded-full bg-surface-container flex items-center justify-center group-hover:bg-primary group-hover:text-on-primary transition-colors"
        >
          <ChevronRight class="w-4 h-4 text-on-surface-variant group-hover:text-on-primary transition-colors" />
        </div>
      </div>
      <div class="flex items-center gap-2 overflow-hidden text-xs text-on-surface-variant font-medium opacity-80 pt-1">
        <template v-for="(item, index) in displayChain" :key="item">
          <span class="whitespace-nowrap">{{ chainItemLabel(item) }}</span>
          <ArrowRight v-if="index < displayChain.length - 1" class="w-3 h-3 shrink-0" />
        </template>
      </div>
    </div>

    <!-- Output Buffer Size -->
    <div class="bg-surface-bright rounded-2xl p-4 shadow-xs">
      <div class="flex items-center justify-between">
        <div>
          <span class="font-medium text-on-surface">{{ $t('settings.audioParams.bufferSize') }}</span>
          <p class="text-xs text-on-surface-variant mt-0.5">{{ $t('settings.audioParams.bufferSizeDesc') }}</p>
        </div>
        <span class="text-xs w-14 text-right text-on-surface-variant font-medium whitespace-nowrap">
          {{ settings.outputBufferMs }} ms
        </span>
      </div>
      <div class="flex items-center gap-3">
        <span class="text-[10px] text-on-surface-variant shrink-0">{{ $t('settings.audioParams.bufferLow') }}</span>
        <MD3Slider :min="100" :max="1200" :step="100" v-model="settings.outputBufferMs" />
        <span class="text-[10px] text-on-surface-variant shrink-0">{{ $t('settings.audioParams.bufferHigh') }}</span>
      </div>
    </div>

    <Teleport to="body">
      <AudioChainDialog
        :isOpen="showAudioChain"
        :chain="settings.processingChain"
        @update:chain="settings.processingChain = $event"
        @close="showAudioChain = false"
      />
    </Teleport>
  </div>
</template>
