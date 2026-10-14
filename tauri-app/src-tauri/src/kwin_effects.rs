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

//! Compositor-side window effects on KDE Plasma (Wayland): behind-window
//! blur and a native drop shadow for the frameless, transparent windows.
//!
//! Blur uses the standard `ext_background_effect_manager_v1` (KWin 6.7+) and
//! falls back to `org_kde_kwin_blur_manager`; the shadow uses
//! `org_kde_kwin_shadow_manager`. GTK owns the Wayland connection and the
//! window surface; this module attaches to both as a guest and only adds its
//! own requests on a private queue.

use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, FromRawFd};
use std::sync::{Mutex, OnceLock};

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_manager_v1::{
    self, Capability, ExtBackgroundEffectManagerV1,
};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;
use wayland_protocols_plasma::shadow::client::org_kde_kwin_shadow::OrgKdeKwinShadow;
use wayland_protocols_plasma::shadow::client::org_kde_kwin_shadow_manager::OrgKdeKwinShadowManager;

use crate::window::BlurRect;

#[derive(Default)]
struct State {
    blur_capable: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            state.blur_capable = matches!(flags, WEnum::Value(f) if f.contains(Capability::Blur));
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);
delegate_noop!(State: ignore OrgKdeKwinBlurManager);
delegate_noop!(State: ignore OrgKdeKwinBlur);
delegate_noop!(State: ignore OrgKdeKwinShadowManager);
delegate_noop!(State: ignore OrgKdeKwinShadow);

enum BlurEffect {
    /// One effect object per surface; a null region turns the blur off.
    Standard(ExtBackgroundEffectSurfaceV1),
    Kwin { manager: OrgKdeKwinBlurManager, blur: Option<OrgKdeKwinBlur> },
}

struct ShadowState {
    shadow: OrgKdeKwinShadow,
    // The compositor reads the tiles from these for as long as the shadow lives.
    _pool: WlShmPool,
    _buffers: Vec<WlBuffer>,
}

/// Guest attachment to one GTK window's `wl_surface`.
struct Surface {
    conn: Connection,
    queue: EventQueue<State>,
    qh: QueueHandle<State>,
    compositor: WlCompositor,
    surface: WlSurface,
    blur: Result<BlurEffect, String>,
    shm: Option<WlShm>,
    shadow_manager: Option<OrgKdeKwinShadowManager>,
    shadow: Option<ShadowState>,
    /// Radius of the shadow currently set, to skip re-uploading the tiles.
    shadow_radius: Option<u32>,
}

impl Surface {
    fn attach(window: &gtk::ApplicationWindow) -> Result<Self, String> {
        let gdk_window = window.window().ok_or("window is not realized")?;
        let display = gdk_window.display();
        if display.type_().name() != "GdkWaylandDisplay" {
            return Err("not a Wayland session".into());
        }

        // SAFETY: both pointers come from live GDK Wayland objects owned by
        // GTK. The surface is only used while the window stays mapped (see
        // `hook_visibility`), and the display outlives the app.
        let (conn, surface) = unsafe {
            let display_ptr: *mut gtk::gdk::ffi::GdkDisplay = display.to_glib_none().0;
            let wl_display = gdk_wayland_sys::gdk_wayland_display_get_wl_display(display_ptr.cast());
            let window_ptr: *mut gtk::gdk::ffi::GdkWindow = gdk_window.to_glib_none().0;
            let wl_surface = gdk_wayland_sys::gdk_wayland_window_get_wl_surface(window_ptr.cast());
            if wl_display.is_null() || wl_surface.is_null() {
                return Err("GDK returned no Wayland handles".into());
            }
            let conn = Connection::from_backend(Backend::from_foreign_display(wl_display.cast()));
            let id = ObjectId::from_ptr(WlSurface::interface(), wl_surface.cast())
                .map_err(|e| format!("wl_surface: {e}"))?;
            let surface = WlSurface::from_id(&conn, id).map_err(|e| format!("wl_surface: {e}"))?;
            (conn, surface)
        };

        let (globals, mut queue) =
            registry_queue_init::<State>(&conn).map_err(|e| format!("registry: {e}"))?;
        let qh = queue.handle();
        let compositor = globals
            .bind::<WlCompositor, _, _>(&qh, 1..=4, ())
            .map_err(|e| format!("wl_compositor: {e}"))?;
        let blur = bind_blur(&globals, &mut queue, &surface);
        let shm = globals.bind::<WlShm, _, _>(&qh, 1..=1, ()).ok();
        let shadow_manager = globals.bind::<OrgKdeKwinShadowManager, _, _>(&qh, 1..=2, ()).ok();

        Ok(Self {
            conn,
            queue,
            qh,
            compositor,
            surface,
            blur,
            shm,
            shadow_manager,
            shadow: None,
            shadow_radius: None,
        })
    }

    fn apply_blur(&mut self, rects: &[BlurRect]) -> Result<bool, String> {
        let qh = &self.qh;
        let effect = self.blur.as_mut().map_err(|e| e.clone())?;
        let region = (!rects.is_empty()).then(|| {
            let region = self.compositor.create_region(qh, ());
            for rect in rects {
                for (x, y, w, h) in rounded_rect_spans(rect) {
                    region.add(x, y, w, h);
                }
            }
            region
        });

        match effect {
            BlurEffect::Standard(effect) => effect.set_blur_region(region.as_ref()),
            BlurEffect::Kwin { manager, blur } => match &region {
                Some(region) => {
                    let blur = blur.get_or_insert_with(|| manager.create(&self.surface, qh, ()));
                    blur.set_region(Some(region));
                    blur.commit();
                }
                None => {
                    if let Some(blur) = blur.take() {
                        blur.release();
                    }
                    manager.unset(&self.surface);
                }
            },
        }
        if let Some(region) = region {
            region.destroy();
        }
        Ok(!rects.is_empty())
    }

    fn apply_shadow(&mut self, radius: Option<u32>) -> Result<bool, String> {
        let Some(manager) = &self.shadow_manager else {
            return Err("compositor offers no org_kde_kwin_shadow".into());
        };
        if self.shadow.is_some() && self.shadow_radius == radius {
            return Ok(true);
        }
        self.shadow_radius = None;
        if let Some(old) = self.shadow.take() {
            old.shadow.destroy();
        }
        let Some(radius) = radius else {
            manager.unset(&self.surface);
            return Ok(false);
        };
        let shm = self.shm.as_ref().ok_or("compositor offers no wl_shm")?;

        let tiles = ShadowTiles::render(radius.min(64) as i32);
        let bytes: Vec<u8> = tiles.tiles.iter().flat_map(|t| t.pixels.iter().copied()).collect();
        let file = shm_file(&bytes)?;
        let pool = shm.create_pool(file.as_fd(), bytes.len() as i32, &self.qh, ());
        let mut offset = 0;
        let buffers: Vec<WlBuffer> = tiles
            .tiles
            .iter()
            .map(|t| {
                let buffer = pool.create_buffer(
                    offset,
                    t.width,
                    t.height,
                    t.width * 4,
                    wl_shm::Format::Argb8888,
                    &self.qh,
                    (),
                );
                offset += t.pixels.len() as i32;
                buffer
            })
            .collect();

        let shadow = manager.create(&self.surface, &self.qh, ());
        let [top_left, top, top_right, right, bottom_right, bottom, bottom_left, left] =
            &buffers[..]
        else {
            unreachable!("eight shadow tiles");
        };
        shadow.attach_top_left(top_left);
        shadow.attach_top(top);
        shadow.attach_top_right(top_right);
        shadow.attach_right(right);
        shadow.attach_bottom_right(bottom_right);
        shadow.attach_bottom(bottom);
        shadow.attach_bottom_left(bottom_left);
        shadow.attach_left(left);
        let extent = f64::from(SHADOW_EXTENT);
        shadow.set_left_offset(extent);
        shadow.set_top_offset(extent);
        shadow.set_right_offset(extent);
        shadow.set_bottom_offset(extent);
        shadow.commit();

        self.shadow = Some(ShadowState { shadow, _pool: pool, _buffers: buffers });
        self.shadow_radius = Some(radius);
        Ok(true)
    }

    fn flush(&mut self) -> Result<(), String> {
        // Nothing here expects events, but keep the private queue drained.
        let _ = self.queue.dispatch_pending(&mut State::default());
        self.conn.flush().map_err(|e| format!("flush: {e}"))
    }
}

fn bind_blur(
    globals: &GlobalList,
    queue: &mut EventQueue<State>,
    surface: &WlSurface,
) -> Result<BlurEffect, String> {
    let qh = queue.handle();
    if let Ok(manager) = globals.bind::<ExtBackgroundEffectManagerV1, _, _>(&qh, 1..=1, ()) {
        // Capabilities arrive right after the bind.
        let mut state = State::default();
        queue.roundtrip(&mut state).map_err(|e| format!("roundtrip: {e}"))?;
        if state.blur_capable {
            return Ok(BlurEffect::Standard(manager.get_background_effect(surface, &qh, ())));
        }
        manager.destroy();
    }
    globals
        .bind::<OrgKdeKwinBlurManager, _, _>(&qh, 1..=1, ())
        .map(|manager| BlurEffect::Kwin { manager, blur: None })
        .map_err(|_| "compositor offers no background blur".to_string())
}

fn shm_file(bytes: &[u8]) -> Result<std::fs::File, String> {
    // SAFETY: memfd_create takes a NUL-terminated name and returns a new fd
    // that the File below takes ownership of.
    let fd = unsafe { libc::memfd_create(c"micyou-shadow".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(format!("memfd_create: {}", std::io::Error::last_os_error()));
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|e| format!("shm write: {e}"))?;
    Ok(file)
}

/// Requested effects for one window, re-applied whenever GTK maps it again.
/// GTK3 destroys the `wl_surface` when a window is hidden and creates a new
/// one when it is shown, so the attachment is dropped on unmap (without
/// sending requests for the dead surface) and rebuilt on map.
#[derive(Default)]
struct WindowEffects {
    blur: Vec<BlurRect>,
    shadow_radius: Option<u32>,
    attached: Option<Surface>,
    /// The GTK window the visibility hooks are connected to. A label can
    /// outlive its window: the settings window is destroyed on close and a
    /// new one is created under the same label.
    window: Option<gtk::glib::SendWeakRef<gtk::ApplicationWindow>>,
}

/// What `sync` managed to apply.
#[derive(Default, Clone, Copy)]
struct Applied {
    blur: bool,
    shadow: bool,
}

/// Which effects a `sync` pushes to the compositor.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Blur,
    Shadow,
    All,
}

impl WindowEffects {
    fn is_for(&self, window: &gtk::ApplicationWindow) -> bool {
        self.window
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .is_some_and(|current| &current == window)
    }

    fn wanted(&self) -> bool {
        !self.blur.is_empty() || self.shadow_radius.is_some()
    }

    fn sync(
        &mut self,
        label: &str,
        window: &gtk::ApplicationWindow,
        mut scope: Scope,
    ) -> Result<Applied, String> {
        if self.attached.is_none() {
            if !self.wanted() {
                return Ok(Applied::default());
            }
            let surface = Surface::attach(window).inspect_err(|e| {
                log::info!(target: "window", "compositor effects unavailable for {label}: {e}");
            })?;
            log::info!(target: "window", "compositor effects attached to {label}");
            self.attached = Some(surface);
            scope = Scope::All;
        }
        let surface = self.attached.as_mut().expect("attached above");
        let blur = match scope {
            Scope::Shadow => Ok(false),
            _ => surface.apply_blur(&self.blur),
        };
        let shadow = match scope {
            Scope::Blur => Ok(false),
            _ => surface.apply_shadow(self.shadow_radius),
        };
        surface.flush()?;
        // Effect state is applied with the next surface commit; ask GTK for a frame.
        window.queue_draw();
        // One effect failing must not undo the other; report it and go on.
        let applied = |effect: &str, result: Result<bool, String>| {
            result
                .inspect_err(|e| log::warn!(target: "window", "{effect} for {label} failed: {e}"))
                .unwrap_or(false)
        };
        Ok(Applied { blur: applied("blur", blur), shadow: applied("shadow", shadow) })
    }
}

fn effects() -> &'static Mutex<HashMap<String, WindowEffects>> {
    static EFFECTS: OnceLock<Mutex<HashMap<String, WindowEffects>>> = OnceLock::new();
    EFFECTS.get_or_init(Default::default)
}

fn hook_visibility(label: &str, window: &gtk::ApplicationWindow) {
    // Both handlers check the window: a destroyed predecessor under the same
    // label must never touch the attachment of its replacement.
    let unmap_label = label.to_string();
    window.connect_unmap(move |window| {
        if let Ok(mut effects) = effects().lock() {
            if let Some(entry) = effects.get_mut(&unmap_label).filter(|e| e.is_for(window)) {
                entry.attached = None;
            }
        }
    });
    let map_label = label.to_string();
    window.connect_map(move |window| {
        let Ok(mut effects) = effects().lock() else { return };
        if let Some(entry) = effects.get_mut(&map_label).filter(|e| e.is_for(window)) {
            if let Err(e) = entry.sync(&map_label, window, Scope::All) {
                log::warn!(target: "window", "failed to restore effects for {map_label}: {e}");
            }
        }
    });
}

fn update(
    label: &str,
    window: &gtk::ApplicationWindow,
    scope: Scope,
    change: impl FnOnce(&mut WindowEffects),
) -> Result<Applied, String> {
    let mut effects = effects().lock().map_err(|_| "effect state poisoned".to_string())?;
    let entry = effects.entry(label.to_string()).or_default();
    if !entry.is_for(window) {
        // A new window under this label: any attachment still points at the
        // previous window's destroyed surface, and using it would be a
        // protocol error that makes the compositor drop the connection.
        // Dropping the proxies sends no requests.
        entry.attached = None;
        entry.window = Some(window.downgrade().into());
        hook_visibility(label, window);
    }
    change(entry);
    if !window.is_visible() {
        // Applied by the map handler once the window has a surface again.
        return Ok(Applied::default());
    }
    entry.sync(label, window, scope)
}

/// Sets the blurred region of a window; an empty list removes the blur.
/// Must run on the GTK main thread. Returns whether blur is in effect.
pub fn set_window_blur(
    label: &str,
    window: &gtk::ApplicationWindow,
    rects: &[BlurRect],
) -> Result<bool, String> {
    update(label, window, Scope::Blur, |e| e.blur = rects.to_vec()).map(|a| a.blur)
}

/// Sets a native drop shadow around the whole window with the given corner
/// radius, or removes it. Must run on the GTK main thread.
pub fn set_window_shadow(
    label: &str,
    window: &gtk::ApplicationWindow,
    radius: Option<u32>,
) -> Result<bool, String> {
    update(label, window, Scope::Shadow, |e| e.shadow_radius = radius).map(|a| a.shadow)
}

/// How far the shadow reaches past each window edge, in logical pixels.
const SHADOW_EXTENT: i32 = 32;
const SHADOW_OFFSET_Y: f64 = 3.0;
const SHADOW_SIGMA: f64 = 11.0;
const SHADOW_ALPHA: f64 = 0.22;

struct ShadowTile {
    width: i32,
    height: i32,
    /// ARGB8888 premultiplied, little-endian (B, G, R, A).
    pixels: Vec<u8>,
}

/// The eight KWin shadow tiles, ordered top-left, top, top-right, right,
/// bottom-right, bottom, bottom-left, left.
struct ShadowTiles {
    tiles: Vec<ShadowTile>,
}

impl ShadowTiles {
    /// Renders the shadow of a rounded window on a square canvas: corners are
    /// `extent + radius` wide so they cover the rounded window corner, edges
    /// are one pixel strips that KWin stretches along the window sides.
    fn render(radius: i32) -> Self {
        let corner = SHADOW_EXTENT + radius;
        let size = 2 * corner + 1;
        let half = f64::from(size - 2 * SHADOW_EXTENT) / 2.0;
        let center = f64::from(size) / 2.0;
        let r = f64::from(radius);

        let alpha_at = |x: i32, y: i32| -> u8 {
            let (px, py) = (f64::from(x) + 0.5 - center, f64::from(y) + 0.5 - center);
            // Nothing under the window itself, with a 1px antialiased rim.
            let outside = rounded_rect_distance(px, py, half, r).clamp(0.0, 1.0);
            let d = rounded_rect_distance(px, py - SHADOW_OFFSET_Y, half, r).max(0.0);
            let a = SHADOW_ALPHA * (-(d * d) / (2.0 * SHADOW_SIGMA * SHADOW_SIGMA)).exp();
            (a * outside * 255.0).round() as u8
        };

        let tile = |x0: i32, y0: i32, w: i32, h: i32| {
            let mut pixels = Vec::with_capacity((w * h * 4) as usize);
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    // Black, premultiplied: only alpha is non-zero.
                    pixels.extend_from_slice(&[0, 0, 0, alpha_at(x, y)]);
                }
            }
            ShadowTile { width: w, height: h, pixels }
        };

        let far = corner + 1;
        Self {
            tiles: vec![
                tile(0, 0, corner, corner),
                tile(corner, 0, 1, corner),
                tile(far, 0, corner, corner),
                tile(far, corner, corner, 1),
                tile(far, far, corner, corner),
                tile(corner, far, 1, corner),
                tile(0, far, corner, corner),
                tile(0, corner, corner, 1),
            ],
        }
    }
}

/// Signed distance from a point (relative to the center) to a square with
/// half side `half` and corner radius `r`; negative inside.
fn rounded_rect_distance(px: f64, py: f64, half: f64, r: f64) -> f64 {
    let qx = px.abs() - (half - r);
    let qy = py.abs() - (half - r);
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - r
}

/// Splits a rounded rectangle into horizontal spans so the blur follows the
/// corners instead of showing square blurred edges.
fn rounded_rect_spans(rect: &BlurRect) -> Vec<(i32, i32, i32, i32)> {
    let x = rect.x.round() as i32;
    let y = rect.y.round() as i32;
    let w = rect.width.round() as i32;
    let h = rect.height.round() as i32;
    if w <= 0 || h <= 0 {
        return Vec::new();
    }
    let r = (rect.radius.round() as i32).clamp(0, w.min(h) / 2);
    let mut spans = Vec::with_capacity(2 * r as usize + 1);
    for row in 0..r {
        let dy = r as f64 - row as f64 - 0.5;
        let inset = r - ((r * r) as f64 - dy * dy).max(0.0).sqrt().round() as i32;
        spans.push((x + inset, y + row, w - 2 * inset, 1));
        spans.push((x + inset, y + h - 1 - row, w - 2 * inset, 1));
    }
    if h > 2 * r {
        spans.push((x, y + r, w, h - 2 * r));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::{rounded_rect_spans, BlurRect, ShadowTiles, SHADOW_EXTENT};

    fn rect(width: f64, height: f64, radius: f64) -> BlurRect {
        BlurRect { x: 0.0, y: 0.0, width, height, radius }
    }

    fn alpha(tile: &super::ShadowTile, x: i32, y: i32) -> u8 {
        tile.pixels[((y * tile.width + x) * 4 + 3) as usize]
    }

    #[test]
    fn square_rect_is_a_single_span() {
        assert_eq!(rounded_rect_spans(&rect(100.0, 50.0, 0.0)), vec![(0, 0, 100, 50)]);
    }

    #[test]
    fn rounded_rect_covers_every_row_once() {
        let spans = rounded_rect_spans(&rect(100.0, 50.0, 16.0));
        let mut rows = [0; 50];
        for (_, y, _, h) in &spans {
            for row in *y..*y + *h {
                rows[row as usize] += 1;
            }
        }
        assert!(rows.iter().all(|&n| n == 1));
        // Corner rows are inset, the middle band is full width.
        assert!(spans[0].0 > 0 && spans[0].2 < 100);
        assert_eq!(spans.last(), Some(&(0, 16, 100, 18)));
    }

    #[test]
    fn radius_is_clamped_to_half_the_short_side() {
        let spans = rounded_rect_spans(&rect(40.0, 20.0, 99.0));
        assert_eq!(spans.len(), 20);
        assert!(spans.iter().all(|&(_, _, w, h)| w > 0 && h == 1));
    }

    #[test]
    fn empty_rect_has_no_spans() {
        assert!(rounded_rect_spans(&rect(0.0, 10.0, 4.0)).is_empty());
    }

    #[test]
    fn shadow_tiles_have_kwin_layout() {
        let radius = 16;
        let corner = SHADOW_EXTENT + radius;
        let tiles = ShadowTiles::render(radius).tiles;
        let sizes: Vec<(i32, i32)> = tiles.iter().map(|t| (t.width, t.height)).collect();
        assert_eq!(
            sizes,
            vec![
                (corner, corner),
                (1, corner),
                (corner, corner),
                (corner, 1),
                (corner, corner),
                (1, corner),
                (corner, corner),
                (corner, 1),
            ]
        );
        for t in &tiles {
            assert_eq!(t.pixels.len(), (t.width * t.height * 4) as usize);
        }
    }

    #[test]
    fn shadow_fades_out_and_stays_clear_under_the_window() {
        let radius = 16;
        let tiles = ShadowTiles::render(radius).tiles;
        let (top, bottom, top_left) = (&tiles[1], &tiles[5], &tiles[0]);
        let corner = SHADOW_EXTENT + radius;
        // Under the window edge (inside) nothing is drawn.
        assert_eq!(alpha(top, 0, corner - 1), 0);
        assert_eq!(alpha(top_left, corner - 1, corner - 1), 0);
        // Just outside the edge it is visible, far away it has faded.
        assert!(alpha(top, 0, SHADOW_EXTENT - 1) > 0);
        assert!(alpha(top, 0, 0) < alpha(top, 0, SHADOW_EXTENT - 1));
        // Edge tiles also cover `radius` rows under the window, which stay clear.
        assert_eq!(alpha(bottom, 0, radius - 1), 0);
        // The shadow is offset downwards, so the bottom edge is darker.
        assert!(alpha(bottom, 0, radius) > alpha(top, 0, SHADOW_EXTENT - 1));
    }
}
