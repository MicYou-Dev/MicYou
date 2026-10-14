import { computed, ref, watch, type Ref } from 'vue';
import { command } from '@/platform';
import { usePlugins } from '@/features/plugins/composables/usePlugins';
import { marketPluginName } from '@/features/plugins/market';

export interface PluginPanelEntry {
  /** Section id: `panel:<pluginId>:<panelId>`. */
  id: string;
  name: string;
  icon?: string;
  pluginId: string;
  panelId: string;
}

/**
 * Sidebar entries for plugin panels: every enabled plugin that declares
 * ui.panels gets its own settings page (panels with sidebar: false only open
 * in their own window).
 */
export function usePluginPanels(locale: Ref<string>) {
  const { plugins, refresh } = usePlugins();
  const icons = ref<Record<string, string>>({});

  async function loadIcons() {
    const next: Record<string, string> = {};
    for (const plugin of plugins.value) {
      if (!plugin.enabled || !plugin.ui?.panels?.length) continue;
      try {
        const got = await command('get_plugin_panel_icons', { id: plugin.id });
        for (const [panelId, icon] of Object.entries(got)) {
          next[`${plugin.id}:${panelId}`] = icon;
        }
      } catch {
        /* a plugin without icons keeps the default one */
      }
    }
    icons.value = next;
  }

  watch(plugins, () => void loadIcons());

  const panels = computed<PluginPanelEntry[]>(() =>
    plugins.value.flatMap((plugin) =>
      plugin.enabled
        ? (plugin.ui?.panels ?? [])
            .filter((panel) => panel.sidebar !== false)
            .map((panel) => ({
              id: `panel:${plugin.id}:${panel.id}`,
              name: `${marketPluginName(plugin, locale.value)} · ${panel.label}`,
              icon: icons.value[`${plugin.id}:${panel.id}`],
              pluginId: plugin.id,
              panelId: panel.id,
            }))
        : [],
    ),
  );

  return { panels, refresh };
}
