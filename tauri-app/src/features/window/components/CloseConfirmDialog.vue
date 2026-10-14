<template>
  <Transition name="dialog">
    <div v-if="show" class="fixed inset-0 z-100 flex items-center justify-center p-4">
      <div class="absolute inset-0 bg-background/70" @click="close"></div>

      <div class="relative w-full max-w-sm bg-surface-bright rounded-3xl overflow-hidden shadow-2xl border border-white/10 flex flex-col">
        <div class="absolute -top-32 -right-32 w-64 h-64 bg-primary/20 rounded-full blur-[80px] pointer-events-none"></div>
        <div class="absolute -bottom-32 -left-32 w-64 h-64 bg-tertiary/10 rounded-full blur-[80px] pointer-events-none"></div>

        <div class="px-6 pt-6 pb-2 text-center relative z-10">
          <h3 class="text-lg font-extrabold text-on-surface tracking-wide">{{ $t('closeConfirm.title') }}</h3>
          <p class="text-sm text-on-surface-variant mt-2 leading-relaxed">{{ $t('closeConfirm.message') }}</p>
        </div>

        <div class="px-6 py-4 flex flex-col gap-2 relative z-10">
          <button
            @click="select('hide')"
            class="w-full py-2.5 rounded-xl bg-primary hover:bg-primary/90 text-on-primary font-semibold shadow-md transition-all hover:scale-[0.99] active:scale-95"
          >
            {{ $t('closeConfirm.hide') }}
          </button>
          <button
            @click="select('exit')"
            class="w-full py-2.5 rounded-xl bg-error hover:bg-error/90 text-on-error font-semibold shadow-md transition-all hover:scale-[0.99] active:scale-95"
          >
            {{ $t('closeConfirm.exit') }}
          </button>
        </div>

        <label class="px-6 pb-5 flex items-center gap-2 text-xs text-on-surface-variant cursor-pointer select-none">
          <button
            type="button"
            role="checkbox"
            :aria-checked="remember"
            @click="remember = !remember"
            class="w-4 h-4 rounded border flex items-center justify-center transition-colors shrink-0"
            :class="remember
              ? 'bg-primary border-primary'
              : 'bg-transparent border-outline hover:border-primary/60'"
          >
            <svg
              v-if="remember"
              class="w-3 h-3 text-on-primary"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <polyline points="3 8 6.5 11.5 13 4.5" />
            </svg>
          </button>
          <span>{{ $t('closeConfirm.remember') }}</span>
        </label>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{
  (e: 'update:show', value: boolean): void;
  (e: 'select', payload: { action: 'hide' | 'exit'; remember: boolean }): void;
}>();

const remember = ref(false);

watch(() => props.show, (v) => {
  if (v) remember.value = false;
});

function close() {
  emit('update:show', false);
}

function select(action: 'hide' | 'exit') {
  emit('select', { action, remember: remember.value });
  emit('update:show', false);
}
</script>
