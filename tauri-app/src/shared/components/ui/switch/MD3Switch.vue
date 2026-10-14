<script setup lang="ts">
const model = defineModel<boolean>({ required: true });

defineProps<{
  disabled?: boolean;
}>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="model"
    :disabled="disabled"
    class="group relative inline-flex h-8 w-14 shrink-0 cursor-pointer items-center rounded-full border-2 transition-colors duration-300 ease-out focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 active:scale-95 disabled:cursor-not-allowed disabled:opacity-40"
    :class="
      model
        ? 'border-primary bg-primary'
        : 'border-on-surface-variant bg-transparent hover:bg-on-surface-variant/10'
    "
    @click.stop="model = !model"
  >
    <div
      class="relative flex items-center justify-center transition-transform duration-300 ease-out"
      :class="model ? 'translate-x-[26px]' : 'translate-x-[4px]'"
    >
      <span
        class="pointer-events-none block rounded-full shadow-xs ring-0 transition-all duration-300 ease-out"
        :class="
          model
            ? 'h-6 w-6 bg-on-primary'
            : 'h-4 w-4 bg-on-surface-variant group-hover:h-5 group-hover:w-5'
        "
      />
    </div>
  </button>
</template>
