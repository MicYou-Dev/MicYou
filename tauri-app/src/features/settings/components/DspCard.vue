<script setup lang="ts">
import MD3Switch from '@/shared/components/ui/switch/MD3Switch.vue';

const enabled = defineModel<boolean>({ required: true });

const props = defineProps<{
  title: string;
  desc?: string;
  disabled?: boolean;
}>();

const toggle = () => {
  if (!props.disabled) enabled.value = !enabled.value;
};
</script>

<template>
  <!-- A DSP stage: the header toggles it, the slot holds its parameters. -->
  <div class="bg-surface-bright rounded-2xl p-4 shadow-xs space-y-4">
    <div
      class="flex justify-between items-center"
      :class="disabled ? '' : 'cursor-pointer'"
      @click="toggle"
    >
      <div>
        <span class="font-medium text-on-surface">{{ title }}</span>
        <p v-if="desc" class="text-xs text-on-surface-variant mt-0.5">{{ desc }}</p>
        <slot name="hint" />
      </div>
      <MD3Switch v-model="enabled" :disabled="disabled" />
    </div>
    <div v-if="enabled && $slots.default" class="pt-4 border-t border-surface-variant/20">
      <slot />
    </div>
  </div>
</template>
