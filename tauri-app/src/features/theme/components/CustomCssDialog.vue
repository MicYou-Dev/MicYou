<template>
  <Transition name="dialog">
    <div v-if="isOpen" class="fixed inset-0 z-60 flex items-center justify-center bg-black/40 p-4 backdrop-blur-xs" @click.self="close">
      <div class="theme-editor flex h-[80vh] w-full max-w-3xl flex-col backdrop-blur-2xl">
        <div class="flex items-center justify-between border-b border-surface-variant/20 bg-surface/50 p-6">
          <div>
            <h2 class="text-xl font-bold text-primary">{{ $t('settings.customCss.title') }}</h2>
            <p class="text-xs text-on-surface-variant">{{ $t('settings.customCss.desc') }}</p>
          </div>
          <div class="flex items-center gap-2">
            <label class="flex items-center gap-2 rounded-full bg-surface-container px-3 py-2 text-xs text-on-surface-variant">
              <input v-model="customCssEnabled" type="checkbox" class="accent-primary" />
              {{ $t('settings.customCss.enabled') }}
            </label>
            <button class="theme-editor-action" @click="triggerFileInput">
              <Upload class="h-4 w-4" /> {{ $t('settings.customCss.loadFromFile') }}
            </button>
            <button class="theme-editor-action" @click="exportCss">
              <Download class="h-4 w-4" /> {{ $t('settings.customCss.export') }}
            </button>
            <button class="theme-editor-action text-error hover:bg-error/20" @click="clearCss">
              <Trash2 class="h-4 w-4" /> {{ $t('settings.customCss.clear') }}
            </button>
            <button class="ml-2 rounded-full p-2 transition-colors hover:bg-surface-variant/50" @click="close">
              <X class="h-5 w-5 text-on-surface" />
            </button>
          </div>
        </div>

        <div class="flex flex-1 flex-col overflow-hidden bg-surface-container-lowest/50 p-6">
          <div class="flex-1 overflow-hidden rounded-xl border border-surface-variant/20 bg-surface shadow-inner">
            <Codemirror
              v-model="customCss"
              :placeholder="$t('settings.customCss.placeholder')"
              :style="{ height: '100%', width: '100%' }"
              :autofocus="true"
              :indent-with-tab="true"
              :tab-size="2"
              :extensions="extensions"
            />
          </div>
          <details class="mt-3 rounded-xl bg-surface-container/60 px-4 py-3 text-xs text-on-surface-variant">
            <summary class="cursor-pointer font-medium text-on-surface">{{ $t('settings.customCss.referenceTitle') }}</summary>
            <pre class="mt-2 whitespace-pre-wrap font-mono leading-relaxed">{{ cssReference }}</pre>
          </details>
        </div>

        <input ref="fileInput" type="file" class="hidden" accept=".css,.txt" @change="handleFileChange" />
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { Download, Trash2, Upload, X } from '@lucide/vue';
import { Codemirror } from 'vue-codemirror';
import { css } from '@codemirror/lang-css';
import { oneDark } from '@codemirror/theme-one-dark';
import { useTheme } from '../composables/useTheme';

defineProps<{ isOpen: boolean }>();
const emit = defineEmits(['close']);

const extensions = [css(), oneDark];
const { customCss, customCssEnabled } = useTheme();
const fileInput = ref<HTMLInputElement | null>(null);

const cssReference = `/* Theme tokens */
--primary, --secondary, --tertiary
--background, --surface, --surface-container
--foreground, --on-surface, --on-surface-variant
--outline, --border, --error

/* Semantic classes */
.surface-card  .surface-dialog  .settings-panel
.popup-panel   .theme-editor    .is-active
.is-disabled   .has-glass`;

const triggerFileInput = () => fileInput.value?.click();

const handleFileChange = (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  const reader = new FileReader();
  reader.onload = (loadEvent) => {
    const content = loadEvent.target?.result;
    if (typeof content === 'string') customCss.value = content;
  };
  reader.readAsText(file);
  target.value = '';
};

const clearCss = () => {
  customCss.value = '';
};

const exportCss = () => {
  const blob = new Blob([customCss.value], { type: 'text/css;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = 'micyou-theme.css';
  anchor.click();
  URL.revokeObjectURL(url);
};

const close = () => emit('close');
</script>
