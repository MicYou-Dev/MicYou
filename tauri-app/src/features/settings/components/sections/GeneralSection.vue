<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { AlertTriangle, CheckCircle2 } from '@lucide/vue';
import { command, type ModeStatus } from '@/platform';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/shared/components/ui/select';
import MD3Switch from '@/shared/components/ui/switch/MD3Switch.vue';
import { muteSyncEnabled } from '@/features/connection/composables/useMuteSync';
import { useAppPrefs } from '../../composables/useAppPrefs';
import { useAudioSettings } from '../../composables/useAudioSettings';
import { useLanguageSetting } from '../../composables/useLanguageSetting';
import { useOutputDevices } from '../../composables/useOutputDevices';
import { useRestoreDefaults } from '../../composables/useRestoreDefaults';
import SettingRow from '../SettingRow.vue';
import VirtualDeviceCard from '../VirtualDeviceCard.vue';

const { t } = useI18n();
const { currentLanguage } = useLanguageSetting();
const {
  closeBehavior,
  startMinimized,
  notificationsEnabled,
  autoStream,
  pocketMode,
  autostartEnabled,
  refreshAutostart,
  setAutostart,
} = useAppPrefs();
const { settings } = useAudioSettings();
const {
  audioDevices,
  isVirtualDeviceSelected,
  isExplicitPhysicalDevice,
  isAutoFallbackToPhysical,
  fetchDevices,
} = useOutputDevices();
const restore = useRestoreDefaults();

const languages = [
  ['zh', '简体中文'],
  ['en', 'English'],
  ['cat', '喵喵语 (´,,•ω•,,)'],
  ['zh-hk', '粤语'],
  ['zh-tw', '繁體中文（台灣）'],
  ['zh-ss', '中国人（坚硬）'],
  ['lzh', '文言'],
] as const;

// ---- Run mode (GUI / CLI / TUI) ----
const modeStatus = ref<ModeStatus>({ mode: 'none', pid: null, running: false });

const modeLabel = computed(() => {
  const { mode, running } = modeStatus.value;
  if (mode === 'cli' && running) return t('settings.runMode.cliRunningShort');
  if (mode === 'tui' && running) return t('settings.runMode.tuiRunningShort');
  return t('settings.runMode.guiCurrent');
});

async function refreshModeStatus() {
  try {
    modeStatus.value = await command('get_mode_status');
  } catch (e) {
    console.error('get_mode_status failed:', e);
  }
}

async function switchMode(mode: 'cli' | 'tui') {
  const ok = confirm(t(mode === 'cli' ? 'settings.runMode.confirmSwitch' : 'settings.runMode.confirmSwitchTui'));
  if (!ok) return;
  try {
    await command(mode === 'cli' ? 'switch_to_cli' : 'switch_to_tui');
    await command('exit_app');
  } catch (e) {
    console.error(`switch_to_${mode} failed:`, e);
    alert(`${t('settings.runMode.switchFailed')}: ${e}`);
    void refreshModeStatus();
  }
}

onMounted(() => {
  void refreshModeStatus();
  void refreshAutostart();
  void fetchDevices();
});
</script>

<template>
  <div class="space-y-6">
    <!-- Run Mode -->
    <div class="bg-surface-bright/60 backdrop-blur-lg rounded-2xl p-4 shadow-xs border border-white/5">
      <div class="flex items-center justify-between">
        <div>
          <h4 class="font-bold text-on-surface">{{ $t('settings.runMode.title') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.runMode.desc') }}</p>
        </div>
        <span
          class="px-3 py-1 rounded-full text-xs font-semibold"
          :class="
            modeStatus.mode !== 'gui' && modeStatus.mode !== 'none' && modeStatus.running
              ? 'bg-warning-container/40 text-warning'
              : 'bg-primary-container/40 text-primary'
          "
        >
          {{ modeLabel }}
        </span>
      </div>
      <p v-if="modeStatus.mode === 'cli' && modeStatus.running" class="mt-2 text-xs text-warning">
        {{ $t('settings.runMode.cliRunning', { pid: modeStatus.pid }) }}
      </p>
      <p v-if="modeStatus.mode === 'tui' && modeStatus.running" class="mt-2 text-xs text-warning">
        {{ $t('settings.runMode.tuiRunning', { pid: modeStatus.pid }) }}
      </p>
      <div class="mt-3 grid grid-cols-2 gap-2">
        <button
          @click="switchMode('cli')"
          class="w-full rounded-xl bg-primary px-4 py-2.5 text-sm font-semibold text-on-primary hover:opacity-90 transition-opacity"
        >
          {{ $t('settings.runMode.switchButton') }}
        </button>
        <button
          @click="switchMode('tui')"
          class="w-full rounded-xl bg-secondary px-4 py-2.5 text-sm font-semibold text-on-secondary hover:opacity-90 transition-opacity"
        >
          {{ $t('settings.runMode.switchTuiButton') }}
        </button>
      </div>
    </div>

    <SettingRow :title="$t('settings.language.title')" :desc="$t('settings.language.desc')">
      <Select v-model="currentLanguage">
        <SelectTrigger class="w-[140px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
          <SelectValue placeholder="Language" />
        </SelectTrigger>
        <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
          <SelectGroup>
            <SelectItem value="system">{{ $t('settings.language.system') }}</SelectItem>
            <SelectItem v-for="[value, label] in languages" :key="value" :value="value">{{ label }}</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </SettingRow>

    <SettingRow :title="$t('closeBehavior.title')" :desc="$t('closeBehavior.desc')">
      <Select v-model="closeBehavior">
        <SelectTrigger class="w-[160px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
          <SelectValue :placeholder="$t('closeBehavior.ask')" />
        </SelectTrigger>
        <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
          <SelectGroup>
            <SelectItem value="ask">{{ $t('closeBehavior.ask') }}</SelectItem>
            <SelectItem value="hide">{{ $t('closeBehavior.hide') }}</SelectItem>
            <SelectItem value="exit">{{ $t('closeBehavior.exit') }}</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </SettingRow>

    <SettingRow :title="$t('startMinimized.title')" :desc="$t('startMinimized.desc')">
      <MD3Switch v-model="startMinimized" />
    </SettingRow>

    <SettingRow :title="$t('autostart.title')" :desc="$t('autostart.desc')">
      <MD3Switch :model-value="autostartEnabled" @update:model-value="setAutostart" />
    </SettingRow>

    <SettingRow :title="$t('notifications.title')" :desc="$t('notifications.desc')">
      <MD3Switch v-model="notificationsEnabled" />
    </SettingRow>

    <SettingRow :title="$t('autoStream.title')" :desc="$t('autoStream.desc')">
      <MD3Switch v-model="autoStream" />
    </SettingRow>

    <SettingRow :title="$t('muteSync.title')" :desc="$t('muteSync.desc')">
      <MD3Switch v-model="muteSyncEnabled" />
    </SettingRow>

    <SettingRow :title="$t('settings.pocketMode.title')" :desc="$t('settings.pocketMode.desc')">
      <MD3Switch v-model="pocketMode" />
    </SettingRow>

    <!-- Output Device -->
    <div class="bg-surface-bright rounded-2xl p-4 space-y-4 shadow-xs">
      <div>
        <h4 class="font-bold text-on-surface">{{ $t('settings.audioOutput.title') }}</h4>
        <p class="text-xs text-on-surface-variant">{{ $t('settings.audioOutput.desc') }}</p>
      </div>
      <Select v-model="settings.audioDevice">
        <SelectTrigger class="w-full bg-surface-container border-none shadow-none rounded-xl h-12 px-4 font-medium text-sm">
          <SelectValue :placeholder="$t('settings.audioOutput.auto')" />
        </SelectTrigger>
        <SelectContent class="border-surface-variant/20 rounded-xl bg-surface shadow-lg max-h-[40vh]">
          <SelectGroup>
            <SelectItem value="auto">{{ $t('settings.audioOutput.auto') }}</SelectItem>
            <SelectItem v-for="dev in audioDevices" :key="dev" :value="dev">{{ dev }}</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>

      <div v-if="isVirtualDeviceSelected" class="flex items-center gap-1.5 text-xs text-green-400 font-medium">
        <CheckCircle2 class="w-3.5 h-3.5 shrink-0" />
        <span>{{ $t('settings.audioOutput.routingActive') }}</span>
      </div>
      <div
        v-else-if="isExplicitPhysicalDevice || isAutoFallbackToPhysical"
        class="rounded-xl bg-amber-500/10 border border-amber-500/20 p-3 space-y-1 text-xs text-amber-400"
      >
        <div class="flex items-center gap-1.5 font-bold">
          <AlertTriangle class="w-4 h-4 shrink-0 text-amber-400" />
          <span>
            {{
              isExplicitPhysicalDevice
                ? $t('settings.audioOutput.physicalWarningTitle')
                : $t('settings.audioOutput.noVirtualDeviceTitle')
            }}
          </span>
        </div>
        <p class="text-on-surface-variant leading-relaxed">
          {{
            isExplicitPhysicalDevice
              ? $t('settings.audioOutput.physicalWarningDesc')
              : $t('settings.audioOutput.noVirtualDeviceDesc')
          }}
        </p>
      </div>
    </div>

    <VirtualDeviceCard />

    <!-- Restore default settings -->
    <div class="rounded-2xl bg-surface-variant/20 border border-outline/10 p-4">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-bold text-foreground">{{ $t('settings.restoreDefaults.label') }}</p>
          <p class="text-xs text-on-surface-variant mt-0.5">{{ $t('settings.restoreDefaults.desc') }}</p>
        </div>
        <button
          class="shrink-0 rounded-full bg-error/10 px-4 py-2 text-sm font-medium text-error transition-colors hover:bg-error/20"
          @click="restore.request('settings')"
        >
          {{ $t('settings.restoreDefaults.button') }}
        </button>
      </div>
    </div>
  </div>
</template>
