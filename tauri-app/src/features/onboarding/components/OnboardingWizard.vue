<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { X, CheckCircle2, Download, Loader2, ArrowRight } from '@lucide/vue';
import { command, onEvent, openUrl } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';
import { isWindows } from '@/shared/lib/os';

const { t } = useI18n();

const emit = defineEmits<{ complete: [] }>();

defineProps<{
  visible: boolean;
}>();

const step = ref(1);
const virtualAudioInstalled = ref(false);
const installing = ref(false);
const installProgress = ref('');
const installError = ref('');

const track = useListeners();

const TOTAL_STEPS_WINDOWS = 4;
const TOTAL_STEPS_OTHER = 2;

const totalSteps = isWindows ? TOTAL_STEPS_WINDOWS : TOTAL_STEPS_OTHER;

onMounted(async () => {
  if (isWindows) {
    try {
      virtualAudioInstalled.value = await command('check_steam_audio');
      if (virtualAudioInstalled.value) {
        step.value = 4;
      }
    } catch (e) {
      console.error('Failed to check Steam Streaming Microphone:', e);
    }
  }

  await track(onEvent('virtual-audio-install-progress', (payload) => {
    installProgress.value = payload;
  }));
});

function skip() {
  if (isWindows && step.value === 1) {
    step.value = 4;
  } else {
    complete();
  }
}

function complete() {
  localStorage.setItem('micyou_onboarding_completed', 'true');
  emit('complete');
}

async function installVirtualAudio() {
  installing.value = true;
  installError.value = '';
  installProgress.value = '';

  try {
    const result = await command('install_steam_audio');

    if (result.success) {
      virtualAudioInstalled.value = true;
    } else {
      installError.value = result.error_type || 'unknown';
    }
  } catch (e: any) {
    installError.value = 'unknown';
    installProgress.value = '';
  } finally {
    installing.value = false;
  }
}

async function openManualDownload() {
  await openUrl('https://github.com/milo-src/SteamLinkAudioDriver');
}

async function openVideoGuide() {
  await openUrl('https://www.bilibili.com/video/BV1MpNKz8ELw');
}

async function openTextGuide() {
  await openUrl('https://github.com/MicYou-Dev/MicYou/blob/master/docs/FAQ.md');
}
</script>

<template>
  <div v-if="visible" class="fixed inset-0 z-100 flex items-center justify-center bg-black/60 backdrop-blur-xs">
    <div class="bg-surface rounded-3xl shadow-2xl w-full max-w-lg mx-4 overflow-hidden">
      <!-- Header -->
      <div class="flex items-center justify-between p-5 border-b border-outline-variant/20">
        <div class="flex items-center gap-2">
          <span class="text-sm text-on-surface-variant">{{ step }} / {{ totalSteps }}</span>
        </div>
        <button @click="skip" class="text-on-surface-variant hover:text-on-surface transition-colors">
          <X class="w-5 h-5" />
        </button>
      </div>

      <!-- Step 1: Welcome -->
      <div v-if="step === 1" class="p-6 space-y-5 max-h-[60vh] overflow-y-auto">
        <h2 class="text-xl font-bold text-on-surface">{{ t('onboarding.title') }}</h2>
        <p class="text-sm text-on-surface-variant">{{ t('onboarding.desc') }}</p>

        <h3 class="text-base font-bold text-on-surface">{{ t('onboarding.quickStart') }}</h3>

        <div class="space-y-3">
          <div class="bg-surface-bright rounded-xl p-3">
            <p class="text-sm font-bold text-on-surface">{{ t('onboarding.step1Title') }}</p>
            <p class="text-xs text-on-surface-variant">{{ t('onboarding.step1Desc') }}</p>
          </div>
          <div class="bg-surface-bright rounded-xl p-3">
            <p class="text-sm font-bold text-on-surface">{{ t('onboarding.step2Title') }}</p>
            <p class="text-xs text-on-surface-variant">{{ t('onboarding.step2WifiDesc') }}</p>
            <p class="text-xs text-on-surface-variant">{{ t('onboarding.step2UsbDesc') }}</p>
          </div>
          <div class="bg-surface-bright rounded-xl p-3">
            <p class="text-sm font-bold text-on-surface">{{ t('onboarding.step3Title') }}</p>
            <p class="text-xs text-on-surface-variant">{{ t('onboarding.step3Desc') }}</p>
          </div>
          <div class="bg-surface-bright rounded-xl p-3">
            <p class="text-sm font-bold text-on-surface">{{ t('onboarding.step4Title') }}</p>
            <p class="text-xs text-on-surface-variant">{{ t('onboarding.step4Desc') }}</p>
          </div>
        </div>

        <div class="flex items-center gap-3 text-sm">
          <button @click="openVideoGuide" class="text-primary hover:underline">{{ t('onboarding.videoGuide') }}</button>
          <span class="text-outline">|</span>
          <button @click="openTextGuide" class="text-primary hover:underline">{{ t('onboarding.textGuide') }}</button>
        </div>
      </div>

      <!-- Step 2: Steam Streaming Microphone detection (Windows only) -->
      <div v-else-if="step === 2" class="p-6 space-y-5">
        <h2 class="text-xl font-bold text-on-surface">{{ t('vbcableDetect.title') }}</h2>

        <div v-if="virtualAudioInstalled" class="flex items-center gap-3 bg-green-500/10 rounded-xl p-4">
          <CheckCircle2 class="w-6 h-6 text-green-400 shrink-0" />
          <p class="text-sm text-green-400">{{ t('vbcableDetect.installed') }}</p>
        </div>

        <template v-else>
          <p class="text-sm text-on-surface-variant">{{ t('vbcableDetect.message') }}</p>

          <div v-if="installError" class="bg-error/10 rounded-xl p-4">
            <p class="text-sm text-error" v-if="installError === 'uac_denied'">{{ t('vbcableInstall.adminRequired') }}</p>
            <p class="text-sm text-error" v-else-if="installError === 'timeout'">{{ t('vbcableInstall.timeout') }}</p>
            <p class="text-sm text-error" v-else>{{ t('vbcableInstall.failed') }}</p>
          </div>

          <div class="space-y-2">
            <button
              @click="installVirtualAudio"
              :disabled="installing"
              class="w-full py-3 bg-primary hover:bg-primary/90 disabled:opacity-50 rounded-xl text-sm font-bold text-on-primary flex items-center justify-center gap-2 transition-colors"
            >
              <Loader2 v-if="installing" class="w-4 h-4 animate-spin" />
              <Download v-else class="w-4 h-4" />
              {{ installing ? installProgress || t('vbcableInstall.installing') : t('vbcableDetect.autoInstall') }}
            </button>

            <button
              @click="openManualDownload"
              class="w-full py-3 bg-surface-variant hover:bg-surface-variant/80 rounded-xl text-sm font-bold flex items-center justify-center gap-2 transition-colors"
            >
              <Download class="w-4 h-4" /> {{ t('vbcableDetect.manualDownload') }}
            </button>

            <button
              @click="skip"
              class="w-full py-2 text-sm text-on-surface-variant hover:text-on-surface transition-colors"
            >
              {{ t('vbcableDetect.skipForNow') }}
            </button>
          </div>
        </template>
      </div>

      <!-- Step 3 is the install in progress state shown inside step 2; the
           wizard only advances to the completion page once the driver exists. -->

      <!-- Step 4: Complete -->
      <div v-else-if="step === 4" class="p-6 space-y-5">
        <div class="flex items-center gap-3">
          <CheckCircle2 class="w-8 h-8 text-green-400" />
          <h2 class="text-xl font-bold text-on-surface">{{ t('onboarding.complete') }}</h2>
        </div>
        <p class="text-sm text-on-surface-variant">{{ t('onboarding.completeDesc') }}</p>

        <div v-if="isWindows && virtualAudioInstalled" class="bg-green-500/10 rounded-xl p-3">
          <p class="text-sm text-green-400">{{ t('vbcableDetect.installed') }}</p>
        </div>
      </div>

      <!-- Footer -->
      <div class="p-5 border-t border-outline-variant/20 flex items-center justify-between">
        <button
          v-if="step === 1"
          @click="skip"
          class="px-4 py-2 text-sm text-on-surface-variant hover:text-on-surface transition-colors"
        >
          {{ t('onboarding.gotIt') }}
        </button>
        <div v-else></div>

        <div class="flex items-center gap-2">
          <button
            v-if="step === 2 && virtualAudioInstalled"
            @click="step = 4"
            class="px-6 py-2 bg-primary hover:bg-primary/90 rounded-xl text-sm font-bold text-on-primary flex items-center gap-1 transition-colors"
          >
            {{ t('onboarding.next') }} <ArrowRight class="w-4 h-4" />
          </button>
          <button
            v-if="step === 4"
            @click="complete"
            class="px-6 py-2 bg-primary hover:bg-primary/90 rounded-xl text-sm font-bold text-on-primary transition-colors"
          >
            {{ t('onboarding.startUsing') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
