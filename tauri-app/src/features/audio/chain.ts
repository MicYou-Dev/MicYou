/**
 * 音频处理链共享工具（issue #347）。
 *
 * 每个启用中的 DSP 插件在处理链中占用一个独立的 `Plugin:<id>` 节点，可单独
 * 拖拽调整位置；旧的合成节点 `Plugins`（所有插件共享一个位置）会被就地展开，
 * 仅保留其翻译键作为回退。
 *
 * 节点归一化逻辑与 Rust 宿主端保持镜像，修改时请同步：
 *   - crates/micyou-audio/src/dsp.rs（PLUGIN_CHAIN_NODE / PLUGIN_NODE_PREFIX）
 *   - src-tauri/src/plugins.rs（reconcile_plugin_chain）
 */
import { marketPluginName, type LocalizablePlugin } from '@/features/plugins/market';

/** 逐插件链节点前缀（`Plugin:<plugin-id>`），与后端 PLUGIN_NODE_PREFIX 一致 */
export const PLUGIN_NODE_PREFIX = 'Plugin:';
/** 旧版合成节点（所有插件共享一个位置），与后端 PLUGIN_CHAIN_NODE 一致 */
export const LEGACY_PLUGINS_NODE = 'Plugins';

/** 链处理所需的最小插件信息（PluginView 天然满足） */
export interface ChainPlugin extends LocalizablePlugin {
  id: string;
  /** `dsp.allowMultipleNodes`: keep duplicate chain nodes for this plugin. */
  dsp?: { allowMultipleNodes?: boolean } | null;
}

export function pluginNodeId(id: string): string {
  return `${PLUGIN_NODE_PREFIX}${id}`;
}

/** 解析链节点中的插件 id；非插件节点返回 null */
export function parsePluginNodeId(item: string): string | null {
  return item.startsWith(PLUGIN_NODE_PREFIX) ? item.slice(PLUGIN_NODE_PREFIX.length) : null;
}

export function isPluginNode(item: string): boolean {
  return item.startsWith(PLUGIN_NODE_PREFIX);
}

/** 插件显示名：nameI18n 命中当前 locale（或语言前缀）时优先，回退 id */
export function pluginDisplayName(plugin: ChainPlugin, locale: string): string {
  return marketPluginName(plugin, locale) || plugin.id;
}

/**
 * 将链中的插件节点与启用中的 DSP 插件列表对齐（镜像后端 reconcile_plugin_chain）：
 *  1. 移除已停用插件的节点与重复节点；
 *  2. 旧 `Plugins` 合成节点就地展开为逐插件节点（保留用户给定的位置）；
 *  3. 为缺失节点的插件补插：最后一个插件节点之后 → AEC 之后 → 链尾。
 */
export function reconcilePluginNodes(chain: string[], activePlugins: ChainPlugin[]): string[] {
  const activeIds = activePlugins.map((p) => p.id);
  const multiIds = new Set(
    activePlugins.filter((p) => p.dsp?.allowMultipleNodes).map((p) => p.id),
  );
  const present = new Set<string>();
  const out: string[] = [];
  for (const item of chain) {
    const id = parsePluginNodeId(item);
    if (id === null) {
      out.push(item);
      continue;
    }
    if (!activeIds.includes(id)) continue;
    if (!multiIds.has(id) && present.has(id)) continue;
    present.add(id);
    out.push(item);
  }

  const legacyAt = out.indexOf(LEGACY_PLUGINS_NODE);
  if (legacyAt !== -1) {
    out.splice(legacyAt, 1);
    let expandAt = legacyAt;
    for (const id of activeIds) {
      if (present.has(id)) continue;
      present.add(id);
      out.splice(expandAt, 0, pluginNodeId(id));
      expandAt += 1;
    }
  }

  let lastPluginAt = -1;
  out.forEach((item, i) => {
    if (parsePluginNodeId(item) !== null) lastPluginAt = i;
  });
  const aecAt = out.indexOf('AEC');
  let insertAt = lastPluginAt !== -1 ? lastPluginAt + 1 : aecAt !== -1 ? aecAt + 1 : out.length;
  for (const id of activeIds) {
    if (present.has(id)) continue;
    present.add(id);
    out.splice(insertAt, 0, pluginNodeId(id));
    insertAt += 1;
  }
  return out;
}

/**
 * UI 显示/编辑用的完整链归一化：
 * 去重 + AEC 置顶（不支持的平台移除）+ 插件节点对齐。
 */
export function normalizeChain(
  chain: string[],
  activePlugins: ChainPlugin[],
  isAecSupported: boolean,
): string[] {
  const multiIds = new Set(
    activePlugins.filter((p) => p.dsp?.allowMultipleNodes).map((p) => p.id),
  );
  const seen = new Set<string>();
  const deduped = chain.filter((item) => {
    const id = parsePluginNodeId(item);
    // Multi-node plugins own several identical entries on purpose.
    if (id !== null && multiIds.has(id)) return true;
    if (seen.has(item)) return false;
    seen.add(item);
    return true;
  });
  const rest = deduped.filter((i) => i !== 'AEC');
  const base = isAecSupported ? ['AEC', ...rest] : rest;
  return reconcilePluginNodes(base, activePlugins);
}

/** 链节点显示名：插件节点显示插件名（回退 id），其余走 i18n 翻译键 */
export function chainStageLabel(
  item: string,
  t: (key: string) => string,
  plugins: ChainPlugin[],
  locale: string,
): string {
  const id = parsePluginNodeId(item);
  if (id !== null) {
    const plugin = plugins.find((p) => p.id === id);
    return plugin ? pluginDisplayName(plugin, locale) : id;
  }
  return t(`settings.audioChain.${item}`);
}
