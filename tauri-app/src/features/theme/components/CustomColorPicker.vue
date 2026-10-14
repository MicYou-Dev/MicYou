<template>
  <Transition name="dialog">
  <div v-if="isOpen" class="fixed inset-0 z-60 flex items-center justify-center p-4 bg-black/50 backdrop-blur-xs" @click.self="$emit('close')">
    <div class="bg-surface-bright w-full max-w-sm rounded-3xl p-6 shadow-2xl border border-surface-variant/20">
      
      <div class="flex items-center justify-between mb-6">
        <h3 class="text-xl font-bold text-on-surface">{{ $t('settings.customColor.title') }}</h3>
        <button @click="$emit('close')" class="w-8 h-8 rounded-full bg-surface-container hover:bg-surface-variant flex items-center justify-center transition-colors">
          <X class="w-4 h-4 text-on-surface" />
        </button>
      </div>

      <!-- Color Preview -->
      <div class="w-full h-24 rounded-2xl mb-6 shadow-inner flex items-center justify-center border border-surface-variant/20 transition-colors duration-200"
           :style="{ backgroundColor: `hsl(${localH}, ${localS}%, ${localL}%)` }">
        <span class="text-sm font-medium" :style="{ color: `hsl(${localH}, ${localS}%, ${localL < 50 ? 95 : 10}%)` }">
          {{ $t('settings.customColor.preview') }}
        </span>
      </div>

      <div class="space-y-6">
        <!-- Hue Slider -->
        <div class="space-y-2">
          <div class="flex justify-between items-center">
            <label class="text-sm font-medium text-on-surface">{{ $t('settings.customColor.hue') }}</label>
            <span class="text-xs text-on-surface-variant font-mono">{{ localH }}°</span>
          </div>
          <input 
            type="range" 
            min="0" :max="360" 
            v-model.number="localH"
            class="w-full h-3 rounded-full appearance-none cursor-pointer hue-slider"
          />
        </div>

        <!-- Advanced Toggle -->
        <div class="flex items-center justify-between pt-2 border-t border-surface-variant/20">
          <span class="text-sm font-medium text-on-surface">{{ $t('settings.customColor.advanced') }}</span>
          <MD3Switch v-model="advanced" />
        </div>

        <!-- Advanced Sliders -->
        <div v-if="advanced" class="space-y-4 animate-in slide-in-from-top-2 duration-200">
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <label class="text-xs font-medium text-on-surface-variant">{{ $t('settings.customColor.saturation') }}</label>
              <span class="text-xs text-on-surface-variant font-mono">{{ localS }}%</span>
            </div>
            <MD3Slider :min="0" :max="100" v-model="localS" />
          </div>
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <label class="text-xs font-medium text-on-surface-variant">{{ $t('settings.customColor.lightness') }}</label>
              <span class="text-xs text-on-surface-variant font-mono">{{ localL }}%</span>
            </div>
            <MD3Slider :min="0" :max="100" v-model="localL" />
          </div>
        </div>
        <div v-else class="text-xs text-on-surface-variant/80 text-center py-2">
          {{ $t('settings.customColor.autoManaged') }}
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="flex gap-3 mt-8">
        <button @click="$emit('close')" class="flex-1 py-2.5 rounded-xl font-medium text-sm bg-surface-container hover:bg-surface-variant text-on-surface transition-colors">
          {{ $t('dialogs.cancel') }}
        </button>
        <button @click="applyColor" class="flex-1 py-2.5 rounded-xl font-medium text-sm bg-primary text-on-primary hover:opacity-90 transition-opacity shadow-xs">
          {{ $t('dialogs.apply') }}
        </button>
      </div>

    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import MD3Switch from '@/shared/components/ui/switch/MD3Switch.vue';
import MD3Slider from "@/shared/components/ui/slider/MD3Slider.vue"
import { ref, watch } from 'vue';
import { X } from '@lucide/vue';

const props = defineProps<{
  isOpen: boolean;
  initialH: number;
  initialS?: number;
  initialL?: number;
}>();

const emit = defineEmits(['close', 'apply']);

const localH = ref(props.initialH || 215);
const localS = ref(props.initialS || 35);
const localL = ref(props.initialL || 55);
const advanced = ref(false);

watch(() => props.isOpen, (newVal) => {
  if (newVal) {
    localH.value = props.initialH || 215;
    localS.value = props.initialS || 35;
    localL.value = props.initialL || 55;
    // If it's pure auto (35/55), keep advanced off, otherwise turn it on
    if (localS.value !== 35 || localL.value !== 55) {
      advanced.value = true;
    } else {
      advanced.value = false;
    }
  }
});

watch([localH, advanced], () => {
  if (!advanced.value) {
    localS.value = 35;
    localL.value = 55;
  }
});

const applyColor = () => {
  emit('apply', { h: localH.value, s: localS.value, l: localL.value });
  emit('close');
};
</script>

<style scoped>
.hue-slider {
  background: linear-gradient(to right, 
    #ff0000 0%, 
    #ffff00 17%, 
    #00ff00 33%, 
    #00ffff 50%, 
    #0000ff 67%, 
    #ff00ff 83%, 
    #ff0000 100%
  );
}
.hue-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: white;
  border: 2px solid #ccc;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
  cursor: pointer;
}
.hue-slider::-moz-range-thumb {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: white;
  border: 2px solid #ccc;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
  cursor: pointer;
}
</style>
