<template>
  <div class="flex flex-col gap-6 h-full">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h3 class="text-lg font-bold text-on-surface">{{ $t('settings.equalizer.title') }}</h3>
      </div>
      <div class="flex items-center gap-3">
        <span class="text-sm font-medium text-on-surface-variant">{{ $t('settings.equalizer.enable') }}</span>
        <MD3Switch v-model="config.enabled" />
      </div>
    </div>

    <!-- Presets & Pre-amp -->
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4" :class="{ 'opacity-50 pointer-events-none': !config.enabled }">
      
      <!-- Presets -->
      <div class="bg-surface-bright rounded-2xl p-4 shadow-xs">
        <h4 class="text-sm font-bold text-on-surface mb-3">{{ $t('settings.equalizer.presets') }}</h4>
        <div class="relative">
          <Select :model-value="selectedPreset" @update:model-value="val => { selectedPreset = val as string; applyPreset(); }">
            <SelectTrigger class="w-full bg-surface-container border-none shadow-none rounded-xl h-10 px-4 font-medium text-sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent class="border-surface-variant/20 rounded-xl bg-surface shadow-lg">
              <SelectGroup>
                <SelectItem value="Normal">{{ $t('settings.equalizer.presetNormal') }}</SelectItem>
                <SelectItem value="BrightVocal">{{ $t('settings.equalizer.presetBrightVocal') }}</SelectItem>
                <SelectItem value="DeepVoice">{{ $t('settings.equalizer.presetDeepVoice') }}</SelectItem>
                <SelectItem value="Podcast">{{ $t('settings.equalizer.presetPodcast') }}</SelectItem>
                <SelectItem value="VocalClarity">{{ $t('settings.equalizer.presetVocalClarity') }}</SelectItem>
                <SelectItem value="WarmVocal">{{ $t('settings.equalizer.presetWarmVocal') }}</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
      </div>

      <!-- Pre-amp -->
      <div class="bg-surface-bright rounded-2xl p-4 shadow-xs flex flex-col justify-center">
        <div class="flex justify-between items-center mb-2">
          <h4 class="text-sm font-bold text-on-surface">{{ $t('settings.equalizer.preAmp') }}</h4>
          <span class="text-xs font-mono font-medium text-primary">{{ config.preAmp > 0 ? '+' : '' }}{{ config.preAmp.toFixed(1) }} dB</span>
        </div>
        <MD3Slider v-model="config.preAmp" :min="-15" :max="15" :step="0.1" />
        <div class="flex justify-between text-[10px] text-on-surface-variant mt-2">
          <span>-15dB</span>
          <span>0dB</span>
          <span>+15dB</span>
        </div>
      </div>
      
    </div>

    <!-- 10-Band EQ Panel -->
    <div class="bg-surface-bright rounded-3xl p-6 shadow-xs flex-1 flex flex-col min-h-[320px] relative overflow-hidden" :class="{ 'opacity-50 pointer-events-none': !config.enabled }">
      
      <!-- Y-Axis Labels -->
      <div class="absolute left-3 top-12 bottom-12 flex flex-col justify-between text-[10px] text-on-surface-variant/60 font-mono pointer-events-none z-10">
        <span>+15</span>
        <span>0</span>
        <span>-15</span>
      </div>

      <!-- Horizontal Grid Lines -->
      <div class="absolute left-8 right-6 top-12 bottom-12 flex flex-col justify-between pointer-events-none z-0">
        <div class="w-full h-px bg-surface-variant/10"></div>
        <div class="w-full h-px bg-surface-variant/30 border-t border-dashed border-primary/20"></div>
        <div class="w-full h-px bg-surface-variant/10"></div>
      </div>

      <!-- 10 Vertical Sliders Area -->
      <div class="flex-1 flex flex-col ml-8 mr-2 relative z-10 py-6">
        
        <!-- Sliders and Curve Container -->
        <div class="flex-1 flex relative">
          <!-- Curve Shadow Visualization -->
          <div class="absolute inset-0 pointer-events-none z-0">
            <svg class="w-full h-full overflow-visible" preserveAspectRatio="none" viewBox="0 0 100 100">
              <path :d="svgPathLine" fill="none" stroke="hsl(var(--primary))" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="opacity-100" vector-effect="non-scaling-stroke" />
            </svg>
          </div>

          <!-- 10 Slider Tracks -->
          <div v-for="(band, index) in bands" :key="index" class="flex-1 flex justify-center group z-10 relative">
            <div class="relative w-full h-full flex items-center justify-center">
              <input 
                type="range" 
                v-model.number="config.gains[index]" 
                min="-15" 
                max="15" 
                step="0.1"
                @input="onSliderInput"
                class="eq-slider absolute h-full w-4 appearance-none bg-transparent cursor-pointer"
              />
              <!-- Visual Track -->
              <div class="absolute h-full w-1.5 bg-surface-variant/20 rounded-full pointer-events-none overflow-hidden group-hover:bg-surface-variant/30 transition-colors">
                 <!-- Active track fill -->
                 <div class="absolute bottom-1/2 w-full bg-primary/40" v-if="config.gains[index] > 0" :style="{ height: `${(config.gains[index] / 15) * 50}%` }"></div>
                 <div class="absolute top-1/2 w-full bg-primary/40" v-else-if="config.gains[index] < 0" :style="{ height: `${(-config.gains[index] / 15) * 50}%` }"></div>
              </div>
              <!-- Thumb Proxy (Purely visual) -->
              <div 
                class="absolute w-4 h-4 bg-surface-bright border-2 border-primary rounded-full shadow-md pointer-events-none transition-transform group-hover:scale-110"
                :style="{ bottom: `${50 + (config.gains[index] / 15) * 50}%`, transform: 'translateY(50%)' }"
              ></div>
              
              <!-- Current Value Tooltip-like label -->
              <div class="text-[9px] font-mono text-primary/70 opacity-0 group-hover:opacity-100 transition-opacity absolute -top-5 whitespace-nowrap">
                {{ config.gains[index] > 0 ? '+' : '' }}{{ config.gains[index].toFixed(1) }}
              </div>
            </div>
            
            <!-- X-Axis Label -->
            <div class="absolute -bottom-6 text-[10px] font-mono font-medium text-on-surface-variant group-hover:text-primary transition-colors text-center w-full">
              {{ formatBand(band) }}
            </div>
          </div>
        </div>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import MD3Switch from '@/shared/components/ui/switch/MD3Switch.vue';
import MD3Slider from "@/shared/components/ui/slider/MD3Slider.vue"
import { ref, computed } from 'vue';
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from '@/shared/components/ui/select';

const props = defineProps<{
  config: {
    enabled: boolean;
    preAmp: number;
    gains: number[];
  }
}>();

const bands = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

const formatBand = (freq: number) => {
  return freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
};

const presets = {
  Normal: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  BrightVocal: [-2, -1, 0, 1, 2, 3, 4, 4, 3, 2],
  DeepVoice: [3, 4, 4, 3, 1, 0, -1, -2, -2, -1],
  Podcast: [2, 3, 2, 0, -1, -1, 1, 2, 3, 2],
  VocalClarity: [-1, -1, 0, 1, 2, 3, 4, 3, 2, 1],
  WarmVocal: [1, 2, 3, 2, 1, 0, -1, -1, 0, 1]
};

const selectedPreset = ref('Normal');

const applyPreset = () => {
  const presetKey = selectedPreset.value as keyof typeof presets;
  if (presets[presetKey]) {
    props.config.gains.forEach((_, i) => {
      props.config.gains[i] = presets[presetKey][i];
    });
  }
};

const onSliderInput = () => {
  // If user changes a slider, we can optionally change preset to 'Custom'
  // But KMP doesn't seem to have a explicit Custom preset in the string resources.
  // We just leave the dropdown as is, or reset it. We'll leave it for now.
};

// --- SVG Curve Generation ---

// Generate path strings for the SVG shadow
const createSpline = (points: {x: number, y: number}[]) => {
  if (points.length === 0) return '';
  if (points.length === 1) return `M ${points[0].x},${points[0].y}`;
  
  // Create a Catmull-Rom spline approximation or simple bezier
  let d = `M ${points[0].x},${points[0].y}`;
  
  for (let i = 0; i < points.length - 1; i++) {
    const curr = points[i];
    const next = points[i + 1];
    
    const controlPointX = (curr.x + next.x) / 2;
    
    d += ` C ${controlPointX},${curr.y} ${controlPointX},${next.y} ${next.x},${next.y}`;
  }
  
  return d;
};

const svgPaths = computed(() => {
  const points = props.config.gains.map((gain, index) => {
    // x varies from 5% to 95% because there are 10 items taking 10% width each.
    // The center of item `i` is at (i + 0.5) * 10%
    const x = (index + 0.5) * 10;
    // y varies from 100% to 0% depending on gain (-15 to +15)
    // +15 = 0% (top), 0 = 50% (middle), -15 = 100% (bottom)
    const y = 50 - (gain / 15) * 50;
    return { x, y };
  });
  
  // We need absolute pixel coordinates because SVG percentages in path 'M x%, y%' aren't valid.
  // Instead, we use viewBox="0 0 100 100" and rely on preserveAspectRatio="none",
  // so the coordinates are naturally 0-100!
  
  const linePath = createSpline(points);
  
  return { line: linePath, filled: '' };
});

const svgPathLine = computed(() => svgPaths.value.line);

</script>

<style scoped>
/* Reset and style the vertical range input */
.eq-slider {
  writing-mode: vertical-lr; /* Firefox standard */
  -webkit-appearance: slider-vertical; /* Webkit standard */
  direction: rtl; /* Makes top positive and bottom negative */
  opacity: 0; /* Hide the native track/thumb, we use proxy */
  z-index: 10;
}

.eq-slider::-webkit-slider-thumb {
  width: 24px;
  height: 24px;
  cursor: pointer;
}

.eq-slider::-moz-range-thumb {
  width: 24px;
  height: 24px;
  cursor: pointer;
}
</style>
