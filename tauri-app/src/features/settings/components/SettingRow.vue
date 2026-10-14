<script setup lang="ts">
defineProps<{
  title: string;
  desc: string;
}>();
</script>

<template>
  <!-- A titled setting with its control on the right. -->
  <div
    class="bg-surface-bright/60 backdrop-blur-lg rounded-2xl p-4 flex items-center justify-between shadow-xs border border-white/5"
  >
    <div>
      <h4 class="font-bold text-on-surface">{{ title }}</h4>
      <p class="text-xs text-on-surface-variant">{{ desc }}</p>
    </div>
    <slot />
  </div>
</template>
