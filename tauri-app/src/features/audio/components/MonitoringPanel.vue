<template>
  <div class="haze-surface h-full overflow-hidden">
    <div class="settings-scrollbar h-full flex flex-col overflow-y-auto p-4 space-y-4">
    <!-- Header -->
    <div class="flex items-center space-x-2">
      <ActivityIcon class="w-5 h-5 text-primary" />
      <h3 class="text-sm font-bold text-on-surface uppercase tracking-wider">{{ $t('app.monitoring.title') }}</h3>
      <div class="grow"></div>
      <div class="w-2 h-2 rounded-full" :class="statusColor"></div>
    </div>

    <!-- Real-time Metrics -->
    <div class="space-y-2.5">
      <div class="flex justify-between items-center text-sm">
        <span class="text-on-surface-variant font-medium">{{ $t('app.monitoring.rtt') }}</span>
        <span class="text-on-surface font-bold font-mono">{{ metrics?.networkLatencyMs ?? '--' }} <span class="text-xs text-on-surface-variant font-normal">ms</span></span>
      </div>
      <div class="flex justify-between items-center text-sm">
        <span class="text-on-surface-variant font-medium">{{ $t('app.monitoring.jitter') }}</span>
        <span class="text-on-surface font-bold font-mono">{{ formatDecimal(metrics?.jitterMs) }} <span class="text-xs text-on-surface-variant font-normal">ms</span></span>
      </div>
      <div class="flex justify-between items-center text-sm">
        <span class="text-on-surface-variant font-medium">{{ $t('app.monitoring.loss') }}</span>
        <span class="text-on-surface font-bold font-mono">{{ formatDecimal(metrics?.packetLossRate) }} <span class="text-xs text-on-surface-variant font-normal">%</span></span>
      </div>
    </div>

    <!-- Latency Trend -->
    <div class="space-y-2">
      <div class="flex items-center space-x-1.5 opacity-80">
        <TrendingUpIcon class="w-3.5 h-3.5 text-on-surface-variant" />
        <span class="text-xs font-medium text-on-surface-variant">{{ $t('app.monitoring.trend') }}</span>
      </div>
      <div class="h-20 bg-surface-container-highest/40 rounded-lg overflow-hidden relative">
        <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
          <polyline :points="trendPoints" fill="none" class="stroke-primary" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
    </div>

    <!-- Waveform -->
    <div class="space-y-2">
      <div class="flex items-center space-x-1.5 opacity-80">
        <WaveformIcon class="w-3.5 h-3.5 text-on-surface-variant" />
        <span class="text-xs font-medium text-on-surface-variant">{{ $t('app.monitoring.waveform') }}</span>
      </div>
      <div class="h-16 bg-surface-container-highest/40 rounded-lg overflow-hidden relative flex items-center justify-center">
        <svg class="w-full h-full" preserveAspectRatio="none" viewBox="0 0 100 100">
          <!-- Draw mirrored bars -->
          <g v-for="(val, index) in waveform" :key="index">
            <rect 
              :x="index * (100 / waveformSize)" 
              :y="50 - (val * 0.4)" 
              :width="2" 
              :height="Math.max(val * 0.8, 1)" 
              class="fill-primary" 
              rx="50"
            />
          </g>
        </svg>
      </div>
    </div>

    <!-- Audio Specs -->
    <div class="space-y-2">
      <div class="flex items-center space-x-1.5 opacity-80">
        <Settings2Icon class="w-3.5 h-3.5 text-on-surface-variant" />
        <span class="text-xs font-medium text-on-surface-variant">{{ $t('app.monitoring.specs') }}</span>
      </div>
      <div class="bg-surface-container-highest/40 rounded-lg p-3 space-y-2">
        <div class="flex justify-between items-center text-xs">
          <span class="text-on-surface-variant">{{ $t('app.monitoring.sampleRate') }}</span>
          <span class="text-on-surface font-medium">{{ metrics?.sampleRate ? `${metrics.sampleRate} Hz` : '--' }}</span>
        </div>
        <div class="flex justify-between items-center text-xs">
          <span class="text-on-surface-variant">{{ $t('app.monitoring.bitrate') }}</span>
          <span class="text-on-surface font-medium">{{ metrics?.bitrate ? `${metrics.bitrate / 1000} kbps` : '--' }}</span>
        </div>
        <div class="flex justify-between items-center text-xs">
          <span class="text-on-surface-variant">{{ $t('app.monitoring.totalLatency') }}</span>
          <span class="text-on-surface font-medium">{{ metrics?.latencyMs ? `${metrics.latencyMs} ms` : '--' }}</span>
        </div>
        <div class="flex justify-between items-center text-xs">
          <span class="text-on-surface-variant">{{ $t('app.monitoring.buffer') }}</span>
          <span class="text-on-surface font-medium">{{ metrics?.bufferDurationMs ? `${metrics.bufferDurationMs} ms` : '--' }}</span>
        </div>
      </div>
    </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { Activity as ActivityIcon, TrendingUp as TrendingUpIcon, ActivitySquare as WaveformIcon, Settings2 as Settings2Icon } from '@lucide/vue'

const props = defineProps<{
  serverState: string
  audioLevel: number
  metrics: {
    bitrate: number
    sampleRate: number
    latencyMs: number
    networkLatencyMs: number
    packetLossRate: number
    jitterMs: number
    bufferDurationMs: number
  } | null
}>()

const statusColor = computed(() => {
  if (props.serverState === 'streaming') return 'bg-tertiary shadow-[0_0_8px_rgba(var(--color-tertiary),0.6)]'
  if (props.serverState === 'connecting' || props.serverState === 'starting') return 'bg-secondary animate-pulse'
  return 'bg-outline-variant'
})

const formatDecimal = (val?: number) => {
  if (val === undefined || val === null) return '--'
  return val.toFixed(2)
}

// Latency Trend Chart
const maxTrendPoints = 30
const trendHistory = ref<number[]>([])

watch(() => props.metrics?.latencyMs, (newVal) => {
  if (newVal !== undefined && newVal !== null) {
    trendHistory.value.push(newVal)
    if (trendHistory.value.length > maxTrendPoints) {
      trendHistory.value.shift()
    }
  }
})

const trendPoints = computed(() => {
  if (trendHistory.value.length === 0) return ''
  const maxVal = Math.max(...trendHistory.value, 100)
  const minVal = 0
  const range = maxVal - minVal || 1
  
  return trendHistory.value.map((val, i) => {
    const x = (i / (maxTrendPoints - 1)) * 100
    const y = 100 - ((val - minVal) / range * 100)
    return `${x},${y}`
  }).join(' ')
})

// Waveform
const waveformSize = 33
const waveform = ref<number[]>(Array(waveformSize).fill(0))

let waveformTimer: ReturnType<typeof setInterval> | null = null
const stopWaveform = () => {
  if (waveformTimer !== null) {
    clearInterval(waveformTimer)
    waveformTimer = null
  }
}
const startWaveform = () => {
  stopWaveform()
  if (props.serverState !== 'streaming') return
  waveformTimer = setInterval(() => {
    waveform.value = [...waveform.value.slice(1), props.audioLevel]
  }, 100)
}

watch(() => props.serverState, (newVal) => {
  if (newVal === 'streaming') {
    startWaveform()
  } else {
    stopWaveform()
    waveform.value = Array(waveformSize).fill(0)
  }
})

onMounted(startWaveform)

onUnmounted(stopWaveform)
</script>
