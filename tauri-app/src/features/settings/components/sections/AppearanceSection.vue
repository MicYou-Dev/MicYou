<script setup lang="ts">
import { computed, defineAsyncComponent, ref } from 'vue';
import { Palette } from '@lucide/vue';
import { command } from '@/platform';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/shared/components/ui/select';
import CustomColorPicker from '@/features/theme/components/CustomColorPicker.vue';
// CodeMirror is large: load the editor in its own chunk.
const CustomCssDialog = defineAsyncComponent(() => import('@/features/theme/components/CustomCssDialog.vue'));
import ThemeCatalogDialog from '@/features/theme/components/ThemeCatalogDialog.vue';
import ThemeSelector from '@/features/theme/components/ThemeSelector.vue';
import { useTheme } from '@/features/theme/composables/useTheme';
import { useRestoreDefaults } from '../../composables/useRestoreDefaults';

const {
  colorMode,
  themeMode,
  themeColor,
  uiStyle,
  customH,
  customS,
  customL,
  customVariant,
  systemAccent,
  installedThemeId,
  installedThemeControlsColor,
  clearInstalledTheme,
} = useTheme();
const restore = useRestoreDefaults();

const themePackageActive = computed(() =>
  Boolean(installedThemeId.value && installedThemeControlsColor.value),
);
const showColorPicker = ref(false);
const showCustomCssDialog = ref(false);
const showThemeCatalogDialog = ref(false);

const variants = [
  ['TonalSpot', 'tonalSpot'],
  ['Neutral', 'neutral'],
  ['Vibrant', 'vibrant'],
  ['Expressive', 'expressive'],
  ['Rainbow', 'rainbow'],
  ['FruitSalad', 'fruitSalad'],
  ['Monochrome', 'monochrome'],
  ['Fidelity', 'fidelity'],
  ['Content', 'content'],
] as const;

function applyCustomColor(color: { h: number; s: number; l: number }) {
  customH.value = color.h;
  customS.value = color.s;
  customL.value = color.l;
  themeColor.value = 'theme-custom';
  themeMode.value = 'custom';
}

function openCustomColor() {
  themeMode.value = 'custom';
  showColorPicker.value = true;
}

async function deactivateInstalledTheme() {
  const themeId = installedThemeId.value;
  if (!themeId) return;
  clearInstalledTheme();
  try {
    await command('remove_installed_theme', { themeId });
  } catch (error) {
    console.warn('Failed to remove installed theme:', error);
  }
}
</script>

<template>
  <div class="space-y-6">
    <!-- Theme Mode Settings -->
    <div class="surface-card flex items-center justify-between">
      <div>
        <h4 class="font-bold text-on-surface">{{ $t('settings.theme.title') }}</h4>
        <p class="text-xs text-on-surface-variant">{{ $t('settings.theme.desc') }}</p>
      </div>
      <Select v-model="colorMode">
        <SelectTrigger class="w-[140px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
          <SelectValue :placeholder="$t('settings.theme.auto')" />
        </SelectTrigger>
        <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
          <SelectGroup>
            <SelectItem value="auto">{{ $t('settings.theme.auto') }}</SelectItem>
            <SelectItem value="light">{{ $t('settings.theme.light') }}</SelectItem>
            <SelectItem value="dark">{{ $t('settings.theme.dark') }}</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>

    <div class="relative space-y-6">
      <!-- Theme Color Source -->
      <div class="surface-card flex items-center justify-between">
        <div>
          <h4 class="font-bold text-on-surface">{{ $t('settings.theme.modeTitle') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.theme.modeDesc') }}</p>
        </div>
        <Select v-model="themeMode" :disabled="themePackageActive">
          <SelectTrigger class="w-[140px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
            <SelectValue />
          </SelectTrigger>
          <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
            <SelectGroup>
              <SelectItem value="system">{{ $t('settings.theme.systemColor') }}</SelectItem>
              <SelectItem value="preset">{{ $t('settings.theme.presetColor') }}</SelectItem>
              <SelectItem value="custom">{{ $t('settings.theme.customColor') }}</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>

      <!-- Theme Color Settings -->
      <div class="surface-card relative flex items-center justify-between overflow-hidden">
        <div class="shrink-0 mr-4">
          <h4 class="font-bold text-on-surface">{{ $t('settings.themeColor.title') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.themeColor.desc') }}</p>
        </div>
        <div class="flex justify-end">
          <ThemeSelector
            v-model="themeColor"
            :custom-h="customH"
            :custom-s="customS"
            :custom-l="customL"
            :disabled="themePackageActive"
            @update:model-value="themeMode = 'preset'"
            @open-custom="openCustomColor"
          />
        </div>

        <div
          v-if="!themePackageActive && themeMode === 'system'"
          class="absolute inset-0 z-10 flex items-center justify-center gap-3 bg-surface-bright/80 px-4 backdrop-blur-xs"
        >
          <span
            class="h-8 w-8 shrink-0 rounded-full border border-outline/30 shadow-xs"
            :style="{ backgroundColor: systemAccent.hex }"
          ></span>
          <div class="min-w-0">
            <p class="text-sm font-medium text-on-surface">
              {{ systemAccent.supported ? $t('settings.theme.systemReady') : $t('settings.theme.systemUnavailable') }}
            </p>
            <p class="text-xs text-on-surface-variant">
              {{ $t('settings.theme.systemSource', { source: systemAccent.source }) }}
            </p>
          </div>
        </div>
      </div>

      <!-- Theme Generation Variant Settings -->
      <div class="surface-card flex items-center justify-between">
        <div>
          <h4 class="font-bold text-on-surface">{{ $t('settings.customColor.variant') }}</h4>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.customColor.variantDesc') }}</p>
        </div>
        <Select v-model="customVariant" :disabled="themePackageActive">
          <SelectTrigger class="w-[160px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
            <SelectValue />
          </SelectTrigger>
          <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
            <SelectGroup>
              <SelectItem v-for="[value, key] in variants" :key="value" :value="value">
                {{ $t(`settings.customColor.variants.${key}`) }}
              </SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>

      <div
        v-if="themePackageActive"
        class="absolute inset-0 z-20 mt-0! flex min-h-full items-center justify-center rounded-2xl bg-surface-bright/90 p-6 text-center shadow-lg backdrop-blur-md"
      >
        <div class="flex max-w-sm flex-col items-center gap-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-full bg-primary/15 text-primary">
            <Palette class="h-6 w-6" />
          </div>
          <p class="text-sm font-semibold text-on-surface">{{ $t('settings.theme.packageActive') }}</p>
          <p class="text-xs text-on-surface-variant">
            {{ $t('settings.theme.packageActiveDesc', { id: installedThemeId }) }}
          </p>
          <button
            class="mt-1 rounded-full bg-primary px-4 py-2 text-sm font-medium text-on-primary transition-opacity hover:opacity-90"
            @click="deactivateInstalledTheme"
          >
            {{ $t('settings.theme.deactivate') }}
          </button>
        </div>
      </div>
    </div>

    <!-- UI Style Settings -->
    <div class="surface-card flex items-center justify-between">
      <div>
        <h4 class="font-bold text-on-surface">{{ $t('settings.uiStyle.title') }}</h4>
        <p class="text-xs text-on-surface-variant">{{ $t('settings.uiStyle.desc') }}</p>
      </div>
      <Select v-model="uiStyle">
        <SelectTrigger class="w-[140px] bg-surface-container border-none shadow-none rounded-lg text-sm font-medium">
          <SelectValue :placeholder="$t('settings.uiStyle.glass')" />
        </SelectTrigger>
        <SelectContent class="border-surface-variant/20 rounded-lg bg-surface shadow-lg">
          <SelectGroup>
            <SelectItem value="style-default">{{ $t('settings.uiStyle.default') }}</SelectItem>
            <SelectItem value="style-glass">{{ $t('settings.uiStyle.glass') }}</SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>

    <!-- Custom CSS -->
    <div class="surface-card flex items-center justify-between">
      <div class="mr-4 flex-1">
        <h4 class="font-bold text-on-surface">{{ $t('settings.customCss.title') }}</h4>
        <p class="text-xs text-on-surface-variant">{{ $t('settings.customCss.desc') }}</p>
      </div>
      <button
        @click="showCustomCssDialog = true"
        class="shrink-0 rounded-full bg-primary/10 px-4 py-2 text-sm font-medium text-primary transition-colors hover:bg-primary/20"
      >
        {{ $t('settings.customCss.editBtn') }}
      </button>
    </div>

    <div class="surface-card flex items-center justify-between">
      <div class="flex-1 mr-4">
        <h4 class="font-bold text-on-surface">{{ $t('settings.theme.catalogTitle') }}</h4>
        <p class="text-xs text-on-surface-variant">{{ $t('settings.theme.catalogDesc') }}</p>
      </div>
      <button
        class="rounded-full bg-primary/10 px-4 py-2 text-sm font-medium text-primary transition-colors hover:bg-primary/20"
        @click="showThemeCatalogDialog = true"
      >
        {{ $t('settings.theme.catalogButton') }}
      </button>
    </div>

    <!-- Restore default theme -->
    <div class="rounded-2xl bg-surface-variant/20 border border-outline/10 p-4">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-bold text-foreground">{{ $t('settings.restoreTheme.label') }}</p>
          <p class="text-xs text-on-surface-variant mt-0.5">{{ $t('settings.restoreTheme.desc') }}</p>
        </div>
        <button
          class="shrink-0 rounded-full bg-error/10 px-4 py-2 text-sm font-medium text-error transition-colors hover:bg-error/20"
          @click="restore.request('theme')"
        >
          {{ $t('settings.restoreTheme.button') }}
        </button>
      </div>
    </div>

    <Teleport to="body">
      <CustomCssDialog :isOpen="showCustomCssDialog" @close="showCustomCssDialog = false" />
      <ThemeCatalogDialog :isOpen="showThemeCatalogDialog" @close="showThemeCatalogDialog = false" />
      <CustomColorPicker
        :is-open="showColorPicker"
        :initial-h="customH"
        :initial-s="customS"
        :initial-l="customL"
        @close="showColorPicker = false"
        @apply="applyCustomColor"
      />
    </Teleport>
  </div>
</template>
