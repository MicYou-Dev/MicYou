<template>
  <div
    ref="windowRoot"
    data-drag-surface
    class="settings-panel settings-window relative isolate origin-center"
    @mousedown="startDrag"
  >
    <!-- Window background; also the compositor blur region in glass style -->
    <div
      data-blur-region
      class="absolute inset-0 -z-10 rounded-2xl bg-surface-container pointer-events-none"
    ></div>
    <button
      @click="closeWindow"
      class="absolute top-4 right-4 z-40 w-10 h-10 rounded-full bg-surface-variant/40 hover:bg-surface-variant/80 flex items-center justify-center transition-colors"
    >
      <X class="w-5 h-5 text-on-surface" />
    </button>

    <!-- Left Sidebar -->
    <div class="settings-nav gap-2">
      <div class="px-4 py-4 mb-4 flex items-center gap-3">
        <SettingsIcon class="w-6 h-6 text-primary" />
        <h2 class="text-xl font-bold text-primary">{{ $t('settings.title') }}</h2>
      </div>

      <button
        v-for="section in baseSections"
        :key="section.id"
        @click="currentSection = section.id"
        class="settings-nav-item"
        :class="navItemClass(section.id)"
      >
        <component :is="section.icon" class="w-5 h-5" :class="{ 'text-primary': currentSection === section.id }" />
        <span class="font-medium text-sm">{{ $t(section.nameKey) }}</span>
      </button>

      <!-- Plugin pages sit below the built-in sections -->
      <template v-if="pluginPanels.length > 0">
        <div class="my-2 mx-3 h-px bg-border/70"></div>
        <button
          v-for="panel in pluginPanels"
          :key="panel.id"
          @click="currentSection = panel.id"
          class="settings-nav-item"
          :class="navItemClass(panel.id)"
        >
          <span
            v-if="panel.icon"
            class="w-5 h-5 text-center text-sm leading-5"
            :class="{ 'text-primary': currentSection === panel.id }"
            >{{ panel.icon }}</span
          >
          <LayoutPanelTop v-else class="w-5 h-5" :class="{ 'text-primary': currentSection === panel.id }" />
          <span class="font-medium text-sm">{{ panel.name }}</span>
        </button>
      </template>
    </div>

    <!-- Right Content -->
    <div
      ref="contentRef"
      data-drag-surface
      class="settings-scrollbar flex-1 bg-surface-container-lowest/50 p-8 overflow-y-auto overscroll-contain"
    >
      <div data-drag-surface class="max-w-2xl mx-auto space-y-8">
        <h3 data-drag-surface class="text-3xl font-bold text-primary">{{ currentSectionName }}</h3>

        <Transition name="fade-slide" mode="out-in">
          <GeneralSection v-if="currentSection === 'general'" key="general" data-drag-surface />
          <AppearanceSection v-else-if="currentSection === 'appearance'" key="appearance" data-drag-surface />
          <AudioSection v-else-if="currentSection === 'audio'" key="audio" data-drag-surface />
          <div v-else-if="currentSection === 'equalizer'" key="equalizer" data-drag-surface class="space-y-6 h-[600px]">
            <EqualizerPanel :config="settings.equalizer" />
          </div>
          <div v-else-if="currentSection === 'plugins'" key="plugins" data-drag-surface class="space-y-6">
            <PluginsPanel />
          </div>
          <AboutSection v-else-if="currentSection === 'about'" key="about" data-drag-surface />
          <PluginPanelSection
            v-else-if="activePanel"
            :key="activePanel.id"
            :plugin-id="activePanel.pluginId"
            :panel-id="activePanel.panelId"
            data-drag-surface
          />
        </Transition>
      </div>
    </div>
  </div>

  <!-- Restore defaults: confirm dialog -->
  <Transition
    enter-active-class="transition ease-out duration-200"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="transition ease-in duration-150"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div v-if="restore.confirmTarget.value" class="fixed inset-0 z-60 flex items-center justify-center bg-black/40 backdrop-blur-xs">
      <div class="bg-surface rounded-2xl shadow-2xl border border-outline/10 p-6 w-80">
        <h3 class="text-sm font-bold text-foreground mb-2">
          {{ $t(restore.confirmTarget.value === 'settings' ? 'settings.restoreDefaults.title' : 'settings.restoreTheme.title') }}
        </h3>
        <p class="text-xs text-on-surface-variant mb-5">
          {{ $t(restore.confirmTarget.value === 'settings' ? 'settings.restoreDefaults.confirmDesc' : 'settings.restoreTheme.confirmDesc') }}
        </p>
        <div class="flex justify-end gap-2">
          <button
            class="px-4 py-2 text-xs font-medium text-on-surface-variant hover:bg-surface-variant/50 rounded-lg transition-colors"
            @click="restore.confirmTarget.value = null"
          >
            {{ $t('dialogs.cancel') }}
          </button>
          <button
            class="px-4 py-2 text-xs font-medium text-on-primary bg-primary hover:bg-primary/90 rounded-lg transition-colors"
            @click="restore.confirm"
          >
            {{ $t('dialogs.confirm') }}
          </button>
        </div>
      </div>
    </div>
  </Transition>

  <!-- Restore defaults: result dialog -->
  <Transition
    enter-active-class="transition ease-out duration-200"
    enter-from-class="opacity-0"
    enter-to-class="opacity-100"
    leave-active-class="transition ease-in duration-150"
    leave-from-class="opacity-100"
    leave-to-class="opacity-0"
  >
    <div v-if="restore.result.value" class="fixed inset-0 z-60 flex items-center justify-center bg-black/40 backdrop-blur-xs">
      <div class="bg-surface rounded-2xl shadow-2xl border border-outline/10 p-6 w-80">
        <h3 class="text-sm font-bold text-foreground mb-2">{{ restore.result.value.title }}</h3>
        <p class="text-xs text-on-surface-variant mb-5">{{ restore.result.value.message }}</p>
        <div class="flex justify-end gap-2">
          <button
            class="px-4 py-2 text-xs font-medium text-on-primary bg-primary hover:bg-primary/90 rounded-lg transition-colors"
            @click="restore.result.value = null"
          >
            {{ $t('dialogs.close') }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  Info,
  LayoutPanelTop,
  Mic,
  Palette,
  Puzzle,
  Settings as SettingsIcon,
  SlidersHorizontal,
  X,
} from '@lucide/vue';
import { appWindow, onEvent } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';
import { applyPlatformClasses } from '@/shared/lib/os';
import EqualizerPanel from '@/features/audio/components/EqualizerPanel.vue';
import PluginsPanel from '@/features/plugins/components/PluginsPanel.vue';
import { useTheme } from '@/features/theme/composables/useTheme';
import { useWindowTransition } from '@/features/window/composables/useWindowTransition';
import { useWindowEffects } from '@/features/window/composables/useWindowEffects';
import { useAudioSettings } from '../composables/useAudioSettings';
import { useLanguageSetting } from '../composables/useLanguageSetting';
import { usePluginPanels } from '../composables/usePluginPanels';
import { useRestoreDefaults } from '../composables/useRestoreDefaults';
import { useSettingsRequest } from '../composables/useSettingsRequest';
import AboutSection from './sections/AboutSection.vue';
import AppearanceSection from './sections/AppearanceSection.vue';
import AudioSection from './sections/AudioSection.vue';
import GeneralSection from './sections/GeneralSection.vue';
import PluginPanelSection from './sections/PluginPanelSection.vue';

// Settings live in their own window (#/settings); see useAudioSettings for how
// the main window learns about output device changes.
applyPlatformClasses();
const { t, locale } = useI18n();
const { uiStyle } = useTheme();
const windowEffects = useWindowEffects({
  blur: computed(() => uiStyle.value === 'style-glass'),
  shadowRadius: ref(16),
});
const windowRoot = ref<HTMLElement | null>(null);
const transition = useWindowTransition(windowRoot, { suspendEffects: windowEffects.suspend });

const { settings, loadSettings } = useAudioSettings();
const { saveUiPrefs } = useLanguageSetting();
const restore = useRestoreDefaults();
const { panels: pluginPanels, refresh: refreshPlugins } = usePluginPanels(locale);

// Closing only hides the window: building a new webview on every open is
// what made the settings slow to appear. open_settings_window shows this
// same window again and tells this page to bring its content back.
let closing = false;
let hidden = false;
async function closeWindow() {
  if (closing || hidden) return;
  closing = true;
  await transition.exit();
  try {
    await appWindow.hideCurrentWindow();
    hidden = true;
    // Unmounts the audio section, which stops the spectrum stream.
    currentSection.value = 'general';
    // Stored by the backend while hidden and applied the moment the window
    // shows again, together with it.
    void windowEffects.apply();
  } catch (e) {
    console.error('Hiding settings failed:', e);
    await appWindow.destroyCurrentWindow().catch(() => {});
  } finally {
    closing = false;
  }
}

async function reopen() {
  hidden = false;
  void transition.enter();
  void refreshPlugins();
  // Refresh from the shared settings.json so CLI-side changes show up.
  await loadSettings();
}

// The frameless window drags from the sidebar and from the bare background
// of the content pane (marked data-drag-surface), never from controls.
const INTERACTIVE = 'button, a, input, select, textarea, iframe, [role="button"], [role="switch"], [role="combobox"]';
function startDrag(e: MouseEvent) {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest(INTERACTIVE)) return;
  // A press on a scrollbar targets the scrolling element itself but lies
  // outside its client area; let it scroll instead of moving the window.
  if (e.offsetX >= target.clientWidth || e.offsetY >= target.clientHeight) return;
  if (target.closest('.settings-nav') || target.hasAttribute('data-drag-surface')) {
    void appWindow.startDragging().catch((err) => console.error('Drag failed:', err));
  }
}

const baseSections = [
  { id: 'general', nameKey: 'settings.categories.general', icon: SettingsIcon },
  { id: 'appearance', nameKey: 'settings.categories.appearance', icon: Palette },
  { id: 'audio', nameKey: 'settings.categories.audio', icon: Mic },
  { id: 'equalizer', nameKey: 'settings.equalizer.title', icon: SlidersHorizontal },
  { id: 'plugins', nameKey: 'settings.categories.plugins', icon: Puzzle },
  { id: 'about', nameKey: 'settings.categories.about', icon: Info },
];

const contentRef = ref<HTMLElement | null>(null);
const currentSection = ref('general');
const activePanel = computed(() => pluginPanels.value.find((p) => p.id === currentSection.value));
const currentSectionName = computed(() => {
  const base = baseSections.find((s) => s.id === currentSection.value);
  return base ? t(base.nameKey) : (activePanel.value?.name ?? '');
});

const navItemClass = (id: string) =>
  currentSection.value === id
    ? 'bg-secondary-container/80 text-on-secondary-container shadow-xs scale-[1.02]'
    : 'hover:bg-surface-variant/30 text-on-surface-variant';

watch(currentSection, () => contentRef.value?.scrollTo({ top: 0 }));

const track = useListeners();
onMounted(async () => {
  await windowEffects.apply();
  void transition.enter();
  await track(appWindow.onCloseRequested(() => void closeWindow()));
  await track(onEvent('settings-window-shown', () => {
    if (hidden) void reopen();
  }));
  saveUiPrefs();
  // Plugin pages show in the sidebar before the plugins section is opened.
  void refreshPlugins();
  // Refresh from the shared settings.json so CLI-side changes show up
  await loadSettings();
});

// The macOS app menu asks for a section, or for the sponsors dialog that lives
// in the about section, through shared storage.
useSettingsRequest((section) => {
  if (baseSections.some((s) => s.id === section)) currentSection.value = section;
});
</script>
