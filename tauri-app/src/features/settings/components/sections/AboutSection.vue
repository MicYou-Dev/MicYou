<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useStorage } from '@vueuse/core';
import {
  Check,
  Copy,
  Download,
  ExternalLink,
  FileText,
  FolderOpen,
  Globe,
  Heart,
  Info,
  Loader2,
  User,
  Users,
  Zap,
} from '@lucide/vue';
import { command, openUrl } from '@/platform';
import MD3Switch from '@/shared/components/ui/switch/MD3Switch.vue';
import ContributorsDialog from '../ContributorsDialog.vue';
// The bundled license texts are ~400 kB: load them in their own chunk.
const LicensesDialog = defineAsyncComponent(() => import('../LicensesDialog.vue'));
import SponsorsDialog from '../SponsorsDialog.vue';
import { sponsorsRequested } from '../../composables/useSettingsRequest';

const { t, locale } = useI18n();

const showContributors = ref(false);
const showSponsors = ref(false);
const showLicenses = ref(false);

// The macOS app menu asks for the sponsors dialog from the main window; this
// section is only mounted once it is on screen, so the request is consumed here
// (immediately covers the mount that the request itself triggered).
watch(
  sponsorsRequested,
  (wanted) => {
    if (!wanted) return;
    sponsorsRequested.value = false;
    showSponsors.value = true;
  },
  { immediate: true },
);

const appVersion = ref(__APP_VERSION__);
const checkingUpdate = ref(false);
const useMirrorDownload = useStorage('micyou_use_mirror_download', false);
const mirrorCdk = useStorage('micyou_mirror_cdk', '');
const mirrorCdkHelpUrl = computed(() => {
  const l = locale.value;
  return l.startsWith('zh') || l === 'lzh' || l === 'cat'
    ? 'https://mirrorchyan.com/zh/get-start'
    : 'https://mirrorchyan.com/en/get-start';
});

async function checkUpdate() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    const cdk = useMirrorDownload.value && mirrorCdk.value.trim() ? mirrorCdk.value.trim() : null;
    const res = await command('check_app_update', { cdk });
    if (res.hasUpdate) {
      const msg = res.isMirror
        ? t('dialogs.update.mirrorAvailable', { version: `v${res.latestVersion}` })
        : t('dialogs.update.available', { version: `v${res.latestVersion}` });
      if (confirm(msg)) {
        await openUrl(res.releaseUrl);
      }
    } else {
      alert(t('dialogs.update.latest'));
    }
  } catch (e) {
    alert(t('dialogs.update.failed', { error: String(e) || 'Unknown error' }));
  } finally {
    checkingUpdate.value = false;
  }
}

// ---- Logs ----
const logPath = ref('');
const copiedLog = ref(false);
const copiedPath = ref(false);

function flash(flag: typeof copiedLog) {
  flag.value = true;
  setTimeout(() => {
    flag.value = false;
  }, 2000);
}

async function runLogAction(action: () => Promise<void>) {
  try {
    await action();
  } catch (e) {
    alert(t('dialogs.logs.failed', { error: String(e) }));
  }
}

const openLogDir = () => runLogAction(async () => void (await command('open_log_dir')));

const copyLogContent = () =>
  runLogAction(async () => {
    await navigator.clipboard.writeText(await command('get_log_content'));
    flash(copiedLog);
  });

const copyLogPath = () =>
  runLogAction(async () => {
    if (!logPath.value) logPath.value = await command('get_log_path');
    await navigator.clipboard.writeText(logPath.value);
    flash(copiedPath);
  });

const exportLog = () =>
  runLogAction(async () => {
    await command('export_log');
    alert(t('dialogs.logs.success'));
  });

onMounted(async () => {
  try {
    logPath.value = await command('get_log_path');
  } catch (e) {
    console.error('Failed to get log path:', e);
  }
  try {
    appVersion.value = await command('get_app_version');
  } catch (e) {
    console.error('Failed to get version', e);
  }
});
</script>

<template>
  <div class="space-y-4 pb-12">
    <div class="bg-surface-bright rounded-2xl overflow-hidden shadow-xs flex flex-col border border-border">
      <div class="flex items-center gap-4 p-4 hover:bg-surface-variant transition-colors cursor-default">
        <User class="w-6 h-6 text-on-surface-variant shrink-0" />
        <div class="flex-1">
          <h4 class="text-sm font-medium text-on-surface">Developer</h4>
          <p class="text-xs text-on-surface-variant">LanRhyme、ChinsaaWei、ChouChiu</p>
        </div>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <a
        href="https://github.com/MicYou-Dev/MicYou"
        target="_blank"
        class="flex items-center gap-4 p-4 hover:bg-surface-variant transition-colors cursor-pointer group"
      >
        <Globe class="w-6 h-6 text-on-surface-variant shrink-0" />
        <div class="flex-1">
          <h4 class="text-sm font-medium text-on-surface">GitHub Repository</h4>
          <p class="text-xs text-primary group-hover:underline">https://github.com/MicYou-Dev/MicYou</p>
        </div>
      </a>
      <div class="h-px bg-border mx-4"></div>

      <div
        @click="showContributors = true"
        class="flex items-center gap-4 p-4 hover:bg-surface-variant transition-colors cursor-pointer"
      >
        <Users class="w-6 h-6 text-on-surface-variant shrink-0" />
        <div class="flex-1">
          <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.about.contributorsBtn') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.about.contributorsDesc') }}</p>
        </div>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <div
        @click="showSponsors = true"
        class="flex items-center gap-4 p-4 hover:bg-surface-variant transition-colors cursor-pointer"
      >
        <Heart class="w-6 h-6 text-on-surface-variant shrink-0" />
        <div class="flex-1">
          <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.about.sponsorsBtn') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.about.sponsorsDesc') }}</p>
        </div>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <div class="flex items-center justify-between p-4 hover:bg-surface-variant transition-colors cursor-default">
        <div class="flex items-center gap-4">
          <Info class="w-6 h-6 text-on-surface-variant shrink-0" />
          <div>
            <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.about.version') }}</h4>
            <p class="text-xs text-on-surface-variant">{{ appVersion }}</p>
          </div>
        </div>
        <button
          @click="checkUpdate"
          :disabled="checkingUpdate"
          class="px-3 py-1.5 text-xs font-medium text-on-primary bg-primary hover:opacity-90 rounded-full transition-opacity disabled:opacity-50 flex items-center gap-1.5"
        >
          <Loader2 v-if="checkingUpdate" class="w-3.5 h-3.5 animate-spin" />
          {{ checkingUpdate ? $t('dialogs.update.checking') : $t('settings.about.updatesBtn') }}
        </button>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <!-- MirrorChyan High-speed Download -->
      <div class="p-4 flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-4">
            <Zap
              class="w-6 h-6 shrink-0 transition-colors"
              :class="useMirrorDownload ? 'text-primary' : 'text-on-surface-variant'"
            />
            <div>
              <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.mirrorDownload.title') }}</h4>
              <p class="text-xs text-on-surface-variant">{{ $t('settings.mirrorDownload.desc') }}</p>
            </div>
          </div>
          <MD3Switch v-model="useMirrorDownload" />
        </div>

        <div v-if="useMirrorDownload" class="pl-10 space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-xs text-on-surface-variant">{{ $t('settings.mirrorDownload.cdkLabel') }}</span>
            <a
              :href="mirrorCdkHelpUrl"
              target="_blank"
              class="text-xs text-primary hover:underline flex items-center gap-1 cursor-pointer"
            >
              <span>{{ $t('settings.mirrorDownload.getCdk') }}</span>
              <ExternalLink class="w-3 h-3" />
            </a>
          </div>
          <input
            v-model="mirrorCdk"
            type="text"
            :placeholder="$t('settings.mirrorDownload.cdkPlaceholder')"
            class="w-full bg-surface-container border border-border/40 rounded-xl px-3 py-2 text-xs text-on-surface placeholder:text-on-surface-variant/50 focus:outline-hidden focus:ring-2 focus:ring-primary/40 font-mono"
          />
        </div>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <div
        @click="showLicenses = true"
        class="flex items-center gap-4 p-4 hover:bg-surface-variant transition-colors cursor-pointer"
      >
        <FileText class="w-6 h-6 text-on-surface-variant shrink-0" />
        <div class="flex-1">
          <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.about.licensesBtn') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.about.licensesDesc') }}</p>
        </div>
      </div>
      <div class="h-px bg-border mx-4"></div>

      <div class="p-4 flex flex-col gap-3">
        <div class="flex items-center gap-4">
          <FileText class="w-6 h-6 text-on-surface-variant shrink-0" />
          <div class="flex-1 min-w-0">
            <h4 class="text-sm font-medium text-on-surface">{{ $t('settings.about.logsBtn') }}</h4>
            <p class="text-xs text-on-surface-variant truncate font-mono select-all" :title="logPath">
              {{ logPath || $t('settings.about.logsDesc') }}
            </p>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-2 pt-1">
          <button @click="openLogDir" class="log-action">
            <FolderOpen class="w-3.5 h-3.5" />
            {{ $t('settings.about.openLogDir') }}
          </button>
          <button @click="copyLogContent" class="log-action">
            <Check v-if="copiedLog" class="w-3.5 h-3.5 text-primary" />
            <Copy v-else class="w-3.5 h-3.5" />
            {{ copiedLog ? $t('settings.about.copied') : $t('settings.about.copyLog') }}
          </button>
          <button @click="copyLogPath" class="log-action">
            <Check v-if="copiedPath" class="w-3.5 h-3.5 text-primary" />
            <Copy v-else class="w-3.5 h-3.5" />
            {{ copiedPath ? $t('settings.about.copied') : $t('settings.about.copyLogPath') }}
          </button>
          <button @click="exportLog" class="log-action">
            <Download class="w-3.5 h-3.5" />
            {{ $t('settings.about.exportLog') }}
          </button>
        </div>
      </div>
    </div>

    <div class="bg-secondary-container/50 rounded-2xl p-6">
      <h3 class="text-base font-bold text-on-secondary-container mb-2">{{ $t('settings.about.introTitle') }}</h3>
      <p class="text-sm text-on-secondary-container/80 leading-relaxed">{{ $t('settings.about.introText') }}</p>
    </div>

    <Teleport to="body">
      <ContributorsDialog :isOpen="showContributors" @close="showContributors = false" />
      <SponsorsDialog :isOpen="showSponsors" @close="showSponsors = false" />
      <LicensesDialog :isOpen="showLicenses" @close="showLicenses = false" />
    </Teleport>
  </div>
</template>

<style scoped>
@reference "@/shared/assets/index.css";

.log-action {
  @apply px-3 py-1.5 text-xs font-medium bg-surface-variant hover:bg-surface-variant/80 text-on-surface rounded-lg transition-colors flex items-center gap-1.5;
}
</style>
