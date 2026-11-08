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
 *
 * ---
 * micyou_plugin_abi.h — MicYou native plugin ABI (version 1)
 *
 * Native plugins are platform cdylibs (.so / .dylib / .dll) exposing the
 * symbols declared below. The host loads the library, negotiates the API
 * version, and hands over a host function table (mpl_host_api_t).
 *
 * ABI stability rule: this file is frozen at ABI_VERSION 1. Breaking changes
 * bump ABI_VERSION and ship a new header; the host rejects old plugins via
 * mpl_plugin_info_t.api_version.
 */
#ifndef MICYOU_PLUGIN_ABI_H
#define MICYOU_PLUGIN_ABI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32)
#define MPL_EXPORT __declspec(dllexport)
#else
#define MPL_EXPORT __attribute__((visibility("default")))
#endif

/*
 * ABI versioning rules:
 * - MPL_ABI_VERSION: incremented ONLY on binary-incompatible struct/calling
 *   convention breaks requiring full recompilation of plugins.
 * - MPL_API_VERSION: incremented on backwards-compatible API additions (new
 *   methods appended to mpl_host_api_t, new capabilities, or new event types).
 */
#define MPL_ABI_VERSION 1u
#define MPL_API_VERSION 2u

/* Result codes returned by every plugin function */
typedef enum mpl_result {
    MPL_OK = 0,
    MPL_ERR_NOT_IMPLEMENTED = 1,
    MPL_ERR_INVALID_ARG = 2,
    MPL_ERR_RUNTIME = 3,
    MPL_ERR_BUFFER_TOO_SMALL = 4,
    MPL_ERR_PERMISSION = 5
} mpl_result_t;

/* Log levels (mirror the host PluginLogLevel enum) */
typedef enum mpl_log_level {
    MPL_LOG_ERROR = 0,
    MPL_LOG_WARN = 1,
    MPL_LOG_INFO = 2,
    MPL_LOG_DEBUG = 3,
    MPL_LOG_TRACE = 4
} mpl_log_level_t;

/* Host callbacks handed to the plugin via micyou_plugin_init.
 * All string/buffer outputs follow the same contract: `out`/`out_size`
 * describe a plugin-owned buffer; on success the host writes a NUL-terminated
 * UTF-8 string and sets *out_size to the byte count (excluding NUL). If the
 * buffer is too small the host sets *out_size to the required size and
 * returns MPL_ERR_BUFFER_TOO_SMALL.
 * `ctx` is the opaque userdata pointer the host registered. */
typedef struct mpl_host_api {
    void (*log)(void *ctx, mpl_log_level_t level, const char *msg);
    mpl_result_t (*get_config)(void *ctx, const char *key, char *out, uint32_t *out_size);
    mpl_result_t (*set_config)(void *ctx, const char *key, const char *json_value);
    mpl_result_t (*emit_event)(void *ctx, const char *topic, const char *json_payload);
    /* target is a JSON object: {"type":"local|remote","pluginId":"..."} or
     * {"type":"broadcast"} */
    mpl_result_t (*send_message)(void *ctx, const char *target_json, const uint8_t *payload, uint32_t payload_len);
    /* out receives a JSON snapshot: {"streaming":bool,"sampleRate":u32,...} */
    mpl_result_t (*audio_state)(void *ctx, char *out, uint32_t *out_size);
    /* out receives a JSON array of device snapshots */
    mpl_result_t (*connected_devices)(void *ctx, char *out, uint32_t *out_size);
    void *ctx;
    /* Appended after ctx: older plugins compiled against the previous layout
     * keep working because ctx stays at its original offset. New fields are
     * only ever added here, never before ctx. */
    mpl_result_t (*play_sound)(void *ctx, const char *path);
    /* absolute path of the plugin install directory (read-only query) */
    mpl_result_t (*plugin_dir)(void *ctx, char *out, uint32_t *out_size);
    /* register a global hotkey ("ctrl+shift+p"); the plugin receives
     * handle_message with topic "hotkey:<id>" when pressed */
    mpl_result_t (*register_hotkey)(void *ctx, const char *shortcut, uint64_t *out_id);
    /* open one of the plugin's own ui.panels in an independent host window */
    mpl_result_t (*open_window)(void *ctx, const char *panel_id);
    /* Read a UTF-8 file inside the plugin install dir (requires fs.read).
       String output contract: out/out_size, returns MPL_ERR_BUFFER_TOO_SMALL
       with the required size when out_size is too small. */
    mpl_result_t (*fs_read)(void *ctx, const char *path, char *out, uint32_t *out_size);
    /* Write a UTF-8 file inside the plugin install dir (requires fs.write). */
    mpl_result_t (*fs_write)(void *ctx, const char *path, const char *content);
    /* Arm a one-shot timer; fires topic timer:expired with JSON
       {"timer":id,"payload":"..."}. Returns the id in out_id. */
    mpl_result_t (*set_timeout)(void *ctx, uint64_t ms, const char *payload, uint64_t *out_id);
    /* Cancel a timer previously returned by set_timeout. */
    mpl_result_t (*clear_timeout)(void *ctx, uint64_t id);
    /* Async HTTP request (requires network.io): returns immediately with a
       request id; the response arrives on topic http:response. */
    mpl_result_t (*http_request)(void *ctx, const char *method, const char *url,
                                 const char *headers_json, const char *body, uint64_t *out_id);
    /* Repeating timer; fires topic interval:tick every ms until cleared. */
    mpl_result_t (*set_interval)(void *ctx, uint64_t ms, const char *payload, uint64_t *out_id);
    /* Stop a repeating timer. */
    mpl_result_t (*clear_interval)(void *ctx, uint64_t id);
    /* Open a URL in the default browser (requires open.url). */
    mpl_result_t (*open_url)(void *ctx, const char *url);
    /* Show a system notification. */
    mpl_result_t (*notify)(void *ctx, const char *title, const char *body);
    /* Current host UI locale ("zh-CN", "en", ...). String output contract. */
    mpl_result_t (*locale)(void *ctx, char *out, uint32_t *out_size);
    /* Host identity + API version as JSON. String output contract. */
    mpl_result_t (*host_info)(void *ctx, char *out, uint32_t *out_size);
    /* Read clipboard text (requires clipboard.read). String output contract. */
    mpl_result_t (*clipboard_read)(void *ctx, char *out, uint32_t *out_size);
    /* Replace clipboard text (requires clipboard.write). */
    mpl_result_t (*clipboard_write)(void *ctx, const char *text);
    /* Set the settings-sidebar panel icon: plugin-dir-relative image file
     * name or short text/emoji. No capability required. */
    mpl_result_t (*set_panel_icon)(void *ctx, const char *panel_id, const char *icon);
    /* Set host mute state (0 = unmute, 1 = mute; requires control.intercept). */
    mpl_result_t (*set_muted)(void *ctx, uint32_t muted);
    /* Get host mute state (out_muted receives 0 or 1; requires control.observe). */
    mpl_result_t (*get_muted)(void *ctx, uint32_t *out_muted);
    /* Set audio monitoring / ear-return (0 = off, 1 = on; requires control.intercept). */
    mpl_result_t (*set_monitoring)(void *ctx, uint32_t enabled);
    /* Get audio monitoring / ear-return (out_enabled receives 0 or 1; requires control.observe). */
    mpl_result_t (*get_monitoring)(void *ctx, uint32_t *out_enabled);
    /* Read DSP settings as JSON string (requires control.observe). String output contract. */
    mpl_result_t (*get_dsp_settings)(void *ctx, char *out, uint32_t *out_size);
    /* Update DSP settings from JSON string (requires control.intercept). */
    mpl_result_t (*set_dsp_settings)(void *ctx, const char *settings_json);
} mpl_host_api_t;

/* Static plugin identity. The id/version must match the manifest. */
typedef struct mpl_plugin_info {
    uint32_t abi_version;   /* must equal MPL_ABI_VERSION */
    uint32_t api_version;   /* must equal MPL_API_VERSION */
    const char *id;         /* reverse-DNS id, matches plugin.json */
    const char *version;    /* semver, matches plugin.json */
} mpl_plugin_info_t;

/* ── Required entry points ──────────────────────────────────────────────── */

/* Returns a pointer to a static mpl_plugin_info_t (never NULL). */
MPL_EXPORT const mpl_plugin_info_t *micyou_plugin_info(void);

/* Called once after loading. The plugin must retain `host` for its lifetime. */
MPL_EXPORT mpl_result_t micyou_plugin_init(const mpl_host_api_t *host);

/* Called once before the library is unloaded. */
MPL_EXPORT void micyou_plugin_deinit(void);

/* ── Optional entry points (missing symbols behave as bypass/no-op) ─────── */

/* Real-time DSP: process `samples` interleaved f32 frames in place.
 * `bypass` is set to 1 when the plugin wants the host to keep the input
 * untouched this frame. Must not call blocking host APIs. */
MPL_EXPORT mpl_result_t micyou_plugin_process(float *data, uint32_t samples,
                                              uint32_t channels, double queued_ms,
                                              uint32_t *bypass);

/* Multi-node DSP extension (optional). Same contract as
 * `micyou_plugin_process`, plus the 0-based occurrence index of this
 * `Plugin:<id>` entry in `processingChain`. Hosts that keep duplicate plugin
 * nodes (manifest `dsp.allowMultipleNodes: true`) prefer this symbol. */
MPL_EXPORT mpl_result_t micyou_plugin_process_node(float *data, uint32_t samples,
                                                   uint32_t channels, double queued_ms,
                                                   uint32_t node_index,
                                                   uint32_t *bypass);

/* Local bus event; `type` is the snake_case event type, `json` its payload. */
MPL_EXPORT mpl_result_t micyou_plugin_handle_event(const char *type, const char *json);

/* Cross-device message addressed to this plugin. */
MPL_EXPORT mpl_result_t micyou_plugin_handle_message(const char *source,
                                                     const char *topic,
                                                     const uint8_t *payload,
                                                     uint32_t payload_len);

#ifdef __cplusplus
}
#endif

#endif /* MICYOU_PLUGIN_ABI_H */
