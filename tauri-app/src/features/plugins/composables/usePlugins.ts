import { ref } from 'vue';
import {
  command,
  pickFile,
  type PluginPreview,
  type PluginSyncStatus,
  type PluginUpdate,
  type PluginView,
} from '@/platform';
import { i18n } from '@/i18n';

/** Permission preview shown before a .zip plugin is installed. */
function formatImportPreview(preview: PluginPreview): string {
  const t = i18n.global.t;
  const field = (key: string, value: string) => `${t(`plugins.importPreview.${key}`)}: ${value}`;
  const lines = [
    t('plugins.importPreview.title'),
    '',
    field('name', `${preview.name} (${preview.id})`),
    field('version', preview.version),
    ...(preview.author ? [field('author', preview.author)] : []),
    ...(preview.license ? [field('license', preview.license)] : []),
    field('runtime', preview.runtime),
    '',
    `${t('plugins.importPreview.capabilities')}:`,
    ...(preview.capabilities.length
      ? preview.capabilities.map((c) => `  · ${c}`)
      : [`  ${t('plugins.importPreview.none')}`]),
    '',
    t('plugins.marketConfirmText'),
  ];
  return lines.join('\n');
}

// 模块级单例：设置对话框与（曾经的）独立对话框共享同一份状态
const plugins = ref<PluginView[]>([]);
const syncStatus = ref<PluginSyncStatus>({ deviceConnected: false, transportReady: false });
const loading = ref(false);
const busyId = ref<string | null>(null);
const error = ref<string | null>(null);

export function usePlugins() {
  async function refresh() {
    loading.value = true;
    error.value = null;
    try {
      plugins.value = await command('list_plugins');
      syncStatus.value = await command('get_plugin_sync_status');
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function toggle(plugin: PluginView) {
    busyId.value = plugin.id;
    error.value = null;
    const stale = plugins.value.find((v) => v.id === plugin.id);
    if (stale) stale.error = null;
    try {
      await command('set_plugin_enabled', { id: plugin.id, enabled: !plugin.enabled });
      await refresh();
    } catch (e) {
      error.value = String(e);
      await refresh();
      const msg = String(e);
      const failed = plugins.value.find((v) => v.id === plugin.id);
      if (failed) failed.error = msg;
    } finally {
      busyId.value = null;
    }
  }

  async function uninstall(plugin: PluginView) {
    busyId.value = plugin.id;
    error.value = null;
    try {
      await command('uninstall_plugin', { id: plugin.id });
      await refresh();
    } catch (e) {
      error.value = String(e);
    } finally {
      busyId.value = null;
    }
  }

  async function saveConfig(plugin: PluginView | string, key: string, value: unknown) {
    try {
      const pluginId = typeof plugin === 'string' ? plugin : plugin.id;
      await command('set_plugin_config', { id: pluginId, key, value });
      return true;
    } catch (e) {
      error.value = String(e);
      return false;
    }
  }

  async function getConfig(plugin: PluginView | string): Promise<Record<string, unknown>> {
    try {
      const pluginId = typeof plugin === 'string' ? plugin : plugin.id;
      const v = await command('get_plugin_config', { id: pluginId });
      return v ?? {};
    } catch {
      return {};
    }
  }

  async function logs(plugin: PluginView): Promise<string[]> {
    try {
      return await command('get_plugin_logs', { id: plugin.id });
    } catch {
      return [];
    }
  }

  /** 触发插件 UI 动作（soundpad 按钮等）：topic ui:<action>，payload 为 JSON 字符串 */
  async function trigger(plugin: PluginView, action: string, payload?: string) {
    error.value = null;
    try {
      await command('plugin_trigger', { pluginId: plugin.id, action, payload: payload ?? null });
      return true;
    } catch (e) {
      error.value = String(e);
      return false;
    }
  }

  /** 打开系统文件管理器显示插件目录（目录由后端 open_plugins_dir 命令直接打开） */
  async function openDir(): Promise<boolean> {
    try {
      await command('open_plugins_dir');
      return true;
    } catch (e) {
      error.value = String(e);
      return false;
    }
  }

  /** 选择并导入插件压缩包（.zip） */
  // Peek a plugin zip's manifest so the UI can show a permission prompt
  // before the plugin is actually installed
  async function previewPlugin(path: string): Promise<PluginPreview> {
    return await command('preview_plugin_zip', { zipPath: path });
  }

  async function checkUpdates(): Promise<PluginUpdate[]> {
    try {
      return await command('check_plugin_updates');
    } catch {
      return [];
    }
  }

  async function updatePlugin(id: string): Promise<boolean> {
    try {
      busyId.value = id + ':update';
      error.value = null;
      await command('update_plugin', { id });
      await refresh();
      return true;
    } catch (e) {
      error.value = String(e);
      return false;
    } finally {
      busyId.value = null;
    }
  }

  async function importPlugin(): Promise<boolean> {
    try {
      const picked = await pickFile([{ name: 'MicYou plugin', extensions: ['zip'] }]);
      if (!picked) return false; // 用户取消
      return await importFromPath(picked);
    } catch (e) {
      error.value = String(e);
      return false;
    }
  }

  /** 按路径导入插件（.zip 或插件目录）。zip 先权限预览确认；目录直接导入（后端会校验 manifest）。 */
  async function importFromPath(path: string): Promise<boolean> {
    try {
      const isZip = /\.zip$/i.test(path);
      if (isZip) {
        // 权限预览：名称/作者/许可/请求的能力
        const preview: PluginPreview = await previewPlugin(path);
        const confirmed = window.confirm(formatImportPreview(preview));
        if (!confirmed) return false;
      }
      busyId.value = 'import';
      error.value = null;
      await command('import_plugin', { source: path });
      await refresh();
      return true;
    } catch (e) {
      error.value = String(e);
      return false;
    } finally {
      busyId.value = null;
    }
  }

  return {
    plugins,
    syncStatus,
    loading,
    busyId,
    error,
    refresh,
    toggle,
    uninstall,
    saveConfig,
    getConfig,
    logs,
    trigger,
    openDir,
    previewPlugin,
    importPlugin,
    importFromPath,
    checkUpdates,
    updatePlugin,
  };
}
