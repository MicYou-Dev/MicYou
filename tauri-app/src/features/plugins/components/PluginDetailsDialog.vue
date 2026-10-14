<template>
  <Teleport to="body">
    <Transition name="dialog" appear>
      <div
        v-if="plugin"
        class="fixed inset-0 z-60 flex items-center justify-center p-4 pointer-events-none"
      >
        <div
          class="pointer-events-auto relative w-full max-w-lg max-h-[75vh] flex flex-col rounded-2xl border border-surface-variant/40 bg-surface shadow-2xl"
        >
          <!-- Header -->
          <div class="flex items-center justify-between px-5 py-4 border-b border-surface-variant/30">
            <div class="min-w-0">
              <h2 class="text-base font-semibold truncate">{{ displayName(plugin) }}</h2>
              <p class="text-xs text-on-surface-variant font-mono truncate">{{ plugin.id }}</p>
            </div>
            <button
              class="w-8 h-8 rounded-full hover:bg-surface-variant/40 flex items-center justify-center"
              @click="emit('close')"
            >
              <X class="w-4 h-4" />
            </button>
          </div>
          <!-- Tabs -->
          <div class="flex gap-1 px-4 pt-3">
            <button
              class="px-3 py-1.5 rounded-full text-xs font-medium transition-colors"
              :class="
                activeTab === 'config'
                  ? 'bg-primary/20 text-primary'
                  : 'text-on-surface-variant hover:bg-surface-variant/40'
              "
              @click="activeTab = 'config'"
            >
              {{ $t('plugins.config') }}
            </button>
            <button
              class="px-3 py-1.5 rounded-full text-xs font-medium transition-colors"
              :class="
                activeTab === 'logs'
                  ? 'bg-primary/20 text-primary'
                  : 'text-on-surface-variant hover:bg-surface-variant/40'
              "
              @click="activeTab = 'logs'"
            >
              {{ $t('plugins.logs') }}
            </button>
          </div>
          <!-- Body -->
          <div class="flex-1 overflow-y-auto p-4">
            <Transition name="tab" mode="out-in">
              <!-- Config -->
              <div v-if="activeTab === 'config'" key="config">
                <p v-if="savedHint" class="text-xs text-green-400 mb-2">
                  {{ $t('plugins.configSaved') }}
                </p>
                <PluginConfigForm
                  v-if="plugin.configSchema?.fields?.length"
                  :key="plugin.id"
                  :plugin-id="plugin.id"
                  :schema="plugin.configSchema"
                  @saved="onAutoSaved"
                />
                <template v-else>
                  <textarea
                    v-model="configJson"
                    rows="8"
                    spellcheck="false"
                    class="w-full bg-surface-variant/20 rounded-lg p-2 text-xs font-mono text-on-surface outline-hidden focus:ring-1 focus:ring-primary/40"
                    placeholder='{ "key": "value" }'
                  ></textarea>
                  <div class="flex justify-end mt-2">
                    <button
                      @click="saveConfig"
                      class="px-4 py-1.5 rounded-full bg-primary/20 text-primary hover:bg-primary/30 text-xs font-medium"
                    >
                      {{ $t('plugins.save') }}
                    </button>
                  </div>
                </template>
              </div>
              <!-- Logs -->
              <div v-else key="logs">
                <pre
                  class="plugin-log-box max-h-72 overflow-y-auto bg-black/30 rounded-lg p-3 text-[11px] font-mono text-green-300/90 whitespace-pre-wrap"
                  @selectstart.stop
                  >{{ logLines.join('\n') || $t('plugins.noLogs') }}</pre>
              </div>
            </Transition>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.plugin-log-box,
.plugin-log-box * {
  cursor: text;
  user-select: text !important;
  -webkit-user-select: text !important;
}
.tab-enter-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}
.tab-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
</style>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { X } from '@lucide/vue';
import PluginConfigForm from './PluginConfigForm.vue';
import { usePlugins } from '../composables/usePlugins';
import type { PluginView } from '@/platform';

const props = defineProps<{ plugin: PluginView | null; tab: 'config' | 'logs' }>();
const emit = defineEmits<{ close: [] }>();

const { locale } = useI18n();
const p = usePlugins();

const activeTab = ref<'config' | 'logs'>(props.tab);
const configJson = ref('{}');
const savedHint = ref(false);
const logLines = ref<string[]>([]);

function displayName(plugin: PluginView) {
  const lang = locale.value;
  const map = plugin.nameI18n ?? {};
  if (map[lang]) return map[lang];
  // zh-CN -> zh 前缀回退
  if (lang.startsWith('zh') && map['zh-CN']) return map['zh-CN'];
  return plugin.name;
}

async function loadLogs() {
  if (!props.plugin) return;
  logLines.value = await p.logs(props.plugin);
}

async function saveConfig() {
  if (!props.plugin) return;
  try {
    const parsed = JSON.parse(configJson.value);
    for (const [key, value] of Object.entries(parsed)) {
      await p.saveConfig(props.plugin, key, value);
    }
    savedHint.value = true;
    setTimeout(() => (savedHint.value = false), 2000);
    p.error.value = null;
  } catch (e) {
    p.error.value = String(e);
  }
}

function onAutoSaved() {
  savedHint.value = true;
  setTimeout(() => (savedHint.value = false), 2000);
}

// 打开对话框时初始化：tab 跟随入口、配置预填、日志加载
watch(
  () => [props.plugin, props.tab],
  () => {
    activeTab.value = props.tab;
    configJson.value = '{}';
    savedHint.value = false;
    if (props.plugin) void loadLogs();
  },
  { immediate: true },
);
</script>