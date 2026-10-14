<template>
  <Transition name="dialog">
    <div v-if="show && details" class="fixed inset-0 z-100 flex items-center justify-center p-4">
      <div class="absolute inset-0 bg-background/70" @click="emit('dismiss')"></div>

      <div class="relative w-full max-w-md bg-surface-bright rounded-3xl overflow-hidden shadow-2xl border border-white/10 flex flex-col max-h-[80vh]">
        <div class="absolute -top-32 -right-32 w-64 h-64 bg-error/15 rounded-full blur-[80px] pointer-events-none"></div>
        <div class="absolute -bottom-32 -left-32 w-64 h-64 bg-tertiary/10 rounded-full blur-[80px] pointer-events-none"></div>

        <div class="px-6 pt-6 pb-2 relative z-10">
          <h3 class="text-lg font-extrabold text-on-surface tracking-wide">{{ details.title }}</h3>
        </div>

        <div class="px-6 py-2 overflow-y-auto relative z-10 flex-1 min-h-0">
          <p class="text-sm text-on-surface-variant leading-relaxed">{{ details.message }}</p>

          <div v-if="details.suggestions.length > 0" class="mt-4">
            <p class="text-xs font-bold text-primary mb-2">{{ $t('error.suggestionsTitle') }}</p>
            <p
              v-for="(s, i) in details.suggestions"
              :key="i"
              class="text-xs text-on-surface-variant pl-2 py-0.5"
            >{{ s }}</p>
          </div>

          <div
            v-if="details.type === 'UnknownError'"
            class="mt-3 p-2 rounded-lg bg-surface-variant/40"
          >
            <p class="text-xs text-on-surface-variant font-mono break-all">
              {{ $t('error.technicalDetails') }} {{ details.originalMessage }}
            </p>
          </div>
        </div>

        <div class="px-6 py-4 flex items-center justify-end gap-2 relative z-10">
          <a
            v-if="details.showHelp && details.helpUrl"
            :href="details.helpUrl"
            @click.prevent="openHelp(details.helpUrl)"
            class="px-4 py-2 rounded-xl text-sm font-medium text-primary hover:bg-primary/10 transition-colors"
          >
            {{ $t('error.dialog.help') }}
          </a>
          <button
            v-if="details.showRetry"
            @click="emit('retry')"
            class="px-4 py-2 rounded-xl bg-primary hover:bg-primary/90 text-on-primary text-sm font-semibold shadow-md transition-all hover:scale-[0.99] active:scale-95"
          >
            {{ $t('error.dialog.retry') }}
          </button>
          <button
            @click="emit('dismiss')"
            class="px-4 py-2 rounded-xl text-sm font-medium text-on-surface-variant hover:bg-surface-variant/60 transition-colors"
          >
            {{ $t('error.dialog.dismiss') }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { openUrl } from '@/platform';
import type { ConnectionErrorDetails } from '../utils/connectionError';

defineProps<{
  show: boolean;
  details: ConnectionErrorDetails | null;
}>();

const emit = defineEmits<{
  (e: 'dismiss'): void;
  (e: 'retry'): void;
}>();

const openHelp = (url: string) => {
  void openUrl(url);
};
</script>
