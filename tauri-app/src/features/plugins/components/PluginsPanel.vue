<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import PluginDetailsDialog from './PluginDetailsDialog.vue';
import PluginMarketDialog from './PluginMarketDialog.vue';
import { ref, computed, onMounted, watch } from 'vue';
import { appWindow, type PluginView } from '@/platform';
import { useListeners } from '@/shared/lib/listeners';
import {
  RefreshCw,
  Puzzle,
  FolderOpen,
  Download,
  Trash2,
  ToggleLeft,
  ToggleRight,
  TerminalSquare,
  Store,
  Search,
} from '@lucide/vue';
import { usePlugins } from '../composables/usePlugins';

// 可复用的插件管理面板：用于设置对话框的「插件」页面
// 首次挂载即拉取插件列表（单例状态，两个入口共享）
const p = usePlugins();
const { locale } = useI18n();

function pickI18n(map: Record<string, string> | undefined, fallback: string): string {
  if (!map) return fallback;
  const loc = locale.value.toLowerCase();
  const base = loc.split('-')[0];
  const entries = Object.entries(map);
  // 精确（原样/小写）→ 语言前缀双向（宿主 zh 命中插件 zh-CN，反之亦然）
  const direct = map[locale.value] ?? entries.find(([k]) => k.toLowerCase() === loc)?.[1];
  if (direct) return direct;
  const prefixed = map[base] ?? entries.find(([k]) => k.toLowerCase().split('-')[0] === base)?.[1];
  return prefixed ?? fallback;
}

function displayName(plugin: { name: string; nameI18n?: Record<string, string> }): string {
  return pickI18n(plugin.nameI18n, plugin.name);
}

// 描述本地化：descriptionI18n 命中当前语言（或语言前缀）时优先，否则回退基础描述
function displayDescription(plugin: {
  description?: string | null;
  descriptionI18n?: Record<string, string>;
}): string {
  return pickI18n(plugin.descriptionI18n, plugin.description ?? '');
}

const dragOver = ref(false);
const track = useListeners();

onMounted(async () => {
  p.refresh();
  // 拖拽插件文件夹 / .zip 上传（对应文案「把插件文件夹或 .zip 放进来」）
  try {
    await track(appWindow.onFileDrop((event) => {
      if (event.type === 'enter' || event.type === 'over') {
        dragOver.value = true;
      } else if (event.type === 'leave') {
        dragOver.value = false;
      } else {
        dragOver.value = false;
        for (const path of event.paths) {
          void p.importFromPath(path);
        }
      }
    }));
  } catch (e) {
    console.error('Failed to listen drag-drop:', e);
  }
});

const uiConfigs = ref<Record<string, Record<string, unknown>>>({});

// 对声明了按钮面板（ui.route === 'buttons'）的插件加载其配置（音效列表）
async function loadUiConfigs() {
  const next: Record<string, Record<string, unknown>> = {};
  for (const plugin of p.plugins.value) {
    if (plugin.ui?.route === 'buttons' && plugin.loaded) {
      try {
        next[plugin.id] = await p.getConfig(plugin);
      } catch {
        // 单个插件配置读取失败不影响其他按钮面板
      }
    }
  }
  uiConfigs.value = next;
}
watch(
  () => [p.plugins.value, p.syncStatus.value],
  () => loadUiConfigs(),
);

function runtimeLabel(runtime: string) {
  return runtime === 'wasm' ? 'WASM' : 'Native';
}

function kindLabel(kind: string) {
  return `plugins.kind.${kind}`;
}

const detailsPlugin = ref<PluginView | null>(null);
const detailsTab = ref<'config' | 'logs'>('config');

function openDetails(plugin: PluginView, tab: 'config' | 'logs') {
  detailsPlugin.value = plugin;
  detailsTab.value = tab;
}

const searchQuery = ref('');
const filteredPlugins = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return p.plugins.value;
  return p.plugins.value.filter(
    (pl) =>
      pl.name.toLowerCase().includes(q) ||
      pl.id.toLowerCase().includes(q) ||
      (pl.description ?? '').toLowerCase().includes(q),
  );
});

const uninstallTarget = ref<string | null>(null);

function requestUninstall(plugin: PluginView) {
  uninstallTarget.value = plugin.id;
}

function cancelUninstall() {
  uninstallTarget.value = null;
}

async function confirmUninstall() {
  if (!uninstallTarget.value) return;
  const target = uninstallTarget.value;
  uninstallTarget.value = null;
  const plugin = p.plugins.value.find((pl) => pl.id === target);
  if (plugin) await p.uninstall(plugin);
}

const checking = ref(false);
const marketOpen = ref(false);
const updates = ref<{ id: string; currentVersion: string; latestVersion: string }[]>([]);

async function checkUpdates() {
  checking.value = true;
  updates.value = await p.checkUpdates();
  checking.value = false;
}

async function applyUpdate(id: string) {
  const ok = await p.updatePlugin(id);
  if (ok) {
    updates.value = updates.value.filter((u) => u.id !== id);
  }
}
</script>

<template>
  <div class="space-y-4 relative" :class="dragOver ? 'ring-2 ring-primary/60 rounded-xl' : ''">
    <!-- 拖拽悬停提示：把插件文件夹或 .zip 放进来 -->
    <div
      v-if="dragOver"
      class="absolute inset-0 z-20 rounded-xl bg-primary/10 backdrop-blur-xs border-2 border-dashed border-primary flex items-center justify-center pointer-events-none"
    >
      <span class="text-sm font-bold text-primary">{{ $t('plugins.installHint') }}</span>
    </div>
    <!-- Toolbar -->
    <div class="flex items-center justify-end">
      <div class="flex items-center gap-2">
        <button
          @click="marketOpen = true"
          class="inline-flex items-center gap-1.5 px-3 h-9 rounded-full bg-primary/15 text-primary hover:bg-primary/25 text-xs font-medium transition-colors"
        >
          <Store class="w-4 h-4" />
          {{ $t('plugins.market') }}
        </button>
        <button
          @click="p.refresh"
          class="w-9 h-9 rounded-full bg-surface-variant/40 hover:bg-surface-variant flex items-center justify-center transition-all duration-150 active:scale-90"
          :title="$t('plugins.refresh')"
        >
          <RefreshCw
            class="w-4 h-4 text-on-surface-variant"
            :class="{ 'animate-spin': p.loading.value }"
          />
        </button>
      </div>
    </div>
    <PluginMarketDialog :is-open="marketOpen" @close="marketOpen = false" />
    <p v-if="p.error.value" class="px-4 py-2 rounded-lg bg-red-500/10 text-red-400 text-sm">
      {{ p.error.value }}
    </p>
    <div
      v-if="p.loading.value && p.plugins.value.length === 0"
      class="py-16 text-center text-on-surface-variant text-sm"
    >
      {{ $t('plugins.loading') }}
    </div>
    <div v-else-if="p.plugins.value.length === 0" class="py-16 text-center">
      <p class="text-on-surface-variant text-sm">{{ $t('plugins.noPlugins') }}</p>
      <div class="mt-4 flex items-center justify-center gap-3">
        <button
          @click="p.importPlugin()"
          :disabled="p.busyId.value === 'import'"
          class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-primary/20 text-primary hover:bg-primary/30 text-sm font-medium disabled:opacity-50"
        >
          <Download class="w-4 h-4" />
          {{ p.busyId.value === 'import' ? $t('plugins.importing') : $t('plugins.import') }}
        </button>
        <button
          @click="p.openDir()"
          class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant text-sm font-medium"
        >
          <FolderOpen class="w-4 h-4" />
          {{ $t('plugins.openDir') }}
        </button>
        <button
          @click="checkUpdates()"
          :disabled="checking"
          class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant text-sm font-medium"
        >
          <RefreshCw :class="checking ? 'animate-spin' : ''" class="w-4 h-4" />
          {{ $t('plugins.checkUpdates') }}
        </button>
      </div>
      <!-- Update banner -->
      <div
        v-if="updates.length"
        class="mt-3 rounded-lg border border-sky-500/30 bg-sky-500/10 px-4 py-3 text-xs text-sky-300 space-y-2"
      >
        <div v-for="u in updates" :key="u.id" class="flex items-center justify-between gap-3">
          <span> {{ u.id }}: {{ u.currentVersion }} → {{ u.latestVersion }} </span>
          <button
            @click="applyUpdate(u.id)"
            :disabled="p.busyId.value === u.id + ':update'"
            class="px-3 py-1 rounded-full bg-sky-500/20 hover:bg-sky-500/30 text-sky-200 text-xs font-medium disabled:opacity-50"
          >
            {{
              p.busyId.value === u.id + ':update' ? $t('plugins.updating') : $t('plugins.updateNow')
            }}
          </button>
        </div>
      </div>
    </div>
    <template v-else>
      <!-- Install hint: import zip + open dir -->
      <div
        class="rounded-lg bg-surface-variant/20 px-4 py-3 text-xs text-on-surface-variant space-y-2"
      >
        <div class="flex items-center justify-between gap-3">
          <span>{{ $t('plugins.installHint') }}</span>
          <button
            @click="p.importPlugin()"
            :disabled="p.busyId.value === 'import'"
            class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-primary/20 text-primary hover:bg-primary/30 font-medium disabled:opacity-50"
          >
            <Download class="w-3.5 h-3.5" />
            {{ p.busyId.value === 'import' ? $t('plugins.importing') : $t('plugins.import') }}
          </button>
        </div>
        <div class="flex items-center justify-between gap-3">
          <span>{{ $t('plugins.installHintDir') }}</span>
          <button
            @click="p.openDir()"
            class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface-variant/40 hover:bg-surface-variant text-on-surface-variant font-medium"
          >
            <FolderOpen class="w-3.5 h-3.5" />
            {{ $t('plugins.openDir') }}
          </button>
        </div>
      </div>
      <!-- Search -->
      <div class="relative">
        <Search
          class="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-on-surface-variant/60"
        />
        <input
          v-model="searchQuery"
          type="text"
          :placeholder="$t('plugins.search')"
          class="w-full h-10 pl-9 pr-3 rounded-full bg-surface-variant/20 text-sm text-on-surface outline-hidden placeholder:text-on-surface-variant/60 focus:ring-1 focus:ring-primary/40"
        />
      </div>
      <p
        v-if="filteredPlugins.length === 0"
        class="py-8 text-center text-sm text-on-surface-variant"
      >
        {{ $t('plugins.noPlugins') }}
      </p>
      <TransitionGroup name="plug" tag="div" class="space-y-3">
        <div
          v-for="plugin in filteredPlugins"
          :key="plugin.id"
          class="rounded-xl bg-surface-container-lowest/60 border border-surface-variant/20 p-4 transition-all duration-200 hover:border-primary/30 hover:shadow-lg hover:shadow-black/20"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <h3 class="font-bold text-on-surface">{{ displayName(plugin) }}</h3>
                <span
                  class="px-2 py-0.5 rounded-md text-[10px] font-semibold tracking-wide"
                  :class="
                    plugin.runtime === 'wasm'
                      ? 'bg-purple-500/15 text-purple-400'
                      : 'bg-amber-500/15 text-amber-400'
                  "
                >
                  {{ runtimeLabel(plugin.runtime) }}
                </span>
                <span class="px-2 py-0.5 rounded-md text-[10px] bg-primary/10 text-primary">
                  {{ $t(kindLabel(plugin.kind)) }}
                </span>
                <span
                  v-if="plugin.dspNode"
                  class="px-2 py-0.5 rounded-md text-[10px] bg-green-500/15 text-green-400"
                >
                  {{ $t('plugins.inChain') }}
                </span>
              </div>
              <p class="text-xs text-on-surface-variant mt-1 font-mono">
                {{ plugin.id }} · v{{ plugin.version
                }}<template v-if="plugin.author"> · {{ plugin.author }}</template>
              </p>
              <p
                v-if="displayDescription(plugin)"
                class="text-xs text-on-surface-variant/80 mt-1 line-clamp-2"
              >
                {{ displayDescription(plugin) }}
              </p>
              <p v-if="plugin.error" class="text-xs text-red-400 mt-1">{{ plugin.error }}</p>
              <div v-if="plugin.capabilities.length" class="flex flex-wrap gap-1 mt-2">
                <span
                  v-for="cap in plugin.capabilities"
                  :key="cap"
                  class="px-1.5 py-0.5 rounded text-[10px] bg-surface-variant/30 text-on-surface-variant/70 font-mono"
                >
                  {{ cap }}
                </span>
              </div>
            </div>
            <div class="flex flex-col items-end gap-3 shrink-0">
              <div class="flex items-center gap-2">
              <button
                @click="openDetails(plugin, 'logs')"
                class="w-9 h-9 rounded-full bg-surface-variant/40 hover:bg-surface-variant flex items-center justify-center transition-all duration-150 active:scale-90"
                :title="$t('plugins.logs')"
              >
                <TerminalSquare class="w-4 h-4 text-on-surface-variant" />
              </button>
              <button
                @click="openDetails(plugin, 'config')"
                class="w-9 h-9 rounded-full bg-surface-variant/40 hover:bg-surface-variant flex items-center justify-center transition-all duration-150 active:scale-90"
                :title="$t('plugins.config')"
              >
                <Puzzle class="w-4 h-4 text-on-surface-variant" />
              </button>
              <button
                @click="requestUninstall(plugin)"
                class="w-9 h-9 rounded-full bg-surface-variant/40 hover:bg-red-500/20 flex items-center justify-center transition-colors"
                :title="$t('plugins.uninstall')"
              >
                <Trash2 class="w-4 h-4 text-on-surface-variant hover:text-red-400" />
              </button>
              <button
                @click="p.toggle(plugin)"
                :disabled="p.busyId.value === plugin.id"
                class="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-colors disabled:opacity-50"
                :class="
                  plugin.enabled
                    ? 'bg-primary/20 text-primary'
                    : 'bg-surface-variant/40 text-on-surface-variant'
                "
              >
                <ToggleRight v-if="plugin.enabled" class="w-4 h-4" />
                <ToggleLeft v-else class="w-4 h-4" />
                {{ plugin.enabled ? $t('plugins.enabled') : $t('plugins.disabled') }}
              </button>
              </div>
              <!-- Uninstall confirm: placed in the empty area below the
                   enable toggle / right of the plugin info (was a full-width
                   bar under the whole card) -->
              <Transition name="fade">
                <div
                  v-if="uninstallTarget === plugin.id"
                  class="w-64 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3"
                >
                  <p class="text-xs text-red-300">
                    {{ $t('plugins.uninstallConfirm', { name: displayName(plugin) }) }}
                  </p>
                  <div class="flex gap-2 mt-2 justify-end">
                    <button
                      @click="confirmUninstall"
                      class="px-3 py-1.5 rounded-full text-xs bg-red-500 text-red-950 font-medium hover:bg-red-400"
                    >
                      {{ $t('plugins.uninstall') }}
                    </button>
                    <button
                      @click="cancelUninstall"
                      class="px-3 py-1.5 rounded-full text-xs bg-surface-variant/40 hover:bg-surface-variant"
                    >
                      {{ $t('plugins.cancel') }}
                    </button>
                  </div>
                </div>
              </Transition>
            </div>
          </div>
          <!-- Soundpad panel: ui.route === 'buttons' -->
          <div
            v-if="plugin.ui?.route === 'buttons' && plugin.loaded"
            class="mt-3 pt-3 border-t border-surface-variant/20"
          >
            <span class="text-xs font-medium text-on-surface-variant">{{
              $t('plugins.soundpad')
            }}</span>
            <div
              v-if="(uiConfigs[plugin.id]?.sounds as any[])?.length"
              class="grid grid-cols-3 gap-2 mt-2"
            >
              <button
                v-for="snd in uiConfigs[plugin.id]?.sounds as any[]"
                :key="snd.id"
                @click="p.trigger(plugin, 'play', JSON.stringify({ id: snd.id }))"
                class="px-3 py-2 rounded-lg bg-surface-variant/30 hover:bg-surface-variant text-sm font-medium transition-colors"
              >
                {{ snd.label ?? snd.id }}
              </button>
            </div>
            <p v-else class="mt-2 text-xs text-on-surface-variant/70">
              {{ $t('plugins.soundpadEmpty') }}
            </p>
          </div>
          <!-- 注意：PluginDetailsDialog 已移出 v-for，见列表末尾 -->
        </div>
      </TransitionGroup>
    </template>

    <!-- Details dialog: config + logs（单实例，放在列表之外） -->
    <PluginDetailsDialog
      :plugin="detailsPlugin"
      :tab="detailsTab"
      @close="detailsPlugin = null"
    />
  </div>
</template>

<style scoped>
.plug-enter-active,
.plug-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}
.plug-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.plug-leave-to {
  opacity: 0;
  transform: scale(0.98);
}
.plug-move {
  transition: transform 0.2s ease;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>