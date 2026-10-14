<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import {
  Link, Unlink, RefreshCw, Globe, ChevronDown, MoreHorizontal,
  VolumeX, Volume2, Headphones, Loader2, Wifi, Mic, Settings, CheckCircle2, Minus, X,
} from '@lucide/vue';
import type { ConnectionMode } from '@/features/connection/composables/useServer';
import type { NetworkInterfaceInfo } from '@/platform';

const props = defineProps<{
  serverState: string;
  connectionMode: ConnectionMode;
  serverPort: number;
  webPort: number;
  displayIp: string;
  isAutoBind: boolean;
  selectedIp: string;
  networkInterfaces: NetworkInterfaceInfo[];
  isMuted: boolean;
  isMonitoringEnabled: boolean;
  isMacOS: boolean;
}>();

const emit = defineEmits<{
  toggleStream: [];
  selectIp: [ip: string, autoSelect: boolean];
  updateMode: [mode: ConnectionMode];
  updatePort: [port: number];
  updateWebPort: [port: number];
  toggleMute: [];
  toggleMonitoringEnabled: [];
  openSettings: [];
  minimize: [];
  close: [];
}>();

// The pocket window is sized to this component, so menus expand the window
// downwards instead of opening separate popup windows.
const activePanel = ref<'ip' | 'more' | null>(null);

const togglePanel = (panel: 'ip' | 'more') => {
  activePanel.value = activePanel.value === panel ? null : panel;
};

const closePanel = () => {
  activePanel.value = null;
};

const onKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') closePanel();
};

onMounted(() => {
  window.addEventListener('blur', closePanel);
  window.addEventListener('keydown', onKeydown);
});

onUnmounted(() => {
  window.removeEventListener('blur', closePanel);
  window.removeEventListener('keydown', onKeydown);
});

const modes: { value: ConnectionMode; icon: typeof Wifi; label: string }[] = [
  { value: 'wifi', icon: Wifi, label: 'Wi-Fi' },
  { value: 'usb', icon: Mic, label: 'USB' },
  { value: 'web', icon: Globe, label: 'Web' },
];

const statusColor = computed(() => {
  switch (props.serverState) {
    case 'streaming': return 'bg-primary';
    case 'starting': return 'bg-secondary animate-pulse';
    case 'connecting': return 'bg-tertiary animate-pulse';
    default: return 'bg-on-surface-variant';
  }
});

const buttonColor = computed(() => {
  switch (props.serverState) {
    case 'streaming': return 'bg-error hover:bg-error/90';
    case 'starting': return 'bg-secondary hover:bg-secondary/90';
    case 'connecting': return 'bg-tertiary hover:bg-tertiary/90';
    default: return 'bg-primary hover:bg-primary/90';
  }
});

const selectIp = (ip: string, autoSelect: boolean) => {
  emit('selectIp', ip, autoSelect);
  closePanel();
};

const openSettings = () => {
  closePanel();
  emit('openSettings');
};

const portValue = (e: Event) => Number((e.target as HTMLInputElement).value);
</script>

<template>
  <div class="w-max flex flex-col gap-1.5">
    <div data-blur-region class="h-10 flex items-center haze-surface rounded-2xl px-3 gap-2">
      <!-- macOS uses the native window controls; keep their footprint clear -->
      <div v-if="isMacOS" class="macos-titlebar-spacer-pocket shrink-0" />

      <!-- Status Dot -->
      <div class="w-2 h-2 rounded-full shrink-0 pointer-events-none" :class="statusColor" />

      <!-- Connect Button -->
      <button
        @click="emit('toggleStream')"
        class="h-8 px-3 rounded-lg text-xs font-bold text-on-primary flex items-center gap-1.5 transition-all duration-300 hover:scale-105 active:scale-95 hover:shadow-md shrink-0"
        :class="buttonColor"
      >
        <RefreshCw v-if="serverState === 'connecting'" class="w-3.5 h-3.5 animate-spin" />
        <Loader2 v-else-if="serverState === 'starting'" class="w-3.5 h-3.5 animate-spin" />
        <Unlink v-else-if="serverState === 'streaming'" class="w-3.5 h-3.5" />
        <Link v-else class="w-3.5 h-3.5" />
        <span>{{ serverState === 'streaming' ? $t('app.status.stateStreaming') : (serverState === 'connecting' ? $t('app.status.stateConnecting') : (serverState === 'starting' ? $t('app.status.stateStarting') : $t('app.start'))) }}</span>
      </button>

      <!-- IP Display -->
      <button
        @click="togglePanel('ip')"
        class="flex items-center gap-1 px-2 py-1 rounded-md transition-all duration-300 hover:shadow-xs active:scale-95 shrink-0 max-w-[120px]"
        :class="activePanel === 'ip' ? 'bg-surface-variant/60' : 'hover:bg-surface-variant/50'"
      >
        <Globe class="w-3 h-3 text-primary shrink-0" />
        <span class="text-xs font-medium text-on-surface truncate">{{ displayIp }}</span>
        <ChevronDown class="w-3 h-3 text-on-surface-variant/50 shrink-0 transition-transform" :class="{ 'rotate-180': activePanel === 'ip' }" />
      </button>

      <div class="w-px h-4 bg-outline/20 shrink-0 pointer-events-none" />

      <!-- Mute -->
      <button
        @click="emit('toggleMute')"
        class="w-8 h-8 rounded-lg flex items-center justify-center hover:bg-surface-variant/60 transition-all duration-300 hover:scale-110 active:scale-90 shrink-0"
        :title="isMuted ? $t('app.status.unmute') : $t('app.status.mute')"
      >
        <VolumeX v-if="isMuted" class="w-4 h-4 text-error" />
        <Volume2 v-else class="w-4 h-4 text-on-surface-variant" />
      </button>

      <div class="w-px h-4 bg-outline/20 shrink-0 pointer-events-none" />

      <!-- Earback / Monitoring -->
      <button
        @click="emit('toggleMonitoringEnabled')"
        class="w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90 shrink-0"
        :class="isMonitoringEnabled ? 'bg-primary/20 text-primary' : 'hover:bg-surface-variant/60 text-on-surface-variant'"
        :title="isMonitoringEnabled ? $t('app.status.disableEarback') : $t('app.status.enableEarback')"
      >
        <Headphones class="w-4 h-4" />
      </button>

      <div class="w-px h-4 bg-outline/20 shrink-0 pointer-events-none" />

      <!-- More Menu -->
      <button
        @click="togglePanel('more')"
        class="w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-300 hover:scale-110 active:scale-90 shrink-0"
        :class="activePanel === 'more' ? 'bg-surface-variant/60' : 'hover:bg-surface-variant/50'"
      >
        <MoreHorizontal class="w-4 h-4 text-on-surface-variant" />
      </button>

      <!-- Window Controls (non-macOS: right) -->
      <template v-if="!isMacOS">
        <button @click="emit('minimize')" class="w-7 h-7 flex items-center justify-center rounded-full hover:bg-white/10 transition-colors shrink-0">
          <Minus class="w-3.5 h-3.5 text-on-surface" />
        </button>
        <button @click="emit('close')" class="w-7 h-7 flex items-center justify-center rounded-full hover:bg-error/20 hover:text-error transition-colors shrink-0">
          <X class="w-3.5 h-3.5 text-on-surface" />
        </button>
      </template>
    </div>

    <!-- w-0 min-w-full: panels follow the bar width instead of widening the window -->
    <div v-if="activePanel === 'ip'" data-blur-region class="popup-panel w-0 min-w-full max-h-60 overflow-y-auto overflow-x-hidden no-scrollbar">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 hover:bg-surface-variant/50 transition-colors text-left"
        @click="selectIp('', true)"
      >
        <Globe class="w-3.5 h-3.5 text-primary shrink-0" />
        <div class="flex-1 min-w-0 text-xs font-medium text-foreground truncate">{{ $t('app.ipSelector.allInterfaces') }}</div>
        <CheckCircle2 v-if="isAutoBind" class="w-3.5 h-3.5 text-primary shrink-0" />
      </button>
      <button
        v-for="iface in networkInterfaces"
        :key="iface.ip"
        class="w-full flex items-center gap-2 px-3 py-2 hover:bg-surface-variant/50 transition-colors text-left"
        @click="selectIp(iface.ip, false)"
      >
        <Globe class="w-3.5 h-3.5 text-on-surface-variant shrink-0" />
        <div class="flex-1 min-w-0">
          <div class="text-xs font-medium text-foreground truncate" :title="iface.ip">{{ iface.ip }}</div>
          <div class="text-[10px] text-on-surface-variant truncate">{{ iface.interface_name }}</div>
        </div>
        <CheckCircle2 v-if="!isAutoBind && selectedIp === iface.ip" class="w-3.5 h-3.5 text-primary shrink-0" />
      </button>
    </div>

    <div v-else-if="activePanel === 'more'" data-blur-region class="popup-panel w-0 min-w-full">
      <div class="px-3 py-2">
        <div class="text-[10px] text-on-surface-variant font-medium mb-1.5 uppercase tracking-wider">{{ $t('app.connectionMode') }}</div>
        <div class="flex gap-1">
          <button
            v-for="mode in modes"
            :key="mode.value"
            @click="emit('updateMode', mode.value)"
            class="flex-1 flex flex-col items-center py-1.5 rounded-lg transition-colors text-[10px] font-medium"
            :class="connectionMode === mode.value ? 'bg-primary text-on-primary' : 'bg-surface-variant/40 text-on-surface-variant hover:bg-surface-variant/60'"
          >
            <component :is="mode.icon" class="w-3.5 h-3.5 mb-0.5" />
            {{ mode.label }}
          </button>
        </div>
      </div>

      <div class="px-3 py-2 border-t border-outline/10">
        <div class="text-[10px] text-on-surface-variant font-medium mb-1.5 uppercase tracking-wider">{{ $t('app.port') }}</div>
        <input
          v-if="connectionMode !== 'web'"
          :value="serverPort"
          @input="emit('updatePort', portValue($event))"
          type="number"
          max="65534"
          class="w-full bg-surface-variant/40 border border-white/5 rounded-lg px-2.5 py-1.5 text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-primary"
        />
        <input
          v-else
          :value="webPort"
          @input="emit('updateWebPort', portValue($event))"
          type="number"
          class="w-full bg-surface-variant/40 border border-white/5 rounded-lg px-2.5 py-1.5 text-xs text-foreground focus:outline-hidden focus:ring-1 focus:ring-primary"
        />
      </div>

      <div class="border-t border-outline/10 py-1">
        <button
          @click="openSettings"
          class="w-full flex items-center gap-2.5 px-3 py-2 hover:bg-surface-variant/60 transition-colors text-left"
        >
          <Settings class="w-3.5 h-3.5 text-on-surface-variant" />
          <span class="text-xs text-on-surface">{{ $t('settings.title') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
