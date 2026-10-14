<script setup lang="ts">
import { ref } from 'vue'
import { Copy, Users, CheckCircle2 } from '@lucide/vue'

const props = defineProps<{
  show: boolean
  qrDataUrl: string
  webUrl: string
  clientCount: number
}>()

const emit = defineEmits<{
  dismiss: []
}>()

const copied = ref(false)

async function copyUrl() {
  try {
    await navigator.clipboard.writeText(props.webUrl)
    copied.value = true
    setTimeout(() => { copied.value = false }, 2000)
  } catch {}
}
</script>

<template>
  <Transition name="dialog">
    <div v-if="show" class="fixed inset-0 z-100 flex items-center justify-center p-4">
      <div class="absolute inset-0 bg-background/70" @click="emit('dismiss')"></div>

      <div class="relative w-full max-w-xs bg-surface-bright rounded-3xl overflow-hidden shadow-2xl border border-white/10 flex flex-col">
        <div class="absolute -top-32 -right-32 w-64 h-64 bg-primary/10 rounded-full blur-[80px] pointer-events-none"></div>
        <div class="absolute -bottom-32 -left-32 w-64 h-64 bg-tertiary/10 rounded-full blur-[80px] pointer-events-none"></div>

        <div class="px-6 pt-6 pb-2 relative z-10 flex items-center justify-between">
          <h3 class="text-lg font-extrabold text-on-surface tracking-wide">{{ $t('app.web.dialogTitle') }}</h3>
          <span v-if="clientCount > 0" class="flex items-center gap-1 text-xs text-primary font-medium">
            <Users class="w-3.5 h-3.5" />
            {{ clientCount }}
          </span>
        </div>

        <div class="px-6 py-4 relative z-10 flex flex-col items-center gap-4">
          <div v-if="qrDataUrl" class="w-48 h-48 bg-white rounded-2xl flex items-center justify-center border border-surface-variant/50 shadow-inner overflow-hidden">
            <img :src="qrDataUrl" class="w-full h-full" alt="QR Code" />
          </div>

          <p class="text-xs text-on-surface-variant text-center">{{ $t('app.web.scanToConnect') }}</p>

          <div class="w-full flex items-center gap-2 bg-surface-variant/40 rounded-xl px-3 py-2">
            <span class="text-xs text-on-surface-variant truncate flex-1 font-mono">{{ webUrl }}</span>
            <button @click="copyUrl" class="p-1 rounded-lg hover:bg-surface-variant/60 transition-colors" :title="$t('app.web.copyUrl')">
              <Copy v-if="!copied" class="w-3.5 h-3.5 text-on-surface-variant" />
              <CheckCircle2 v-else class="w-3.5 h-3.5 text-primary" />
            </button>
          </div>
        </div>

        <div class="px-6 py-4 flex items-center justify-end relative z-10">
          <button @click="emit('dismiss')" class="px-4 py-2 rounded-xl text-sm text-on-surface-variant hover:bg-surface-variant/40 transition-colors">
            {{ $t('app.web.close') }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>
