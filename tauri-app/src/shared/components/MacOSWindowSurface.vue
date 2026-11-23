<script setup lang="ts">
import { isMacOS } from '@/shared/lib/os';

const props = defineProps<{
  surfaceClass: string;
  elementRef: (element: HTMLElement | null) => void;
}>();
function setElement(element: unknown) {
  props.elementRef(element as HTMLElement | null);
}
</script>

<template>
  <div v-if="isMacOS" :ref="setElement" :class="surfaceClass"><slot /></div>
  <slot v-else />
</template>
