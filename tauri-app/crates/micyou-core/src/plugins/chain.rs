/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

use std::collections::HashSet;

pub const PLUGIN_NODE_AFTER: &str = "AEC";

/// Reconcile the processing chain's plugin nodes against the live DSP plugin
/// registry (issue #347: every plugin owns a dedicated `Plugin:<id>` node):
/// * per-plugin nodes of unregistered plugins (and duplicates) are removed;
///   duplicates of ids listed in `multi_node_ids` are kept;
/// * the legacy synthetic `"Plugins"` node expands in place into one node
///   per registered plugin, keeping the position the user gave it;
/// * nodes missing for registered plugins are inserted after the last
///   existing plugin node, else after [`PLUGIN_NODE_AFTER`], else appended.
///
/// `registered_ids` must be in registry execution order (see
/// `PluginDspRegistry::plugin_ids`). `multi_node_ids` lists plugins whose
/// manifest set `dsp.allowMultipleNodes`; every occurrence of those nodes is
/// kept. Pure function for testability.
pub fn reconcile_plugin_chain(
    chain: &mut Vec<String>,
    registered_ids: &[String],
    multi_node_ids: &[String],
) {
    use micyou_audio::dsp::{
        parse_plugin_chain_node, plugin_chain_node, PLUGIN_CHAIN_NODE, PLUGIN_NODE_PREFIX,
    };

    // Drop plugin nodes whose plugin is no longer registered. Duplicates are
    // removed unless the plugin opted into multiple diagnostic nodes.
    let mut present: HashSet<String> = HashSet::new();
    chain.retain(|node| {
        let Some(id) = parse_plugin_chain_node(node) else {
            return true; // built-in stage or the legacy synthetic node
        };
        if !registered_ids.iter().any(|r| r.as_str() == id) {
            return false;
        }
        let multi = multi_node_ids.iter().any(|m| m.as_str() == id);
        if multi {
            present.insert(id.to_string());
            return true;
        }
        present.insert(id.to_string())
    });

    // Expand the legacy synthetic node in place.
    if let Some(pos) = chain.iter().position(|n| n == PLUGIN_CHAIN_NODE) {
        chain.remove(pos);
        let mut insert_at = pos;
        for id in registered_ids {
            if !present.insert(id.clone()) {
                continue; // node already lives elsewhere in the chain
            }
            chain.insert(insert_at, plugin_chain_node(id));
            insert_at += 1;
        }
    }

    // Insert nodes for registered plugins that are still missing one.
    let mut insert_at = chain
        .iter()
        .rposition(|n| n.starts_with(PLUGIN_NODE_PREFIX))
        .map(|p| p + 1)
        .or_else(|| chain.iter().position(|n| n == PLUGIN_NODE_AFTER).map(|p| p + 1))
        .unwrap_or(chain.len());
    for id in registered_ids {
        if !present.insert(id.clone()) {
            continue;
        }
        chain.insert(insert_at, plugin_chain_node(id));
        insert_at += 1;
    }
}

#[cfg(test)]
mod chain_tests {
    use super::reconcile_plugin_chain;

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn legacy_node_expands_in_place() {
        let mut chain = owned(&["AEC", "Plugins", "NoiseReduction"]);
        reconcile_plugin_chain(&mut chain, &owned(&["eq", "comp"]), &[]);
        assert_eq!(
            chain,
            owned(&["AEC", "Plugin:eq", "Plugin:comp", "NoiseReduction"])
        );
    }

    #[test]
    fn missing_nodes_insert_after_last_plugin_node() {
        let mut chain = owned(&["AEC", "NoiseReduction", "Plugin:eq", "VAD"]);
        reconcile_plugin_chain(&mut chain, &owned(&["eq", "comp"]), &[]);
        assert_eq!(
            chain,
            owned(&["AEC", "NoiseReduction", "Plugin:eq", "Plugin:comp", "VAD"])
        );
    }

    #[test]
    fn missing_nodes_insert_after_aec_when_no_plugin_nodes() {
        let mut chain = owned(&["AEC", "NoiseReduction"]);
        reconcile_plugin_chain(&mut chain, &owned(&["eq"]), &[]);
        assert_eq!(chain, owned(&["AEC", "Plugin:eq", "NoiseReduction"]));

        // No AEC either → appended at the end.
        let mut chain = owned(&["NoiseReduction", "VAD"]);
        reconcile_plugin_chain(&mut chain, &owned(&["eq"]), &[]);
        assert_eq!(chain, owned(&["NoiseReduction", "VAD", "Plugin:eq"]));
    }

    #[test]
    fn stale_and_duplicate_nodes_removed() {
        let mut chain = owned(&["AEC", "Plugin:gone", "Plugin:eq", "Plugin:eq", "VAD"]);
        reconcile_plugin_chain(&mut chain, &owned(&["eq"]), &[]);
        assert_eq!(chain, owned(&["AEC", "Plugin:eq", "VAD"]));
    }

    #[test]
    fn empty_registry_clears_plugin_nodes() {
        let mut chain = owned(&["AEC", "Plugins", "Plugin:eq", "VAD"]);
        reconcile_plugin_chain(&mut chain, &[], &[]);
        assert_eq!(chain, owned(&["AEC", "VAD"]));
    }

    #[test]
    fn builtin_chain_untouched_without_plugins() {
        let mut chain = owned(&["AEC", "NoiseReduction", "Dereverb"]);
        reconcile_plugin_chain(&mut chain, &[], &[]);
        assert_eq!(chain, owned(&["AEC", "NoiseReduction", "Dereverb"]));
    }

    #[test]
    fn multi_node_plugin_keeps_every_copy() {
        let mut chain = owned(&[
            "AEC",
            "Plugin:diag",
            "Plugin:eq",
            "Plugin:diag",
            "Plugin:diag",
        ]);
        reconcile_plugin_chain(&mut chain, &owned(&["diag", "eq"]), &owned(&["diag"]));
        assert_eq!(
            chain,
            owned(&["AEC", "Plugin:diag", "Plugin:eq", "Plugin:diag", "Plugin:diag"])
        );

        // The capability is per plugin: eq still collapses to one node.
        let mut chain = owned(&["Plugin:eq", "Plugin:eq", "Plugin:diag"]);
        reconcile_plugin_chain(&mut chain, &owned(&["diag", "eq"]), &owned(&["diag"]));
        assert_eq!(chain, owned(&["Plugin:eq", "Plugin:diag"]));
    }
}
