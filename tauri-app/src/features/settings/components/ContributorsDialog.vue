<template>
  <Transition name="dialog">
  <div v-if="isOpen" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs" @click.self="close">
    <div class="bg-surface rounded-3xl w-full max-w-lg shadow-xl overflow-hidden flex flex-col max-h-[80vh]">
      <!-- Header -->
      <div class="flex justify-between items-center p-6 bg-surface">
        <div>
          <h2 class="text-xl font-bold text-primary">{{ $t('dialogs.contributors.title') }}</h2>
          <p class="text-xs text-on-surface-variant">{{ $t('dialogs.contributors.count', { count: contributors.length }) }}</p>
        </div>
        <button @click="close" class="p-2 rounded-full hover:bg-surface-variant/50 transition-colors">
          <X class="w-5 h-5 text-on-surface" />
        </button>
      </div>
      
      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-6">
        <div v-if="isLoading" class="flex flex-col items-center justify-center py-12 gap-4">
          <div class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></div>
          <p class="text-sm text-on-surface-variant">{{ $t('dialogs.contributors.loading') }}</p>
        </div>
        
        <div v-else-if="error" class="flex flex-col items-center justify-center py-12 gap-4 text-center">
          <span class="text-4xl">⚠️</span>
          <p class="text-sm text-error">{{ error }}</p>
        </div>
        
        <div v-else class="grid grid-cols-3 sm:grid-cols-4 gap-4">
          <a v-for="c in contributors" :key="c.login" :href="c.html_url" target="_blank" class="flex flex-col items-center gap-2 group">
            <div class="w-16 h-16 rounded-full overflow-hidden border-2 border-primary/20 group-hover:border-primary transition-colors bg-secondary-container">
              <img v-if="c.avatar_url" :src="c.avatar_url" class="w-full h-full object-cover" />
              <div v-else class="w-full h-full flex items-center justify-center text-lg font-bold text-on-secondary-container">
                {{ c.login.charAt(0).toUpperCase() }}
              </div>
            </div>
            <span class="text-xs font-medium text-center truncate w-full text-on-surface">{{ c.login }}</span>
          </a>
        </div>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { X } from '@lucide/vue';

const props = defineProps<{ isOpen: boolean }>();
const emit = defineEmits(['close']);

interface Contributor {
  login: string;
  avatar_url: string;
  html_url: string;
  contributions: number;
}

const contributors = ref<Contributor[]>([]);
const isLoading = ref(false);
const error = ref<string | null>(null);

const fetchContributors = async () => {
  if (contributors.value.length > 0) return;
  
  isLoading.value = true;
  error.value = null;
  
  try {
    const res = await fetch('https://api.github.com/repos/MicYou-Dev/MicYou/contributors', {
      headers: {
        'Accept': 'application/vnd.github.v3+json'
      }
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    contributors.value = data;
  } catch (e: any) {
    error.value = e.message || 'Failed to load contributors';
  } finally {
    isLoading.value = false;
  }
};

watch(() => props.isOpen, (newVal) => {
  if (newVal) {
    fetchContributors();
  }
});

const close = () => {
  emit('close');
};
</script>
