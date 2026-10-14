<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue';
import { usePlugins } from '../composables/usePlugins';

const props = defineProps<{
  pluginId: string;
  schema: {
    fields: Array<{
      key: string;
      fieldType: string;
      label?: string | null;
      description?: string | null;
      default?: unknown;
      min?: number;
      max?: number;
      step?: number;
      options?: Array<{ value: string; label?: string | null }>;
    }>;
  };
}>();

const p = usePlugins();
const emit = defineEmits<{ saved: [] }>();
const values = reactive<Record<string, unknown>>({});
const saving = ref(false);
const saved = ref(false);

function load() {
  p.getConfig(props.pluginId)
    .then((cfg) => {
      const c = (cfg ?? {}) as Record<string, unknown>;
      for (const f of props.schema.fields) {
        values[f.key] = f.key in c ? c[f.key] : (f.default ?? '');
      }
    })
    .catch(() => {
      for (const f of props.schema.fields) {
        values[f.key] = f.default ?? '';
      }
    });
}

onMounted(load);
watch(
  () => props.pluginId,
  () => {
    saved.value = false;
    load();
  },
);

async function save() {
  saving.value = true;
  saved.value = false;
  try {
    for (const f of props.schema.fields) {
      await p.saveConfig(props.pluginId, f.key, values[f.key]);
    }
    saved.value = true;
    emit('saved');
    setTimeout(() => (saved.value = false), 2000);
  } catch {
    saved.value = false;
  } finally {
    saving.value = false;
  }
}

function label(f: { key: string; label?: string | null }) {
  return f.label || f.key;
}
</script>

<template>
  <div class="space-y-3 pt-2">
    <div v-for="f in schema.fields" :key="f.key" class="space-y-1">
      <!-- boolean -->
      <div v-if="f.fieldType === 'boolean'" class="flex items-center justify-between gap-3">
        <div>
          <div class="text-sm text-on-surface">{{ label(f) }}</div>
          <div v-if="f.description" class="text-xs text-on-surface/50">{{ f.description }}</div>
        </div>
        <button
          type="button"
          role="switch"
          :aria-checked="!!values[f.key]"
          class="shrink-0 w-10 h-6 rounded-full transition-colors"
          :class="values[f.key] ? 'bg-primary' : 'bg-surface-variant'"
          @click="values[f.key] = !values[f.key]"
        >
          <span
            class="block w-5 h-5 rounded-full bg-white shadow-sm transition-transform"
            :class="values[f.key] ? 'translate-x-[18px]' : 'translate-x-[2px]'"
          />
        </button>
      </div>
      <!-- number -->
      <div v-else-if="f.fieldType === 'number'" class="space-y-1">
        <div class="flex items-center justify-between">
          <span class="text-sm text-on-surface">{{ label(f) }}</span>
          <span class="text-sm font-mono text-primary">{{ values[f.key] }}</span>
        </div>
        <input
          v-model.number="values[f.key] as number"
          type="range"
          class="w-full accent-[hsl(var(--primary))]"
          :min="f.min ?? 0"
          :max="f.max ?? 10"
          :step="f.step ?? 0.01"
        />
        <div v-if="f.description" class="text-xs text-on-surface/50">{{ f.description }}</div>
      </div>
      <!-- select -->
      <div v-else-if="f.fieldType === 'select'" class="space-y-1">
        <div class="text-sm text-on-surface">{{ label(f) }}</div>
        <select
          v-model="values[f.key] as string"
          class="w-full px-3 py-1.5 rounded-lg bg-surface-variant text-on-surface text-sm outline-hidden"
        >
          <option v-for="o in f.options" :key="o.value" :value="o.value">
            {{ o.label || o.value }}
          </option>
        </select>
      </div>
      <!-- string -->
      <div v-else class="space-y-1">
        <div class="text-sm text-on-surface">{{ label(f) }}</div>
        <input
          v-model="values[f.key] as string"
          type="text"
          class="w-full px-3 py-1.5 rounded-lg bg-surface-variant text-on-surface text-sm outline-hidden"
        />
      </div>
    </div>

    <div class="flex items-center gap-2 pt-1">
      <button
        type="button"
        class="px-3 py-1.5 rounded-lg bg-primary text-on-primary text-sm font-medium disabled:opacity-50"
        :disabled="saving"
        @click="save"
      >
        {{ saving ? $t('plugins.saving') : $t('plugins.save') }}
      </button>
      <span v-if="saved" class="text-xs text-primary">{{ $t('plugins.configSaved') }}</span>
    </div>
  </div>
</template>
