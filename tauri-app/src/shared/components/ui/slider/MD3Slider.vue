<script setup lang="ts">
import { SliderRange, SliderRoot, SliderThumb, SliderTrack } from "reka-ui"

defineProps<{
  modelValue: number
  min?: number
  max?: number
  step?: number
}>()

const emits = defineEmits<{
  (e: 'update:modelValue', value: number): void
}>()
</script>

<template>
  <SliderRoot
    class="group relative flex w-full touch-none select-none items-center py-4 cursor-pointer"
    :min="min"
    :max="max"
    :step="step"
    :model-value="[modelValue]"
    @update:model-value="v => { if (v && v.length > 0) emits('update:modelValue', v[0]) }"
  >
    <SliderTrack class="relative h-4 w-full grow overflow-hidden rounded-full bg-surface-variant/70 transition-colors duration-300 group-hover:bg-surface-variant">
      <SliderRange class="absolute h-full bg-primary transition-colors duration-100 ease-out" />
    </SliderTrack>
    <SliderThumb class="block focus-visible:outline-hidden relative w-1.5 h-10 border-0 outline-hidden">
      <div class="w-full h-full rounded-full bg-primary shadow-md ring-offset-background transition-[transform,background-color,box-shadow] duration-300 ease-out">
      </div>
    </SliderThumb>
  </SliderRoot>
</template>
