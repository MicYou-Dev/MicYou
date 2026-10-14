<template>
  <Transition name="dialog">
    <div v-if="show" class="fixed inset-0 z-100 flex items-center justify-center p-4">
      <div class="absolute inset-0 bg-background/70" @click="cancel"></div>

      <div class="relative w-full max-w-sm bg-surface-bright rounded-3xl overflow-hidden shadow-2xl border border-white/10 flex flex-col">
        <div class="absolute -top-32 -right-32 w-64 h-64 bg-amber-500/20 rounded-full blur-[80px] pointer-events-none"></div>
        <div class="absolute -bottom-32 -left-32 w-64 h-64 bg-primary/10 rounded-full blur-[80px] pointer-events-none"></div>

        <!-- Header Icon -->
        <div class="pt-6 pb-2 flex justify-center items-center relative z-10">
          <div class="relative">
            <div class="absolute inset-0 bg-amber-500/20 rounded-full blur-xl animate-pulse"></div>
            <div class="w-16 h-16 rounded-2xl bg-linear-to-br from-amber-500/20 to-amber-500/5 border border-amber-500/20 flex items-center justify-center shadow-inner relative z-10">
              <Headphones class="w-8 h-8 text-amber-400" stroke-width="1.75" />
            </div>
          </div>
        </div>

        <div class="px-6 pt-2 pb-2 text-center relative z-10">
          <h3 class="text-lg font-extrabold text-on-surface tracking-wide">{{ $t('dialogs.monitoringWarning.title') }}</h3>
          <p class="text-sm text-on-surface-variant mt-2 leading-relaxed">{{ $t('dialogs.monitoringWarning.desc') }}</p>
        </div>

        <div class="px-6 py-4 flex flex-col gap-2 relative z-10">
          <button
            @click="confirm"
            class="w-full py-2.5 rounded-xl bg-primary hover:bg-primary/90 text-on-primary font-semibold shadow-md transition-all hover:scale-[0.99] active:scale-95"
          >
            {{ $t('dialogs.monitoringWarning.confirm') }}
          </button>
          <button
            @click="cancel"
            class="w-full py-2.5 rounded-xl bg-surface-variant/40 hover:bg-surface-variant text-on-surface font-semibold transition-all hover:scale-[0.99] active:scale-95"
          >
            {{ $t('dialogs.cancel') }}
          </button>
        </div>

        <label class="px-6 pb-5 flex items-center gap-2 text-xs text-on-surface-variant cursor-pointer select-none">
          <button
            type="button"
            role="checkbox"
            :aria-checked="dontAskAgain"
            @click="dontAskAgain = !dontAskAgain"
            class="w-4 h-4 rounded border flex items-center justify-center transition-colors shrink-0"
            :class="dontAskAgain
              ? 'bg-primary border-primary'
              : 'bg-transparent border-outline hover:border-primary/60'"
          >
            <svg
              v-if="dontAskAgain"
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
          <span>{{ $t('dialogs.monitoringWarning.dontAskAgain') }}</span>
        </label>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { Headphones } from '@lucide/vue';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{
  (e: 'update:show', value: boolean): void;
  (e: 'confirm', dontAskAgain: boolean): void;
  (e: 'cancel'): void;
}>();

const dontAskAgain = ref(false);

watch(() => props.show, (v) => {
  if (v) dontAskAgain.value = false;
});

function cancel() {
  emit('cancel');
  emit('update:show', false);
}

function confirm() {
  emit('confirm', dontAskAgain.value);
  emit('update:show', false);
}
</script>
