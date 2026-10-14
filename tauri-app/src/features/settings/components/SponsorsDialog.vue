<template>
  <Transition name="dialog">
  <div v-if="isOpen" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs" @click.self="close">
    <div class="bg-surface rounded-3xl w-full max-w-lg shadow-xl overflow-hidden flex flex-col max-h-[80vh]">
      <!-- Header -->
      <div class="flex justify-between items-center p-6 bg-surface">
        <div>
          <h2 class="text-xl font-bold text-primary">{{ $t('dialogs.sponsors.title') }}</h2>
          <p class="text-xs text-on-surface-variant">{{ $t('dialogs.sponsors.count', { count: sponsors.length }) }}</p>
        </div>
        <button @click="close" class="p-2 rounded-full hover:bg-surface-variant/50 transition-colors">
          <X class="w-5 h-5 text-on-surface" />
        </button>
      </div>
      
      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-6 flex flex-col gap-4">
        <div class="bg-surface-variant/50 rounded-xl p-3 text-xs text-on-surface-variant">
          {{ $t('dialogs.sponsors.disclaimer') }}
        </div>

        <div v-if="isLoading" class="flex flex-col items-center justify-center py-12 gap-4">
          <div class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></div>
          <p class="text-sm text-on-surface-variant">{{ $t('dialogs.sponsors.loading') }}</p>
        </div>
        
        <div v-else-if="error" class="flex flex-col items-center justify-center py-12 gap-4 text-center">
          <span class="text-4xl">⚠️</span>
          <p class="text-sm text-error">{{ error }}</p>
        </div>
        
        <div v-else-if="sponsors.length === 0" class="flex flex-col items-center justify-center py-12 gap-4 text-center">
          <span class="text-4xl">❤️</span>
          <p class="text-sm text-on-surface-variant">{{ $t('dialogs.sponsors.empty') }}</p>
        </div>

        <div v-else class="flex flex-col gap-2 pb-16">
          <div v-for="(item, idx) in sponsors" :key="idx" class="bg-surface-bright rounded-xl p-3 flex items-center gap-4">
            <div class="w-10 h-10 rounded-full overflow-hidden bg-secondary-container border border-surface-variant">
              <img v-if="item.user && item.user.avatar" :src="item.user.avatar" class="w-full h-full object-cover" />
              <div v-else class="w-full h-full flex items-center justify-center text-sm font-bold text-on-secondary-container">
                {{ item.user?.name?.charAt(0).toUpperCase() || '?' }}
              </div>
            </div>
            
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-on-surface truncate">{{ item.user?.name || 'Anonymous' }}</p>
              <p class="text-xs text-on-surface-variant">{{ formatTime(item.first_pay_time || item.create_time) }}</p>
            </div>
            
            <div class="text-sm font-bold text-primary">
              ¥{{ item.all_sum_amount }}
            </div>
          </div>
        </div>
      </div>

      <!-- Floating Button -->
      <a href="https://afdian.com/a/LanRhyme" target="_blank" class="absolute bottom-6 right-6 w-12 h-12 bg-primary-container text-on-primary-container rounded-full flex items-center justify-center shadow-lg hover:opacity-90 transition-opacity">
        <Heart class="w-6 h-6" />
      </a>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { X, Heart } from '@lucide/vue';
import { command } from '@/platform';

const props = defineProps<{ isOpen: boolean }>();
const emit = defineEmits(['close']);

const sponsors = ref<any[]>([]);
const isLoading = ref(false);
const error = ref<string | null>(null);

const formatTime = (ts: number) => {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};

const fetchSponsors = async () => {
  if (sponsors.value.length > 0) return;
  
  isLoading.value = true;
  error.value = null;
  
  try {
    const res: string = await command('get_sponsors');
    const data = JSON.parse(res);
    if (data.ec === 200 && data.data && data.data.list) {
      const list = data.data.list;
      list.sort((a: any, b: any) => {
        const ta = a.first_pay_time > 0 ? a.first_pay_time : a.create_time;
        const tb = b.first_pay_time > 0 ? b.first_pay_time : b.create_time;
        return tb - ta;
      });
      sponsors.value = list;
    } else {
      throw new Error(data.em || `API Error: ${data.ec}`);
    }
  } catch (e: any) {
    error.value = e.message || e.toString() || 'Failed to load sponsors';
  } finally {
    isLoading.value = false;
  }
};

watch(() => props.isOpen, (newVal) => {
  if (newVal) {
    fetchSponsors();
  }
});

const close = () => {
  emit('close');
};
</script>
