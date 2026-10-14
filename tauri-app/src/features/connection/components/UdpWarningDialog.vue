<template>
  <Transition name="dialog">
    <div v-if="show" class="fixed inset-0 z-100 flex items-center justify-center p-4">
      <!-- Backdrop -->
      <div class="absolute inset-0 bg-background/70" @click="close"></div>
      
      <!-- Dialog Panel -->
      <div class="relative w-full max-w-md bg-surface-bright rounded-3xl overflow-hidden shadow-2xl border border-white/10 flex flex-col transform transition-all">
        
        <!-- Decorative Glow -->
        <div class="absolute -top-32 -right-32 w-64 h-64 bg-error/20 rounded-full blur-[80px] pointer-events-none"></div>
        <div class="absolute -bottom-32 -left-32 w-64 h-64 bg-error/10 rounded-full blur-[80px] pointer-events-none"></div>

        <!-- Header Icon -->
        <div class="pt-8 pb-4 flex justify-center items-center relative z-10">
          <div class="relative">
            <div class="absolute inset-0 bg-error/20 rounded-full blur-xl animate-pulse"></div>
            <div class="w-20 h-20 rounded-2xl bg-linear-to-br from-error/20 to-error/5 border border-error/20 flex items-center justify-center shadow-inner relative z-10">
              <ShieldAlertIcon class="w-10 h-10 text-error" stroke-width="1.5" />
            </div>
            <!-- Floating particles -->
            <div class="absolute -top-1 -right-1 w-3 h-3 rounded-full bg-error/50 blur-xs"></div>
            <div class="absolute bottom-2 -left-2 w-2 h-2 rounded-full bg-error/40 blur-xs"></div>
          </div>
        </div>

        <!-- Content -->
        <div class="px-8 pb-6 text-center space-y-4 relative z-10">
          <h3 class="text-xl font-extrabold text-on-surface tracking-wide">{{ $t('dialogs.udpWarning.title') }}</h3>
          
          <div class="bg-surface-container-highest/30 rounded-xl p-4 text-sm text-on-surface-variant text-left leading-relaxed border border-white/5 shadow-inner">
            {{ $t('dialogs.udpWarning.desc', { port: port }) }}
          </div>
        </div>

        <!-- Actions -->
        <div class="px-8 pb-8 flex flex-col gap-3 relative z-10">
          <button 
            @click="handleAllowFirewall" 
            class="w-full py-3.5 rounded-xl bg-primary hover:bg-primary/90 text-on-primary font-bold shadow-lg shadow-primary/20 hover:shadow-primary/40 transition-all duration-300 hover:scale-[0.98] active:scale-95"
          >
            {{ $t('dialogs.udpWarning.allowButton') }}
          </button>
          <button 
            @click="close" 
            class="w-full py-2.5 rounded-xl bg-surface-container hover:bg-surface-container-high text-on-surface font-medium transition-all duration-300 hover:scale-[0.98] active:scale-95 text-sm"
          >
            {{ $t('dialogs.close') }}
          </button>
        </div>
        
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ShieldAlert as ShieldAlertIcon } from '@lucide/vue'
import { command } from '@/platform';

defineProps<{
  show: boolean
  port: number
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

async function handleAllowFirewall() {
  try {
    await command('allow_firewall')
  } catch (e) {
    console.error('allow_firewall failed:', e)
  }
  emit('close')
}

const close = () => {
  emit('close')
}
</script>
