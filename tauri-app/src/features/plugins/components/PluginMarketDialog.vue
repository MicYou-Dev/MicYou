<template>
  <Transition name="dialog" appear>
    <div v-if="isOpen" class="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div class="absolute inset-0" @click="emit('close')" />
      <div
        class="relative w-full max-w-2xl max-h-[80vh] flex flex-col rounded-3xl border border-surface-variant/40 bg-surface shadow-2xl"
      >
        <div class="flex items-center justify-between px-5 py-4 border-b border-surface-variant/30">
          <div class="flex items-center gap-2.5">
            <Store class="w-5 h-5 text-primary" />
            <h2 class="text-base font-semibold">{{ $t('plugins.marketTitle') }}</h2>
            <span class="text-xs text-on-surface-variant">MicYou-Plugins</span>
          </div>
          <div class="flex items-center gap-1">
            <button
              class="inline-flex h-8 items-center gap-1.5 rounded-full bg-primary/10 px-3 text-xs font-medium text-primary transition-colors hover:bg-primary/20"
              :title="$t('plugins.marketContribute')"
              @click="openContributionGuide"
            >
              <GitPullRequest class="h-3.5 w-3.5" />
              <span>{{ $t('plugins.marketContribute') }}</span>
            </button>
            <button
              class="w-8 h-8 rounded-full hover:bg-surface-variant/40 flex items-center justify-center"
              :title="$t('plugins.refresh')"
              @click="load(); void refreshInstalled()"
            >
              <Loader2 v-if="isLoading" class="w-4 h-4 animate-spin text-on-surface-variant" />
              <RefreshCw v-else class="w-4 h-4 text-on-surface-variant" />
            </button>
            <button
              class="w-8 h-8 rounded-full hover:bg-surface-variant/40 flex items-center justify-center"
              @click="emit('close')"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
        </div>

        <div class="flex-1 overflow-y-auto p-4 space-y-3">
          <div v-if="!isLoading && !loadError && catalog.plugins.length" class="space-y-3">
            <div class="relative">
              <Search class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-on-surface-variant/60" />
              <input
                v-model="marketQuery"
                type="text"
                :placeholder="$t('plugins.marketSearch')"
                class="w-full h-10 pl-9 pr-3 rounded-full bg-surface-variant/20 text-sm text-on-surface outline-hidden placeholder:text-on-surface-variant/60 focus:ring-1 focus:ring-primary/40"
              />
            </div>
            <div class="flex flex-wrap gap-1.5">
              <button
                v-for="k in ['all', 'dsp', 'utility', 'ui']"
                :key="k"
                class="px-3 py-1 rounded-full text-xs font-medium transition-colors duration-150 active:scale-95"
                :class="
                  kindFilter === k
                    ? 'bg-primary/20 text-primary'
                    : 'bg-surface-variant/30 text-on-surface-variant/80 hover:bg-surface-variant/50'
                "
                @click="kindFilter = k"
              >
                {{ k === 'all' ? $t('plugins.marketAll') : $t('plugins.kind.' + k) }}
              </button>
            </div>
          </div>

          <p v-if="loadError" class="text-sm text-error px-2 py-2">
            {{ $t('plugins.marketFailed', { error: loadError }) }}
            <button class="underline ml-2" @click="load">{{ $t('plugins.retry') }}</button>
          </p>
          <div v-else-if="isLoading" class="py-16 text-center text-sm text-on-surface-variant">
            <Loader2 class="w-5 h-5 animate-spin mx-auto mb-2" />
            {{ $t('plugins.marketLoading') }}
          </div>
          <div
            v-else-if="catalog.plugins.length === 0"
            class="py-16 text-center text-sm text-on-surface-variant"
          >
            {{ $t('plugins.marketEmpty') }}
          </div>
          <div
            v-else-if="filteredCatalog.length === 0"
            class="py-16 text-center text-sm text-on-surface-variant"
          >
            {{ $t('plugins.noPlugins') }}
          </div>

          <div
            v-for="plugin in filteredCatalog"
            :key="plugin.id"
            class="rounded-xl border border-surface-variant/30 bg-surface-bright p-4 transition-all duration-200 hover:border-primary/30 hover:shadow-lg hover:shadow-black/20"
          >
            <div class="flex gap-3">
              <img
                v-if="plugin.previewUrl"
                :src="plugin.previewUrl"
                alt=""
                class="w-24 h-16 rounded-lg object-cover shrink-0 bg-surface-variant/30"
                @error="onPreviewError"
              />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="font-semibold text-sm">{{
                        marketPluginName(plugin, locale)
                      }}</span>
                      <span
                        class="text-xs px-2 py-0.5 rounded-full bg-primary/15 text-primary font-medium"
                      >
                        {{ plugin.version }}
                      </span>
                      <span
                        class="text-xs px-2 py-0.5 rounded-full"
                        :class="
                          plugin.runtime === 'wasm'
                            ? 'bg-emerald-500/15 text-emerald-400'
                            : 'bg-amber-500/15 text-amber-400'
                        "
                      >
                        {{ plugin.runtime === 'wasm' ? 'WASM' : 'Native' }}
                      </span>
                      <span
                        class="text-xs px-2 py-0.5 rounded-full"
                        :class="
                          plugin.kind === 'dsp'
                            ? 'bg-sky-500/15 text-sky-400'
                            : 'bg-surface-variant/50 text-on-surface-variant'
                        "
                      >
                        {{ $t('plugins.kind.' + plugin.kind) }}
                      </span>
                    </div>
                    <p class="text-xs text-on-surface-variant mt-1 truncate">{{ plugin.id }}</p>
                    <p class="text-sm text-on-surface-variant mt-1 line-clamp-2">
                      {{ marketPluginDescription(plugin, locale) || '—' }}
                    </p>
                    <div class="flex flex-wrap gap-1.5 mt-2">
                      <span
                        v-for="cap in plugin.capabilities"
                        :key="cap"
                        class="text-[11px] px-2 py-0.5 rounded-full bg-surface-variant/40 text-on-surface-variant"
                      >
                        {{ cap }}
                      </span>
                      <span
                        v-for="p in plugin.platforms || []"
                        :key="'p' + p"
                        class="text-[11px] px-2 py-0.5 rounded-full bg-indigo-500/15 text-indigo-400"
                      >
                        {{ p }}
                      </span>
                      <span
                        v-for="a in plugin.arches || []"
                        :key="'a' + a"
                        class="text-[11px] px-2 py-0.5 rounded-full bg-indigo-500/15 text-indigo-400"
                      >
                        {{ a }}
                      </span>
                    </div>
                    <p
                      v-if="plugin.author || plugin.license"
                      class="text-[11px] text-on-surface-variant mt-2"
                    >
                      {{ plugin.author ? plugin.author : '' }}
                      <span v-if="plugin.author && plugin.license" class="mx-1">·</span>
                      <span v-if="plugin.license">{{ plugin.license }}</span>
                    </p>
                  </div>
                  <div class="shrink-0 flex flex-col items-end gap-2">
                    <div :ref="(el) => setRowRef(plugin.id, el)" class="flex items-center gap-2">
                      <button
                        v-if="plugin.readmeUrl"
                        class="inline-flex items-center gap-1.5 px-3 py-2 rounded-full text-xs font-medium bg-surface-variant/40 text-on-surface-variant hover:bg-surface-variant/60 transition-colors"
                        @click.stop="openReadme(plugin)"
                      >
                        <BookOpen class="w-3.5 h-3.5" />
                        <span>README</span>
                      </button>

                      <button
                        v-if="installingId === plugin.id && downloadProgress[plugin.id]"
                        class="inline-flex items-center gap-1.5 px-3 py-2 rounded-full text-xs font-medium bg-error/10 text-error hover:bg-error/20 transition-colors"
                        @click.stop="cancelInstall(plugin.id)"
                      >
                        <X class="w-3.5 h-3.5" />
                        <span>{{ $t('plugins.cancel') }}</span>
                      </button>

                      <button
                        class="inline-flex items-center gap-1.5 px-4 py-2 rounded-full text-xs font-medium transition-colors disabled:opacity-50"
                        :class="
                          installedIds.includes(plugin.id)
                            ? 'bg-surface-variant/40 text-on-surface-variant cursor-default'
                            : 'bg-primary text-on-primary hover:bg-primary/90'
                        "
                        :disabled="installedIds.includes(plugin.id) || installingId === plugin.id"
                        @click="install(plugin)"
                      >
                        <template v-if="installingId === plugin.id">
                          <Loader2 class="w-3.5 h-3.5 animate-spin" />
                          <span v-if="downloadProgress[plugin.id] && downloadProgress[plugin.id].total > 0">
                            {{ Math.round((downloadProgress[plugin.id].downloaded / downloadProgress[plugin.id].total) * 100) }}%
                          </span>
                          <span v-else>{{ $t('plugins.marketInstalling') }}</span>
                        </template>
                        <Check v-else-if="installedIds.includes(plugin.id)" class="w-3.5 h-3.5" />
                        <span v-else>
                          {{ $t('plugins.marketInstall') }}
                        </span>
                      </button>
                    </div>
                    <div
                      v-if="confirmingId === plugin.id && preview"
                      class="w-64 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3"
                    >
                      <p class="flex items-start gap-1.5 text-xs font-medium text-on-surface">
                        <TriangleAlert class="w-3.5 h-3.5 shrink-0 mt-0.5 text-amber-500" />
                        <span>{{ $t('plugins.marketConfirm') }}</span>
                      </p>
                      <div class="flex flex-wrap gap-1.5 mt-2">
                        <span
                          v-for="cap in preview.capabilities"
                          :key="cap"
                          class="text-[11px] px-2 py-0.5 rounded-full bg-amber-500/15 text-on-surface-variant"
                        >
                          {{ cap }}
                        </span>
                      </div>
                      <p class="text-[11px] text-on-surface-variant mt-2">
                        {{ $t('plugins.marketConfirmText') }}
                      </p>
                      <div class="flex gap-2 mt-3">
                        <button
                          class="px-3 py-1.5 rounded-full text-xs bg-amber-500 text-amber-950 font-medium hover:bg-amber-400"
                          @click="confirmInstall(plugin)"
                        >
                          {{ $t('plugins.marketInstall') }}
                        </button>
                        <button
                          class="px-3 py-1.5 rounded-full text-xs bg-surface-variant/40 hover:bg-surface-variant"
                          @click="cancelConfirm"
                        >
                          {{ $t('plugins.cancel') }}
                        </button>
                      </div>
                    </div>
                    <button
                      v-if="plugin.homepage"
                      class="inline-flex flex-col items-start gap-0.5 px-3 py-1.5 rounded-xl text-[11px] font-medium text-primary bg-primary/10 hover:bg-primary/20 transition-colors min-w-0 max-w-full"
                      :style="rowMaxStyle(plugin.id)"
                      :title="plugin.homepage"
                      @click.stop="openHomepage(plugin.homepage)"
                    >
                      <span class="inline-flex items-center gap-1.5 shrink-0">
                        <ExternalLink class="w-3 h-3" />
                        <span>{{ $t('plugins.marketHomepage') }}</span>
                      </span>
                      <span class="w-full truncate text-left text-[10px] font-normal text-primary/60">
                        {{ homepageLabel(plugin.homepage) }}
                      </span>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Transition>

  <Teleport to="body">
    <Transition name="readme" appear>
      <div v-if="isOpen && readmePlugin" class="fixed inset-0 z-100 flex flex-col bg-surface">
        <div class="shrink-0 flex items-center justify-between px-5 py-4 border-b border-surface-variant/30">
          <div class="flex items-center gap-2.5 min-w-0">
            <button
              class="w-8 h-8 rounded-full hover:bg-surface-variant/40 flex items-center justify-center shrink-0"
              @click="closeReadme"
            >
              <ArrowLeft class="w-4 h-4" />
            </button>
            <BookOpen class="w-4 h-4 text-primary shrink-0" />
            <h2 class="text-base font-semibold truncate">
              {{ marketPluginName(readmePlugin, locale) }} README
            </h2>
          </div>
          <button
            class="w-8 h-8 rounded-full hover:bg-surface-variant/40 flex items-center justify-center shrink-0"
            @click="closeReadme"
          >
            <X class="w-4 h-4" />
          </button>
        </div>
        <div class="flex-1 min-h-0 overflow-y-auto">
          <div class="max-w-3xl mx-auto px-6 py-8">
            <div v-if="readmeLoading" class="py-16 text-center text-sm text-on-surface-variant">
              <Loader2 class="w-5 h-5 animate-spin mx-auto mb-2" />
              {{ $t('plugins.marketLoading') }}
            </div>
            <div
              v-else-if="readmeError"
              class="p-4 rounded-lg bg-error/10 text-error text-sm border border-error/20"
            >
              {{ readmeError }}
              <button class="underline ml-2" @click="openReadme(readmePlugin!)">
                {{ $t('plugins.retry') }}
              </button>
            </div>
            <div v-else v-html="readmeContent"></div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.readme-enter-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}
.readme-enter-from {
  opacity: 0;
  transform: translateY(12px) scale(0.98);
}
.readme-leave-active {
  transition: opacity 0.15s ease;
}
.readme-leave-to {
  opacity: 0;
}
</style>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { ArrowLeft, BookOpen, Check, ExternalLink, GitPullRequest, Loader2, RefreshCw, Search, Store, TriangleAlert, X } from '@lucide/vue';
import { command, onEvent, openUrl } from '@/platform';
import {
  loadPluginCatalog,
  marketPluginName,
  marketPluginDescription,
  hasLocalizedDescription,
  PLUGIN_CONTRIBUTING_URL,
  type MarketPlugin,
} from '../market';
import { usePlugins } from '../composables/usePlugins';

const props = defineProps<{ isOpen: boolean }>();
const emit = defineEmits<{ close: [] }>();

const { locale } = useI18n();
const pluginsState = usePlugins();

const catalog = ref<{ plugins: MarketPlugin[] }>({ plugins: [] });
const marketQuery = ref('');
const kindFilter = ref<string>('all');
const filteredCatalog = computed(() => {
  const q = marketQuery.value.trim().toLowerCase();
  const matched = catalog.value.plugins.filter((pl) => {
    if (kindFilter.value !== 'all' && pl.kind !== kindFilter.value) return false;
    if (!q) return true;
    return (
      pl.name.toLowerCase().includes(q) ||
      pl.id.toLowerCase().includes(q) ||
      [marketPluginDescription(pl, locale.value), marketPluginName(pl, locale.value)]
        .join('\n')
        .toLowerCase()
        .includes(q)
    );
  });
  // 优先展示当前语言下有本地化描述的插件；稳定分区，组内保持目录原顺序
  const localized: MarketPlugin[] = [];
  const others: MarketPlugin[] = [];
  for (const pl of matched) {
    (hasLocalizedDescription(pl, locale.value) ? localized : others).push(pl);
  }
  return [...localized, ...others];
});

const isLoading = ref(false);
const loadError = ref<string | null>(null);
const installedIds = ref<string[]>([]);
const installingId = ref<string | null>(null);
const confirmingId = ref<string | null>(null);
const preview = ref<{ capabilities: string[] } | null>(null);
const downloadProgress = ref<Record<string, { downloaded: number; total: number }>>({});
const unlistenProgress = ref<(() => void) | null>(null);

const openContributionGuide = () => void openUrl(PLUGIN_CONTRIBUTING_URL);

function openHomepage(url: string) {
  void openUrl(url);
}

function homepageLabel(url: string): string {
  try {
    const u = new URL(url);
    return `${u.host}${u.pathname.replace(/\/+$/, '')}`;
  } catch {
    return url.replace(/^https?:\/\//i, '').replace(/\/+$/, '');
  }
}

const rowWidths = ref<Record<string, number>>({});
const rowObserver =
  typeof ResizeObserver === 'undefined'
    ? null
    : new ResizeObserver((entries) => {
        for (const entry of entries) {
          const id = (entry.target as HTMLElement).dataset.pluginRow;
          if (id) rowWidths.value[id] = entry.contentRect.width;
        }
      });

function setRowRef(id: string, el: unknown) {
  if (el instanceof HTMLElement) {
    el.dataset.pluginRow = id;
    rowWidths.value[id] = el.offsetWidth;
    rowObserver?.observe(el);
  }
}

function rowMaxStyle(id: string): Record<string, string> | undefined {
  const w = rowWidths.value[id];
  return w ? { maxWidth: `${w}px` } : undefined;
}

const readmePlugin = ref<MarketPlugin | null>(null);
const readmeLoading = ref(false);
const readmeError = ref<string | null>(null);
const readmeContent = ref('');

function onPreviewError(e: Event) {
  const img = e.target as HTMLImageElement;
  img.style.display = 'none';
}

async function load() {
  isLoading.value = true;
  loadError.value = null;
  try {
    catalog.value = await loadPluginCatalog();
  } catch (cause) {
    loadError.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    isLoading.value = false;
  }
}

async function refreshInstalled() {
  await pluginsState.refresh();
  installedIds.value = pluginsState.plugins.value.map((p) => p.id);
}

function cancelConfirm() {
  confirmingId.value = null;
  preview.value = null;
}

async function install(plugin: MarketPlugin) {
  if (confirmingId.value === plugin.id) return;
  confirmingId.value = plugin.id;
  preview.value = null;
  try {
    const p = await command('preview_plugin_from_url', {
      manifestUrl: plugin.manifestUrl,
    });
    preview.value = { capabilities: p.capabilities };
  } catch (cause) {
    loadError.value = cause instanceof Error ? cause.message : String(cause);
    confirmingId.value = null;
  }
}

async function confirmInstall(plugin: MarketPlugin) {
  installingId.value = plugin.id;
  cancelledIds.delete(plugin.id);
  try {
    await command('install_plugin_from_url', { id: plugin.id, zipUrl: plugin.downloadUrl });
    if (!installedIds.value.includes(plugin.id)) installedIds.value.push(plugin.id);
    void refreshInstalled();
  } catch (cause) {
    // A cancelled download rejects too; that is not an error to show.
    if (!cancelledIds.has(plugin.id)) {
      loadError.value = cause instanceof Error ? cause.message : String(cause);
    }
  } finally {
    installingId.value = null;
    cancelledIds.delete(plugin.id);
    delete downloadProgress.value[plugin.id];
    cancelConfirm();
  }
}

const cancelledIds = new Set<string>();

async function cancelInstall(id: string) {
  cancelledIds.add(id);
  try {
    await command('cancel_plugin_download', { id });
  } catch (e) {
    console.error(e);
  }
}

async function openReadme(plugin: MarketPlugin) {
  if (!plugin.readmeUrl) return;
  readmePlugin.value = plugin;
  readmeLoading.value = true;
  readmeError.value = null;
  readmeContent.value = '';
  try {
    const res = await fetch(plugin.readmeUrl, { cache: 'no-store' });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    readmeContent.value = parseMarkdown(await res.text());
  } catch (e) {
    readmeError.value = e instanceof Error ? e.message : String(e);
  } finally {
    readmeLoading.value = false;
  }
}

function closeReadme() {
  readmePlugin.value = null;
  readmeContent.value = '';
  readmeError.value = null;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && readmePlugin.value) closeReadme();
}

watch(
  () => props.isOpen,
  (open) => {
    if (!open) closeReadme();
  },
);

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function sanitizeHtml(html: string): string {
  let out = html.replace(
    /<(script|style|iframe|object|embed|form|input|textarea|select|button|meta|link|base|svg|math)\b[^>]*>[\s\S]*?<\/\1\s*>/gi,
    '',
  );
  out = out.replace(
    /<\/?(script|style|iframe|object|embed|form|input|textarea|select|button|meta|link|base|svg|math)\b[^>]*>?/gi,
    '',
  );
  out = out.replace(/\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)?/gi, '');
  out = out.replace(/\s(srcdoc|formaction)\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)?/gi, '');
  out = out.replace(/(href|src)\s*=\s*(["']?)\s*(javascript|vbscript)\s*:[^"'>\s]*\2/gi, '$1="#"');
  out = out.replace(/(href|src)\s*=\s*(["']?)\s*data\s*:\s*text\/html[^"'>\s]*\2/gi, '$1="#"');
  return out;
}

function parseMarkdown(md: string): string {
  const codeBlocks: string[] = [];
  const inlineCodes: string[] = [];

  let html = md.replace(/```[^\n]*\n?[\s\S]*?```/g, (block) => {
    const code = block.replace(/```[^\n]*\n?/, '').replace(/\n?```\s*$/, '');
    codeBlocks.push(escapeHtml(code));
    return `\u0000CB${codeBlocks.length - 1}\u0000`;
  });

  html = html.replace(/`([^`\n]+)`/g, (_, code: string) => {
    inlineCodes.push(escapeHtml(code));
    return `\u0000IC${inlineCodes.length - 1}\u0000`;
  });

  html = sanitizeHtml(html);

  html = html.replace(/^######\s+(.+)$/gm, '<h6 class="text-sm font-bold mt-6 mb-2 text-on-surface">$1</h6>');
  html = html.replace(/^#####\s+(.+)$/gm, '<h5 class="text-base font-bold mt-6 mb-2 text-on-surface">$1</h5>');
  html = html.replace(/^####\s+(.+)$/gm, '<h4 class="text-lg font-bold mt-6 mb-2 text-on-surface">$1</h4>');
  html = html.replace(/^###\s+(.+)$/gm, '<h3 class="text-xl font-bold mt-6 mb-2 text-on-surface border-b border-surface-variant/30 pb-1">$1</h3>');
  html = html.replace(/^##\s+(.+)$/gm, '<h2 class="text-2xl font-bold mt-6 mb-3 text-on-surface border-b border-surface-variant/30 pb-1">$1</h2>');
  html = html.replace(/^#\s+(.+)$/gm, '<h1 class="text-3xl font-bold mt-6 mb-4 text-on-surface">$1</h1>');

  html = html.replace(/\*\*\*([^*]+?)\*\*\*/g, '<strong><em>$1</em></strong>');
  html = html.replace(/\*\*([^*]+?)\*\*/g, '<strong class="font-semibold text-on-surface">$1</strong>');
  html = html.replace(/(^|[^*])\*([^*\n]+?)\*(?!\*)/g, '$1<em>$2</em>');

  html = html.replace(/!\[([^\]]*)\]\(([^)\s]+)[^)]*\)/g, '<img src="$2" alt="$1" class="max-w-full rounded-lg my-3 border border-surface-variant/30" />');
  html = html.replace(/\[([^\]]+)\]\(([^)\s]+)[^)]*\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer" class="text-primary underline hover:text-primary/80 break-all">$1</a>');

  html = html.replace(/^>\s?(.+)$/gm, '<blockquote class="border-l-4 border-primary/60 pl-4 italic text-on-surface-variant my-3 bg-primary/5 py-2 pr-2 rounded-r">$1</blockquote>');

  html = html.replace(/^\s*[-*]\s+(.+)$/gm, '<li class="ml-5 list-disc my-1 text-on-surface">$1</li>');
  html = html.replace(/^\s*\d+\.\s+(.+)$/gm, '<li class="ml-5 list-decimal my-1 text-on-surface">$1</li>');
  html = html.replace(/^(-{3,}|\*{3,}|_{3,})$/gm, '<hr class="my-6 border-surface-variant/30" />');

  const blocks = html.split(/\n{2,}/);
  html = blocks
    .map((block) => {
      block = block.trim();
      if (!block) return '';
      if (/^\u0000CB\d+\u0000$/.test(block)) return block;
      if (/^<(h[1-6]|ul|ol|li|pre|blockquote|img|hr|div|table|p|details|figure|section)/i.test(block)) return block;
      return `<p class="my-3 text-sm text-on-surface leading-relaxed">${block.replace(/\n/g, '<br>')}</p>`;
    })
    .join('\n');

  html = html.replace(/(<li[\s\S]*?<\/li>\s*)+/g, (match) => `<ul class="my-3 space-y-1.5">${match}</ul>`);

  html = html.replace(
    /\u0000CB(\d+)\u0000/g,
    (_, i: string) =>
      `<pre class="bg-slate-800/60 p-4 rounded-lg my-4 overflow-x-auto text-xs text-gray-200 font-mono border border-white/10"><code>${codeBlocks[Number(i)]}</code></pre>`,
  );
  html = html.replace(
    /\u0000IC(\d+)\u0000/g,
    (_, i: string) =>
      `<code class="bg-slate-800/60 px-1.5 py-0.5 rounded text-xs text-emerald-400 font-mono">${inlineCodes[Number(i)]}</code>`,
  );
  return html;
}

onMounted(async () => {
  void load();
  void refreshInstalled();
  window.addEventListener('keydown', onKeydown);

  unlistenProgress.value = await onEvent(
    'plugin-download-progress',
    (payload) => {
      const { id, downloaded, total, done } = payload;
      if (done) {
        delete downloadProgress.value[id];
      } else {
        downloadProgress.value[id] = { downloaded, total };
      }
    }
  );
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
  rowObserver?.disconnect();
  unlistenProgress.value?.();
});
</script>
