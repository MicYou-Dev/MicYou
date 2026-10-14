<template>
  <div class="flex flex-wrap gap-2.5">
    <button 
      v-for="theme in themes" 
      :key="theme.id"
      @click="selectTheme(theme.id)"
      :disabled="disabled"
      class="w-7 h-7 rounded-full flex items-center justify-center transition-all duration-200 border-[1.5px] disabled:pointer-events-none disabled:opacity-40"
      :class="modelValue === theme.id ? 'border-primary scale-110 shadow-md' : 'border-transparent hover:scale-110 shadow-xs'"
      :style="{ backgroundColor: theme.color }"
      :title="theme.name"
    >
      <Check v-if="modelValue === theme.id && theme.id !== 'theme-custom'" class="w-3.5 h-3.5 text-white drop-shadow-md" />
    </button>

    <!-- Custom Theme Button -->
    <button 
      @click="$emit('open-custom')"
      :disabled="disabled"
      class="w-7 h-7 rounded-full flex items-center justify-center transition-all duration-200 border-[1.5px] disabled:pointer-events-none disabled:opacity-40"
      :class="modelValue === 'theme-custom' ? 'border-primary scale-110 shadow-md' : 'border-transparent hover:scale-110 shadow-xs'"
      :style="customStyle"
      title="Custom Color"
    >
      <Palette v-if="modelValue !== 'theme-custom'" class="w-3.5 h-3.5 text-white drop-shadow-md opacity-90" />
      <Check v-else class="w-3.5 h-3.5 text-white drop-shadow-md" />
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { Check, Palette } from '@lucide/vue';

const props = defineProps<{
  modelValue: string;
  customH: number;
  customS: number;
  customL: number;
  disabled?: boolean;
}>();

const emit = defineEmits(['update:modelValue', 'open-custom']);

const themes = [
  { id: 'theme-blue', name: 'Blue', color: 'hsl(215, 35%, 55%)' },
  { id: 'theme-cyan', name: 'Cyan', color: 'hsl(195, 35%, 50%)' },
  { id: 'theme-teal', name: 'Teal', color: 'hsl(180, 35%, 45%)' },
  { id: 'theme-green', name: 'Green', color: 'hsl(150, 30%, 50%)' },
  { id: 'theme-amber', name: 'Amber', color: 'hsl(45, 35%, 55%)' },
  { id: 'theme-orange', name: 'Orange', color: 'hsl(25, 35%, 55%)' },
  { id: 'theme-rose', name: 'Rose', color: 'hsl(340, 35%, 55%)' },
  { id: 'theme-purple', name: 'Purple', color: 'hsl(260, 30%, 55%)' },
];

const customStyle = computed(() => {
  if (props.modelValue === 'theme-custom') {
    return { backgroundColor: `hsl(${props.customH}, ${props.customS}%, ${props.customL}%)` };
  }
  return { 
    background: 'conic-gradient(from 90deg, hsl(340, 35%, 55%), hsl(25, 35%, 55%), hsl(150, 30%, 50%), hsl(215, 35%, 55%), hsl(260, 30%, 55%), hsl(340, 35%, 55%))'
  };
});

const selectTheme = (id: string) => {
  emit('update:modelValue', id);
};
</script>
