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

//! Plugin management: listing, import from a folder or zip, market
//! downloads and updates.

use super::{lock_err, PluginHost};
use crate::events::DownloadProgress;
use micyou_plugin::bus::{PluginMessage, PluginSyncTransport};
use micyou_plugin::manifest::{ConfigSchema, PluginDependency, UiDescriptor};
use micyou_plugin::PluginManifest;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const MANIFEST_FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Frontend view of one installed plugin.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub runtime: String,
    pub kind: String,
    pub platforms: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiDescriptor>,
    pub enabled: bool,
    pub loaded: bool,
    pub dsp_node: bool,
    /// Load/enable error surfaced to the user (e.g. artifact missing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "nameI18n", skip_serializing_if = "HashMap::is_empty")]
    pub name_i18n: HashMap<String, String>,
    #[serde(rename = "descriptionI18n", skip_serializing_if = "HashMap::is_empty")]
    pub description_i18n: HashMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PluginDependency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<ConfigSchema>,
}

/// Summary shown in the install confirmation dialog.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPreview {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub runtime: String,
    pub kind: String,
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

impl From<PluginManifest> for PluginPreview {
    fn from(manifest: PluginManifest) -> Self {
        Self {
            runtime: manifest.runtime.to_string(),
            kind: manifest.kind.to_string(),
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            author: manifest.author,
            description: manifest.description,
            capabilities: manifest.capabilities,
            license: manifest.license,
            homepage: manifest.homepage,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdate {
    pub id: String,
    pub current_version: String,
    pub latest_version: String,
    pub update_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSyncStatus {
    /// Whether a phone device session is connected.
    pub device_connected: bool,
    /// Plugins can currently reach the remote device.
    pub transport_ready: bool,
}

static DOWNLOAD_CANCELLATIONS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    OnceLock::new();

fn download_cancellations() -> std::sync::MutexGuard<'static, HashMap<String, Arc<AtomicBool>>> {
    DOWNLOAD_CANCELLATIONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Request cancellation of a running market download.
pub fn cancel_download(id: &str) {
    if let Some(flag) = download_cancellations().get(id) {
        flag.store(true, Ordering::SeqCst);
    }
}

fn http_client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("http client: {e}"))
}

fn fetch_manifest(client: &reqwest::blocking::Client, url: &str) -> Result<PluginManifest, String> {
    let text = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("fetch manifest: {e}"))?
        .text()
        .map_err(|e| format!("read manifest: {e}"))?;
    PluginManifest::from_json(&text).map_err(|e| format!("invalid plugin manifest: {e}"))
}

pub fn preview_zip(zip_path: &Path) -> Result<PluginPreview, String> {
    Ok(read_manifest_from_zip(zip_path)?.0.into())
}

pub fn preview_url(manifest_url: &str) -> Result<PluginPreview, String> {
    Ok(fetch_manifest(&http_client(MANIFEST_FETCH_TIMEOUT)?, manifest_url)?.into())
}

fn read_manifest_from_zip(zip_path: &Path) -> Result<(PluginManifest, PathBuf), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
    let manifest_name = archive
        .file_names()
        .find(|name| *name == "plugin.json" || name.ends_with("/plugin.json"))
        .map(str::to_string)
        .ok_or("zip contains no plugin.json")?;
    let mut text = String::new();
    archive
        .by_name(&manifest_name)
        .map_err(|e| format!("read manifest: {e}"))?
        .read_to_string(&mut text)
        .map_err(|e| format!("read manifest: {e}"))?;
    let manifest = PluginManifest::from_json(&text).map_err(|e| format!("invalid plugin: {e}"))?;
    let prefix = Path::new(&manifest_name)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    Ok((manifest, prefix))
}

/// Extract a plugin zip into `dest_root/<id>`. Entries escaping the archive
/// root are skipped (`enclosed_name`).
fn import_zip(zip_path: &Path, dest_root: &Path) -> Result<String, String> {
    let (manifest, prefix) = read_manifest_from_zip(zip_path)?;
    let dest = dest_root.join(&manifest.id);
    if dest.exists() {
        return Err(format!("plugin {} already installed", manifest.id));
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("create dir: {e}"))?;

    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
    let extracted = (|| {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("zip entry: {e}"))?;
            let Some(rel) = entry.enclosed_name() else {
                log::warn!("[plugins] skipping unsafe zip entry {}", entry.name());
                continue;
            };
            let rel = rel.strip_prefix(&prefix).unwrap_or(&rel).to_path_buf();
            let target = dest.join(&rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&target).map_err(|e| format!("mkdir: {e}"))?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
            }
            let mut out = std::fs::File::create(&target).map_err(|e| format!("create file: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract: {e}"))?;
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = extracted {
        // Never leave a half-extracted plugin that would be discovered later.
        if let Err(cleanup) = std::fs::remove_dir_all(&dest) {
            log::warn!("[plugins] failed to remove partial install {}: {cleanup}", dest.display());
        }
        return Err(e);
    }
    Ok(manifest.id)
}

fn import_dir(src: &Path, dest_root: &Path) -> Result<String, String> {
    let manifest =
        PluginManifest::load_from_dir(src).map_err(|e| format!("invalid plugin: {e}"))?;
    let dest = dest_root.join(&manifest.id);
    if dest.exists() {
        return Err(format!("plugin {} already installed", manifest.id));
    }
    copy_dir_recursive(src, &dest).map_err(|e| format!("copy failed: {e}"))?;
    Ok(manifest.id)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Stream `url` into `path`, resuming a partial file, reporting progress and
/// honouring `cancel`.
fn download_to(
    url: &str,
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64, bool),
) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let resume_from = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut request = client.get(url);
    if resume_from > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
    }
    let mut response = request
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("下载插件失败（清单可能已过期，请刷新市场后重试）：{e}"))?;

    let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut downloaded = if resumed { resume_from } else { 0 };
    let total = response.content_length().unwrap_or(0) + downloaded;
    let mut out = if resumed {
        std::fs::OpenOptions::new().append(true).open(path)
    } else {
        std::fs::File::create(path)
    }
    .map_err(|e| format!("打开临时文件失败: {e}"))?;

    let mut buffer = [0u8; 8192];
    let mut last_report = std::time::Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) {
            drop(out);
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!("[plugins] failed to remove cancelled download: {e}");
            }
            return Err("下载已取消".to_string());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|e| format!("读取插件包失败(网络中断): {e}"))?;
        if read == 0 {
            break;
        }
        out.write_all(&buffer[..read])
            .map_err(|e| format!("写入临时文件失败: {e}"))?;
        downloaded += read as u64;
        if last_report.elapsed() > PROGRESS_INTERVAL {
            progress(downloaded, total, false);
            last_report = std::time::Instant::now();
        }
    }
    progress(downloaded, total, true);
    Ok(())
}

impl PluginHost {
    fn plugins_dir(&self) -> Result<PathBuf, String> {
        let dir = self
            .manager
            .lock()
            .map_err(|_| "plugin manager lock poisoned".to_string())?
            .plugins_dir()
            .to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| format!("create plugins dir: {e}"))?;
        Ok(dir)
    }

    /// Re-sync the per-plugin chain nodes after the registry changed (#347).
    fn sync_chain(&self) {
        if let Ok(controls) = self.controls() {
            self.ensure_plugin_chain_node(&controls.dsp_settings);
        }
    }

    pub fn plugin_dir_path(&self) -> Result<PathBuf, String> {
        self.plugins_dir()
    }

    pub fn sync_status(&self) -> PluginSyncStatus {
        let connected = self.sync.is_connected();
        PluginSyncStatus {
            device_connected: connected,
            transport_ready: connected,
        }
    }

    /// All installed plugins. Enabled plugins that failed to load are
    /// retried, and the error is reported on their view.
    pub fn list(self: &Arc<Self>) -> Result<Vec<PluginView>, String> {
        let mut views: Vec<PluginView> = {
            let manager = self
                .manager
                .lock()
                .map_err(|_| "plugin manager lock poisoned".to_string())?;
            let dsp_ids = self.dsp_registry.plugin_ids();
            manager
                .entries()
                .into_iter()
                .map(|entry| {
                    let m = entry.manifest;
                    PluginView {
                        dsp_node: dsp_ids.contains(&m.id),
                        loaded: manager.is_loaded(&m.id),
                        enabled: entry.state.is_enabled(),
                        error: None,
                        runtime: m.runtime.to_string(),
                        kind: m.kind.to_string(),
                        id: m.id,
                        name: m.name,
                        name_i18n: m.name_i18n,
                        description_i18n: m.description_i18n,
                        dependencies: m.dependencies,
                        config_schema: m.config_schema,
                        version: m.version,
                        author: m.author,
                        description: m.description,
                        platforms: m.platforms,
                        capabilities: m.capabilities,
                        ui: m.ui,
                    }
                })
                .collect()
        };

        let mut retried = false;
        for view in views.iter_mut().filter(|v| v.enabled && !v.loaded) {
            retried = true;
            if let Err(e) = self.enable_plugin(&view.id) {
                view.error = Some(e.to_string());
            }
        }
        if retried {
            self.sync_chain();
        }
        Ok(views)
    }

    pub fn set_enabled(self: &Arc<Self>, id: &str, enabled: bool) -> Result<(), String> {
        let result = if enabled {
            self.enable_plugin(id)
        } else {
            self.disable_plugin(id)
        };
        self.sync_chain();
        result.map_err(|e| e.to_string())
    }

    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        let result = self.uninstall_plugin(id);
        self.sync_chain();
        result.map_err(|e| e.to_string())
    }

    pub fn config(&self, id: &str) -> Result<serde_json::Value, String> {
        let manager = self.manager.lock().map_err(|e| lock_err(e).to_string())?;
        let map = manager.plugin_config(id).map_err(|e| e.to_string())?;
        Ok(serde_json::Value::Object(map))
    }

    /// Persist one config value and tell the plugin (`config:changed`).
    pub fn set_config(&self, id: &str, key: &str, value: serde_json::Value) -> Result<(), String> {
        self.manager
            .lock()
            .map_err(|e| lock_err(e).to_string())?
            .set_plugin_config(id, key, value.clone())
            .map_err(|e| e.to_string())?;
        let payload = serde_json::json!({ "key": key, "value": value });
        let msg = PluginMessage::new("host", id, "config:changed", payload.to_string().into_bytes());
        self.bus.handle_incoming(&msg);
        Ok(())
    }

    /// HTML of a plugin UI panel.
    pub fn panel_html(&self, plugin_id: &str, panel_id: &str) -> Result<String, String> {
        let manager = self.manager.lock().map_err(|e| lock_err(e).to_string())?;
        let entry = manager
            .entry(plugin_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("unknown plugin {plugin_id}"))?;
        let panel = entry
            .manifest
            .ui
            .as_ref()
            .and_then(|ui| ui.panels.iter().find(|p| p.id == panel_id))
            .ok_or_else(|| format!("unknown panel {panel_id}"))?;
        let path = micyou_plugin::sandbox_path(&entry.dir, &panel.entry).map_err(|e| e.to_string())?;
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
    }

    /// Discover a freshly extracted plugin and try to enable it (permissions
    /// were confirmed by the user before installing).
    fn activate_installed(self: &Arc<Self>, plugins_dir: &Path, id: &str) -> Result<(), String> {
        self.manager
            .lock()
            .map_err(|e| lock_err(e).to_string())?
            .discover_plugin(plugins_dir.join(id))
            .map_err(|e| e.to_string())?;
        if let Err(e) = self.enable_plugin(id) {
            log::warn!("[plugins] auto-enable after install failed for {id}: {e}");
        }
        self.sync_chain();
        Ok(())
    }

    /// Install from a plugin folder or a .zip file.
    pub fn import(self: &Arc<Self>, source: &Path) -> Result<String, String> {
        if !source.exists() {
            return Err(format!("source not found: {}", source.display()));
        }
        let plugins_dir = self.plugins_dir()?;
        let id = if source.is_dir() {
            import_dir(source, &plugins_dir)?
        } else if source
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            import_zip(source, &plugins_dir)?
        } else {
            return Err("unsupported source: expected a directory or a .zip file".into());
        };
        self.activate_installed(&plugins_dir, &id)?;
        Ok(id)
    }

    /// Download a plugin zip from the market and install it. Blocking; run
    /// it off the async runtime.
    pub fn install_from_url(self: &Arc<Self>, id: &str, zip_url: &str) -> Result<String, String> {
        // The id names the temp file below; it comes from the market listing.
        if !micyou_plugin::manifest::validate_plugin_id(id) {
            return Err(format!("invalid plugin id {id:?}"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        download_cancellations().insert(id.to_string(), cancel.clone());
        let temp_zip = std::env::temp_dir().join(format!("micyou-market-{id}.zip"));
        let events = self.controls().map(|c| c.events.clone()).ok();
        let downloaded = download_to(zip_url, &temp_zip, &cancel, |downloaded, total, done| {
            if let Some(events) = &events {
                events.plugin_download_progress(DownloadProgress {
                    id: id.to_string(),
                    downloaded,
                    total,
                    done,
                });
            }
        });
        download_cancellations().remove(id);
        downloaded?;

        let result = (|| {
            let plugins_dir = self.plugins_dir()?;
            let installed_id = match import_zip(&temp_zip, &plugins_dir) {
                Ok(installed_id) => installed_id,
                // Idempotent: an already installed plugin counts as success.
                Err(e) if e.contains("already installed") => read_manifest_from_zip(&temp_zip)?.0.id,
                Err(e) => return Err(e),
            };
            self.activate_installed(&plugins_dir, &installed_id)?;
            Ok(installed_id)
        })();
        if let Err(e) = std::fs::remove_file(&temp_zip) {
            log::warn!("[plugins] failed to remove {}: {e}", temp_zip.display());
        }
        result
    }

    /// Installed plugins whose `updateUrl` manifest has a newer version.
    /// Blocking; run it off the async runtime.
    pub fn check_updates(&self) -> Result<Vec<PluginUpdate>, String> {
        let entries = self
            .manager
            .lock()
            .map_err(|e| lock_err(e).to_string())?
            .entries();
        let client = http_client(MANIFEST_FETCH_TIMEOUT)?;
        let updates = entries
            .into_iter()
            .filter_map(|entry| {
                let manifest = entry.manifest;
                let url = manifest.update_url?;
                let current = semver::Version::parse(&manifest.version).ok()?;
                let remote = match fetch_manifest(&client, &url) {
                    Ok(remote) => remote,
                    Err(e) => {
                        log::warn!("[plugins] update check for {} failed: {e}", manifest.id);
                        return None;
                    }
                };
                let latest = semver::Version::parse(&remote.version).ok()?;
                (latest > current).then_some(PluginUpdate {
                    id: manifest.id,
                    current_version: manifest.version,
                    latest_version: remote.version,
                    update_url: url,
                })
            })
            .collect();
        Ok(updates)
    }

    /// Replace an installed plugin with the version its `updateUrl` points
    /// at. Blocking; run it off the async runtime.
    pub fn update(self: &Arc<Self>, id: &str) -> Result<String, String> {
        let (update_url, was_enabled) = {
            let manager = self.manager.lock().map_err(|e| lock_err(e).to_string())?;
            let entry = manager
                .entry(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("unknown plugin {id}"))?;
            let url = entry
                .manifest
                .update_url
                .clone()
                .ok_or_else(|| format!("plugin {id} declares no updateUrl"))?;
            (url, entry.state.is_enabled())
        };

        let client = http_client(DOWNLOAD_TIMEOUT)?;
        let remote = fetch_manifest(&client, &update_url)?;
        if remote.id != id {
            return Err(format!("remote manifest id mismatch: {} != {id}", remote.id));
        }
        // An explicit downloadUrl wins; otherwise the zip sits next to the
        // manifest with the same stem.
        let zip_url = remote.download_url.clone().unwrap_or_else(|| {
            let stem = update_url.strip_suffix(".json").unwrap_or(&update_url);
            format!("{stem}.zip")
        });

        let tmp_zip = std::env::temp_dir().join(format!("micyou-update-{id}.zip"));
        let bytes = client
            .get(&zip_url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map_err(|e| format!("download update: {e}"))?;
        std::fs::write(&tmp_zip, &bytes).map_err(|e| format!("write temp zip: {e}"))?;

        // Validate the download before touching the installed copy.
        let (downloaded, _) = read_manifest_from_zip(&tmp_zip)?;
        if downloaded.id != id {
            return Err(format!("update archive contains {} instead of {id}", downloaded.id));
        }

        if let Err(e) = self.disable_plugin(id) {
            log::info!("[plugins] {id} was not running before update: {e}");
        }
        let plugins_dir = self.plugins_dir()?;
        let dest = plugins_dir.join(id);
        let backup = plugins_dir.join(format!(".{id}.previous"));
        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(|e| format!("remove stale backup: {e}"))?;
        }
        if dest.exists() {
            std::fs::rename(&dest, &backup).map_err(|e| format!("back up old install: {e}"))?;
        }
        let installed = import_zip(&tmp_zip, &plugins_dir);
        if let Err(e) = std::fs::remove_file(&tmp_zip) {
            log::warn!("[plugins] failed to remove {}: {e}", tmp_zip.display());
        }
        match installed {
            Ok(_) => {
                if backup.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&backup) {
                        log::warn!("[plugins] failed to remove backup {}: {e}", backup.display());
                    }
                }
            }
            Err(e) => {
                // Roll back to the previous version.
                if backup.exists() {
                    std::fs::rename(&backup, &dest)
                        .map_err(|restore| format!("install update: {e}; restore failed: {restore}"))?;
                }
                if was_enabled {
                    if let Err(restart) = self.enable_plugin(id) {
                        log::warn!("[plugins] failed to restart {id} after rollback: {restart}");
                    }
                }
                self.sync_chain();
                return Err(format!("install update: {e}"));
            }
        }

        if was_enabled {
            self.enable_plugin(id).map_err(|e| e.to_string())?;
        }
        self.sync_chain();
        Ok(remote.version)
    }
}
