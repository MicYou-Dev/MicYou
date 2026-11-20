<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { CheckCircle2, Download, Loader2, Mic, RefreshCw } from '@lucide/vue';
import { command, openUrl } from '@/platform';
import { isWindows } from '@/shared/lib/os';
import notesData from '../../../../../release-notes.json';

type ReleaseNotes = {
  title: string;
  summary: string;
  fixTitle: string;
  fixDesc: string;
  steamTitle: string;
  steamDesc: string;
  migrationTitle: string;
  checking: string;
  vbDetected: string;
  noVbDetected: string;
  switchSteam: string;
  keepVb: string;
  keepVbDone: string;
  installing: string;
  steamReady: string;
  rebootHint: string;
  uninstallVbHint: string;
  openApps: string;
  restartHint: string;
  installFailed: string;
  otherPlatforms: string;
};

const emit = defineEmits<{
  'set-output-device': [device: string];
}>();

const { locale } = useI18n();

const notesByLocale = notesData as unknown as Record<string, ReleaseNotes>;
const notes = computed(() => notesByLocale[locale.value] ?? notesByLocale.en);
const version = __APP_VERSION__;

const checking = ref(true);
const installing = ref(false);
const steamInstalled = ref(false);
const vbcableInstalled = ref(false);
const switched = ref(false);
const keptVb = ref(false);
const rebootRequired = ref(false);
const error = ref('');
const devices = ref<string[]>([]);

async function loadState() {
  if (!isWindows) {
    checking.value = false;
    return;
  }

  try {
    const [steam, vbcable, list] = await Promise.all([
      command('check_steam_audio').catch(() => false),
      command('check_vbcable').catch(() => false),
      command('get_audio_devices').catch(() => [] as string[]),
    ]);
    steamInstalled.value = steam;
    vbcableInstalled.value = vbcable;
    devices.value = list;
  } catch (e) {
    console.error('Failed to inspect the virtual audio devices:', e);
  } finally {
    checking.value = false;
  }
}

onMounted(() => {
  void loadState();
});

function findDevice(pattern: RegExp) {
  return devices.value.find((device) => pattern.test(device));
}

function keepVbCable() {
  const device = findDevice(/cable.*input|vb-audio.*cable/i);
  if (!device) {
    error.value = 'unknown';
    return;
  }
  emit('set-output-device', device);
  keptVb.value = true;
}

async function switchToSteam() {
  installing.value = true;
  error.value = '';
  keptVb.value = false;

  try {
    if (!steamInstalled.value) {
      const result = await command('install_steam_audio');
      if (!result.success) {
        error.value = result.error_type || 'unknown';
        return;
      }
      steamInstalled.value = await command('check_steam_audio').catch(() => false);
      if (!steamInstalled.value) {
        // pnputil staged the driver but Windows needs a reboot before the
        // endpoints exist. Tell the user instead of claiming it is ready.
        rebootRequired.value = true;
      }
    }

    if (rebootRequired.value) {
      // "auto" prefers the Steam endpoint once it appears, while still
      // falling back to the current device until the reboot.
      emit('set-output-device', 'auto');
    } else {
      if (devices.value.length === 0) {
        devices.value = await command('get_audio_devices').catch(() => []);
      }
      const device = findDevice(/steam streaming microphone/i);
      emit('set-output-device', device ?? 'auto');
    }
    switched.value = true;
  } catch (e) {
    console.error('Switching to the Steam Streaming Microphone failed:', e);
    error.value = 'unknown';
  } finally {
    installing.value = false;
  }
}

function openAppsFeatures() {
  void openUrl('ms-settings:appsfeatures').catch((e) =>
    console.error('Opening the installed-apps page failed:', e),
  );
}
</script>

<template>
  <aside
    class="shrink-0 border-t md:border-t-0 md:border-l border-outline-variant/20 p-5 space-y-4 overflow-y-auto max-h-[45vh] md:max-h-[90vh] md:w-[20rem]"
  >
    <div class="space-y-1">
      <p class="text-xs font-medium uppercase tracking-wide text-primary">{{ notes.title }}</p>
      <h2 class="text-lg font-bold text-on-surface">MicYou {{ version }}</h2>
    </div>

    <p class="text-xs text-on-surface-variant">{{ notes.summary }}</p>

      <div class="space-y-2">
        <div class="flex items-start gap-2 bg-surface-bright rounded-xl p-3">
          <RefreshCw class="w-4 h-4 text-primary shrink-0 mt-0.5" />
          <div>
            <p class="text-xs font-bold text-on-surface">{{ notes.fixTitle }}</p>
            <p class="text-[11px] text-on-surface-variant">{{ notes.fixDesc }}</p>
          </div>
        </div>
        <div class="flex items-start gap-2 bg-surface-bright rounded-xl p-3">
          <Mic class="w-4 h-4 text-primary shrink-0 mt-0.5" />
          <div>
            <p class="text-xs font-bold text-on-surface">{{ notes.steamTitle }}</p>
            <p class="text-[11px] text-on-surface-variant">{{ notes.steamDesc }}</p>
          </div>
        </div>
      </div>

      <div v-if="isWindows" class="border border-outline-variant/30 rounded-2xl p-3 space-y-2">
        <h3 class="text-xs font-bold text-on-surface">{{ notes.migrationTitle }}</h3>

        <div v-if="checking" class="flex items-center gap-2 text-xs text-on-surface-variant">
          <Loader2 class="w-3.5 h-3.5 animate-spin" />
          {{ notes.checking }}
        </div>

        <template v-else-if="keptVb">
          <div class="flex items-start gap-2 text-xs text-green-400">
            <CheckCircle2 class="w-4 h-4 shrink-0" />
            <span>{{ notes.keepVbDone }}</span>
          </div>
          <button
            @click="switchToSteam"
            :disabled="installing"
            class="w-full py-2 bg-surface-variant hover:bg-surface-variant/80 disabled:opacity-50 rounded-lg text-xs font-bold transition-colors"
          >
            {{ notes.switchSteam }}
          </button>
        </template>

        <template v-else-if="switched">
          <div class="flex items-start gap-2 text-xs text-green-400">
            <CheckCircle2 class="w-4 h-4 shrink-0" />
            <span>{{ rebootRequired ? notes.rebootHint : notes.steamReady }}</span>
          </div>
          <p v-if="vbcableInstalled && !rebootRequired" class="text-[11px] text-on-surface-variant">
            {{ notes.uninstallVbHint }}
          </p>
          <button
            v-if="vbcableInstalled && !rebootRequired"
            @click="openAppsFeatures"
            class="w-full py-2 bg-surface-variant hover:bg-surface-variant/80 rounded-lg text-xs font-bold transition-colors"
          >
            {{ notes.openApps }}
          </button>
          <p v-if="!rebootRequired" class="text-[11px] text-on-surface-variant">
            {{ notes.restartHint }}
          </p>
        </template>

        <template v-else>
          <p class="text-[11px] text-on-surface-variant">
            {{ vbcableInstalled ? notes.vbDetected : notes.noVbDetected }}
          </p>

          <div v-if="error" class="bg-error/10 rounded-lg p-2">
            <p class="text-[11px] text-error">{{ notes.installFailed }}</p>
          </div>

          <div class="space-y-2">
            <button
              @click="switchToSteam"
              :disabled="installing"
              class="w-full py-2 bg-primary hover:bg-primary/90 disabled:opacity-50 rounded-lg text-xs font-bold text-on-primary flex items-center justify-center gap-2 transition-colors"
            >
              <Loader2 v-if="installing" class="w-3.5 h-3.5 animate-spin" />
              <Download v-else class="w-3.5 h-3.5" />
              {{ installing ? notes.installing : notes.switchSteam }}
            </button>
            <button
              v-if="vbcableInstalled"
              @click="keepVbCable"
              :disabled="installing"
              class="w-full py-2 text-xs text-on-surface-variant hover:text-on-surface disabled:opacity-50 transition-colors"
            >
              {{ notes.keepVb }}
            </button>
          </div>
        </template>
      </div>

    <p v-else class="text-xs text-on-surface-variant">{{ notes.otherPlatforms }}</p>
  </aside>
</template>
