<template>
  <Transition name="dialog">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-60 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs"
      @click.self="close"
    >
      <div
        class="bg-surface rounded-3xl w-full max-w-sm max-h-[80vh] shadow-xl overflow-hidden flex flex-col"
      >
        <!-- Header -->
        <div class="flex justify-between items-center p-6 pb-2">
          <div>
            <h2 class="text-xl font-bold text-primary">{{ $t('settings.audioChain.title') }}</h2>
            <p class="text-xs text-on-surface-variant mt-1">
              {{ $t('settings.audioChain.descPopup') }}
            </p>
          </div>
          <div class="flex items-center gap-1">
            <button
              @click="resetChain"
              class="p-2 rounded-full hover:bg-surface-variant/50 transition-colors text-on-surface-variant hover:text-primary"
              :title="$t('settings.audioChain.reset')"
            >
              <RotateCcw class="w-4 h-4" />
            </button>
            <button
              @click="close"
              class="p-2 rounded-full hover:bg-surface-variant/50 transition-colors"
            >
              <X class="w-5 h-5 text-on-surface" />
            </button>
          </div>
        </div>

        <!-- Content -->
        <div class="p-6 overflow-y-auto min-h-0">
          <div class="flex flex-col gap-2 relative">
            <div
              v-for="(item, index) in localChain"
              :key="item"
              :data-index="index"
              class="flex min-w-0 items-center bg-surface-container rounded-xl p-3 border-2 transition-all shadow-xs group select-none relative"
              :class="
                draggedIndex === index
                  ? 'opacity-40 border-primary scale-[0.98] pointer-events-none'
                  : 'border-transparent hover:border-primary/30'
              "
            >
              <template v-if="item !== 'AEC'">
                <div
                  @pointerdown.prevent="onPointerDown(index)"
                  class="w-8 h-8 -ml-2 mr-1 flex items-center justify-center cursor-grab active:cursor-grabbing hover:bg-surface-variant/50 rounded-lg group-hover:text-primary transition-colors opacity-50 group-hover:opacity-100 touch-none"
                >
                  <GripVertical class="w-5 h-5 text-on-surface-variant" />
                </div>
              </template>
              <template v-else>
                <div
                  class="w-8 h-8 -ml-2 mr-1 flex items-center justify-center opacity-40"
                  :title="$t('settings.audioChain.aecPinned')"
                >
                  <Lock class="w-4 h-4 text-on-surface-variant" />
                </div>
              </template>

              <div
                class="w-6 h-6 rounded-full bg-surface flex items-center justify-center text-xs font-bold text-on-surface-variant mr-3 shadow-inner border border-surface-variant/30 group-hover:text-primary group-hover:border-primary/50 transition-colors pointer-events-none"
              >
                {{ index + 1 }}
              </div>

              <div class="flex items-center gap-1.5 flex-1 min-w-0 pointer-events-none">
                <Puzzle
                  v-if="isPluginNode(item)"
                  class="w-3.5 h-3.5 shrink-0 text-primary/70"
                />
                <span class="text-sm font-bold text-on-surface truncate">{{
                  chainLabel(item)
                }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from 'vue';
import { usePlugins } from '@/features/plugins/composables/usePlugins';
import { X, GripVertical, RotateCcw, Lock, Puzzle } from '@lucide/vue';
import { useI18n } from 'vue-i18n';
import {
  normalizeChain,
  chainStageLabel,
  isPluginNode,
  type ChainPlugin,
} from '@/features/audio/chain';

const props = defineProps<{ isOpen: boolean; chain: string[] }>();
const emit = defineEmits(['close', 'update:chain']);

import { useAecStatus } from '@/features/audio/composables/useAecStatus';

const { t, locale } = useI18n();

// AEC 可用性由后端判定：只有平台能提供远场参考时（macOS 的 Core Audio
// process tap 或 BlackHole，Windows/Linux 的现有实现）才开放该节点。
const isAecSupported = useAecStatus().aecSupported;

const localChain = ref<string[]>([]);

// issue #347：每个启用中的 DSP 插件在链中占用独立的 Plugin:<id> 节点，
// 可单独拖拽调整处理顺序；旧 'Plugins' 合成节点由 normalizeChain 就地展开
const pluginsState = usePlugins();
const activeDspPlugins = computed<ChainPlugin[]>(() =>
  pluginsState.plugins.value.filter((p) => p.kind === 'dsp' && p.enabled && p.loaded),
);

// 处理链节点显示名（插件节点用完整插件列表解析名称，失效节点也能显示）
function chainLabel(item: string): string {
  return chainStageLabel(item, t, pluginsState.plugins.value, locale.value);
}

watch(
  () => props.isOpen,
  async (newVal) => {
    if (newVal) {
      await pluginsState.refresh(); // 确保 DSP 插件列表最新（未进过插件页时缓存为空）
      // 去重 + AEC 置顶/剔除 + 插件节点对齐（展开旧节点/剔除失效/补插缺失）
      localChain.value = normalizeChain(props.chain, activeDspPlugins.value, isAecSupported.value);
    }
  },
);

const draggedIndex = ref<number>(-1);

const onPointerDown = (index: number) => {
  if (localChain.value[index] === 'AEC') return; // AEC 固定首位，不可拖动
  draggedIndex.value = index;

  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', onPointerMove, { passive: false });
    window.addEventListener('pointerup', onPointerUp);
    window.addEventListener('pointercancel', onPointerUp);
  }
};

const onPointerMove = (e: PointerEvent) => {
  e.preventDefault(); // Prevent scrolling on touch devices
  if (draggedIndex.value === -1) return;

  // Find the element under the pointer (since dragged item has pointer-events-none, it pierces through)
  const el = document.elementFromPoint(e.clientX, e.clientY);
  if (!el) return;

  const row = el.closest('[data-index]');
  if (row) {
    const hoverIndex = parseInt(row.getAttribute('data-index') || '-1', 10);
    if (hoverIndex !== -1 && hoverIndex !== draggedIndex.value) {
      if (hoverIndex === 0 && localChain.value[0] === 'AEC') return; // 不允许排到 AEC 之前
      // Swap instantly
      const newChain = [...localChain.value];
      const draggedItem = newChain[draggedIndex.value];
      newChain.splice(draggedIndex.value, 1);
      newChain.splice(hoverIndex, 0, draggedItem);

      localChain.value = newChain;
      draggedIndex.value = hoverIndex;
    }
  }
};

const onPointerUp = () => {
  if (draggedIndex.value !== -1) {
    // 持久化经 update:chain → SettingsWindow 自动保存 → update_audio_settings；
    // 后端落库前会按 DSP 注册表再次对齐插件节点
    emit('update:chain', localChain.value);
  }
  draggedIndex.value = -1;

  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
  }
};

onUnmounted(() => {
  onPointerUp();
});

const close = () => {
  emit('close');
};

const resetChain = () => {
  const defaultChain = [
    'AEC',
    'NoiseReduction',
    'Dereverb',
    'Equalizer',
    'Amplifier',
    'AGC',
    'VAD',
  ];
  // 内置节点复位；启用中的 DSP 插件节点经 normalizeChain 重新插回默认位置
  localChain.value = normalizeChain(defaultChain, activeDspPlugins.value, isAecSupported.value);
  emit('update:chain', localChain.value);
};
</script>
