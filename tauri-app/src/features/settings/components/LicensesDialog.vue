<template>
  <Transition name="dialog">
  <div v-if="isOpen" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs" @click.self="close">
    <div class="bg-surface rounded-3xl w-full max-w-lg shadow-xl overflow-hidden flex flex-col max-h-[80vh]">
      <!-- Header -->
      <div class="flex justify-between items-center p-6 bg-surface border-b border-surface-variant/20">
        <div>
          <h2 class="text-xl font-bold text-primary">{{ $t('dialogs.licenses.title') }}</h2>
          <p class="text-xs text-on-surface-variant">{{ $t('settings.about.licensesDesc') }}</p>
        </div>
        <button @click="close" class="p-2 rounded-full hover:bg-surface-variant/50 transition-colors">
          <X class="w-5 h-5 text-on-surface" />
        </button>
      </div>
      
      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-6">
        <div class="license-report" v-html="cargoLicensesHtml"></div>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { X } from '@lucide/vue';
import cargoLicensesHtml from '@/generated/third-party-licenses.html?raw';

defineProps<{ isOpen: boolean }>();
const emit = defineEmits(['close']);

const close = () => {
  emit('close');
};
</script>

<style scoped>
.license-report :deep(.license-summary) {
  margin-bottom: 1rem;
  color: hsl(var(--on-surface-variant));
  font-size: 0.75rem;
}

.license-report :deep(.license-summary h3) {
  color: hsl(var(--on-surface));
  font-size: 0.875rem;
  font-weight: 700;
}

.license-report :deep(.license-table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.75rem;
}

.license-report :deep(.license-table th),
.license-report :deep(.license-table td) {
  border-bottom: 1px solid hsl(var(--surface-variant) / 0.2);
  padding: 0.5rem 0.375rem;
  text-align: left;
  vertical-align: top;
}

.license-report :deep(a),
.license-report :deep(summary) {
  color: hsl(var(--primary));
}

.license-report :deep(.license-texts) {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-top: 1.5rem;
}

.license-report :deep(details) {
  border: 1px solid hsl(var(--surface-variant) / 0.2);
  border-radius: 0.75rem;
  padding: 0.75rem;
}

.license-report :deep(summary) {
  cursor: pointer;
  font-size: 0.75rem;
  font-weight: 600;
}

.license-report :deep(pre) {
  margin-top: 0.75rem;
  overflow-x: auto;
  white-space: pre-wrap;
  color: hsl(var(--on-surface-variant));
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.6875rem;
  line-height: 1.45;
}
</style>
