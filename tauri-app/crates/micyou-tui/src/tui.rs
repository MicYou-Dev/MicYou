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

use crate::events::Event;
use crate::i18n;
use crate::theme::{self, Rgba, Theme};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind as K,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use micyou_core::micyou_audio::dsp::AudioDspSettings;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline};
use ratatui::Frame;
use std::collections::VecDeque;
use std::io::stdout;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use micyou_core::config::ServerPrefs;
use micyou_core::server::ServerState;
use micyou_core::stats::AudioMetrics;
use micyou_core::transport::tcp::DeviceInfo;

/// Vertical split of the whole screen: [title, tabs, body, footer].
fn split_layout(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area)
}

pub struct TuiApp {
    pub tab: usize,
    pub port: u16,
    pub mode: String,
    pub device: Option<DeviceInfo>,
    pub metrics: Option<AudioMetrics>,
    pub level: u32,
    pub muted: bool,
    pub web_clients: u32,
    pub settings: AudioDspSettings,
    /// Runtime capability is transient and must never overwrite the persisted
    /// AEC preference in `settings`.
    pub aec_runtime_available: bool,
    pub selected_setting: usize,
    pub chain_index: usize,
    pub logs: VecDeque<String>,
    pub last_event: String,
    /// Scroll offset for the logs page (0 = newest visible).
    pub log_offset: usize,
    /// Processed spectrum (64 bands, 0..1) for the cava-style visualizer.
    pub spectrum: Vec<f32>,
    /// Rolling input-level history for the sparkline (0..=100).
    pub level_history: VecDeque<u64>,
    /// Rolling network-latency history (ms) for the sparkline.
    pub latency_history: VecDeque<u64>,
    pub lang: String,
    pub theme: Theme,
    /// Local IPs (sorted, virtual adapters filtered) shown as connect hints.
    pub ips: Vec<String>,
    /// Shared connection settings (server.json), editable on the Connection tab.
    pub prefs: ServerPrefs,
    /// Selected row on the Connection tab.
    pub selected_conn: usize,
    /// System resource monitor (CPU/memory sampling).
    sys: System,
    last_sample: Instant,
    /// System CPU usage in percent (0..100).
    pub cpu_usage: f32,
    /// System memory usage (used / total bytes).
    pub mem_used: u64,
    pub mem_total: u64,
    /// This process's CPU usage in percent and memory footprint in bytes.
    pub proc_cpu: f32,
    pub proc_mem: u64,
}

impl TuiApp {
    pub fn new(settings: AudioDspSettings, port: u16, mode: String) -> Self {
        let lang = i18n::detect_lang();
        let ips = micyou_core::discovery::query_network_interfaces()
            .into_iter()
            .map(|i| i.ip)
            .filter(|ip| !ip.is_empty())
            .take(4)
            .collect();
        Self {
            tab: 0,
            port,
            mode,
            device: None,
            metrics: None,
            level: 0,
            muted: false,
            web_clients: 0,
            // AEC needs a far-end reference, so only offer the toggle where the
            // platform can supply one.
            aec_runtime_available: micyou_core::micyou_audio::aec_reference_availability()
                .available,
            settings,
            selected_setting: 0,
            chain_index: 0,
            logs: VecDeque::from(["[inf] tui started".to_string()]),
            last_event: String::new(),
            log_offset: 0,
            spectrum: vec![0.0; 64],
            level_history: VecDeque::new(),
            latency_history: VecDeque::new(),
            lang,
            theme: theme::load(),
            ips,
            prefs: micyou_core::config::load_server_prefs(),
            selected_conn: 0,
            sys: System::new(),
            last_sample: Instant::now(),
            cpu_usage: 0.0,
            mem_used: 0,
            mem_total: 0,
            proc_cpu: 0.0,
            proc_mem: 0,
        }
    }

    /// Sample system + process CPU/memory usage at most once per second.
    /// Sample system + process CPU/memory usage every 2s.
    /// Returns true when a value changed so the caller knows a redraw is needed.
    fn sample_system(&mut self) -> bool {
        if self.last_sample.elapsed().as_millis() < 2000 {
            return false;
        }
        self.last_sample = Instant::now();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let cpu = self.sys.global_cpu_usage();
        let mem_u = self.sys.used_memory();
        let mem_t = self.sys.total_memory().max(1);
        let pid = Pid::from_u32(std::process::id());
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let (p_cpu, p_mem) = match self.sys.process(pid) {
            Some(p) => (p.cpu_usage(), p.memory()),
            None => (0.0, 0),
        };
        let changed = (cpu - self.cpu_usage).abs() > 0.5
            || mem_u != self.mem_used
            || (p_cpu - self.proc_cpu).abs() > 0.5
            || p_mem != self.proc_mem;
        self.cpu_usage = cpu;
        self.mem_used = mem_u;
        self.mem_total = mem_t;
        self.proc_cpu = p_cpu;
        self.proc_mem = p_mem;
        changed
    }

    fn t(&self, key: &str) -> String {
        i18n::tr(&self.lang, key)
    }

    pub fn on_event(&mut self, ev: Event) {
        match ev {
            Event::DeviceConnected(info) => {
                let name = info.name.clone();
                self.device = Some(info);
                self.log(format!("[ok] {}: {name}", self.t("connected")));
            }
            Event::DeviceDisconnected => {
                self.device = None;
                self.log("[warn] mic disconnected".to_string());
            }
            Event::Metrics(m) => {
                let latency = m.latency_ms.max(0) as u64;
                self.metrics = Some(m);
                self.latency_history.push_back(latency);
                while self.latency_history.len() > 120 {
                    self.latency_history.pop_front();
                }
            }
            Event::UdpWarning => self.log("[warn] UDP audio stalled".to_string()),
            Event::MuteChanged(muted) => {
                self.muted = muted;
                self.log(format!("[inf] muted: {muted}"));
            }
            Event::Level(level) => {
                self.level = level;
                self.level_history.push_back(level.min(100) as u64);
                while self.level_history.len() > 240 {
                    self.level_history.pop_front();
                }
            }
            Event::Spectrum(_raw, processed) => {
                if processed.len() >= 64 {
                    self.spectrum.clone_from(&processed);
                }
            }
            Event::Stopped => {
                self.log("[err] server stopped".to_string());
            }
            Event::WebClientCount(count) => {
                self.web_clients = count;
                self.log(format!("[inf] web clients: {count}"));
            }
            Event::InstallProgress(msg) => self.log(format!("[inf] install: {msg}")),
            Event::AecStatus(status) => {
                self.aec_runtime_available = status.available;
                if status.available && status.enabled {
                    self.log("[ok] AEC enabled".to_string());
                } else if let Some(reason) = status.reason {
                    self.log(format!("[warn] AEC disabled: {reason}"));
                }
            }
            Event::Log(line) => self.log(line),
        }
    }

    fn log(&mut self, line: String) {
        self.last_event.clone_from(&line);
        self.logs.push_back(line);
        while self.logs.len() > 500 {
            self.logs.pop_front();
        }
    }

    fn tabs(&self) -> Vec<String> {
        vec![
            self.t("tab_dashboard"),
            self.t("tab_audio"),
            self.t("tab_chain"),
            self.t("tab_conn"),
            self.t("tab_logs"),
        ]
    }

    fn mode_label(&self) -> String {
        match self.mode.as_str() {
            "wifi" => self.t("mode_wifi"),
            "usb" => self.t("mode_usb"),
            "web" => self.t("mode_web"),
            _ => self.t("mode_unknown"),
        }
    }

    /// Hit-test the tab bar row: returns the tab index under (row, col), if any.
    /// The tab bar is rendered as "label │ label │ ..." starting at column 0.
    fn tab_hit(&self, col: u16) -> Option<usize> {
        // Rendering is " label " with a " │ " separator; account for the padding
        let mut x: u16 = 0;
        for (i, label) in self.tabs().iter().enumerate() {
            let w = label.chars().count() as u16 + 2;
            let end = x.saturating_add(w);
            if col >= x && col < end {
                return Some(i);
            }
            x = end.saturating_add(3); // " │ " separator
        }
        None
    }

    pub fn render(&mut self, frame: &mut Frame, state: &ServerState) {
        let chunks = split_layout(frame.area());
        self.render_title(frame, chunks[0]);
        self.render_tabs(frame, chunks[1]);
        match self.tab {
            0 => self.render_dashboard(frame, chunks[2], state),
            1 => self.render_settings(frame, chunks[2]),
            2 => self.render_chain(frame, chunks[2]),
            3 => self.render_connection(frame, chunks[2]),
            _ => self.render_logs(frame, chunks[2]),
        }
        self.render_footer(frame, chunks[3]);
    }

    /// Top bar: logo + title on the left, server status badge on the right.
    fn render_title(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let w = area.width as usize;
        let logo = Span::styled(
            " ● MicYou ",
            Style::default()
                .fg(Color::Black)
                .bg(theme.primary.to_color())
                .add_modifier(Modifier::BOLD),
        );
        let title = Span::raw(format!(" {} ", self.t("app_title")));
        let os = Span::styled(
            format!("({})", std::env::consts::OS),
            Style::default().fg(theme.secondary.to_color()),
        );
        let running = self.device.is_some() || self.level > 0;
        let badge = if running {
            Span::styled(
                format!(" ● {} ", self.t("server_running")),
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.primary.to_color())
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!(" ○ {} ", self.t("server_running")),
                Style::default()
                    .fg(theme.on_surface.to_color())
                    .bg(theme.surface_variant.to_color()),
            )
        };
        let mode = Span::styled(
            format!(" {} {} ", self.mode_label(), self.port),
            Style::default().fg(theme.tertiary.to_color()),
        );
        let mut line = vec![logo, title, os];
        let used = 3 + self.t("app_title").chars().count() + 3 + 3 + 3;
        let badge_w = 4 + self.t("server_running").chars().count();
        let mode_w = 2 + self.mode_label().chars().count() + 1 + self.port.to_string().len() + 1;
        if used + badge_w + mode_w + 2 <= w {
            line.push(Span::raw(
                " ".repeat(w.saturating_sub(used + badge_w + mode_w)),
            ));
            line.push(badge);
            line.push(mode);
        }
        frame.render_widget(Paragraph::new(Line::from(line)), area);
    }

    /// Tab bar: each tab is a solid block when selected, plain text otherwise.
    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let mut spans: Vec<Span> = Vec::new();
        for (i, label) in self.tabs().iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" │ "));
            }
            if i == self.tab {
                spans.push(Span::styled(
                    format!(" {} ", label),
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme.primary.to_color())
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", label),
                    Style::default().fg(theme.secondary.to_color()),
                ));
            }
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let key = |label: &str, bg: Color| {
            Span::styled(
                format!(" {label} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            )
        };
        let mut spans = vec![
            key("q", theme.error.to_color()),
            Span::raw(format!(" {}", self.t("quit_hint"))),
            Span::raw("  "),
            key("Tab", theme.secondary.to_color()),
            Span::raw(format!(" {}", self.t("tab_switch"))),
            Span::raw("  "),
            key("↑↓", theme.secondary.to_color()),
            Span::raw(format!(" {}", self.t("nav"))),
            Span::raw("  "),
            key("Enter", theme.primary.to_color()),
            Span::raw(format!(" {}", self.t("toggle"))),
            Span::raw("  "),
            key("-/+", theme.primary.to_color()),
            Span::raw(format!(" {}", self.t("adjust"))),
        ];
        if !self.last_event.is_empty() {
            spans.push(Span::raw("  ·  "));
            spans.push(Span::styled(
                self.last_event.clone(),
                Style::default().fg(theme.secondary.to_color()),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Dashboard: two info cards, sparkline charts, spectrum and resource line.
    /// Layout tiers adapt to terminal height so nothing overflows or clutters.
    fn render_dashboard(&mut self, frame: &mut Frame, area: Rect, state: &ServerState) {
        let _ = self.sample_system();
        let h = area.height;
        let (info_h, charts_h, spectrum_on, resource_on) = if h >= 26 {
            (9u16, 8u16, true, true)
        } else if h >= 20 {
            (8, 6, true, false)
        } else if h >= 15 {
            (8, 5, false, false)
        } else {
            (7, 3, false, false)
        };
        let mut constraints = vec![Constraint::Length(info_h), Constraint::Length(charts_h)];
        if spectrum_on {
            constraints.push(Constraint::Min(4));
        }
        if resource_on {
            constraints.push(Constraint::Length(1));
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let info_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);
        self.render_connect_card(frame, info_cols[0]);
        self.render_metric_card(frame, info_cols[1]);

        self.render_charts(frame, chunks[1]);

        if spectrum_on {
            self.render_spectrum(frame, chunks[2]);
        }
        if resource_on {
            self.render_resource_row(frame, chunks[3]);
        }
        let _ = state;
    }

    /// Left card: connection status, one datum per row (clean table look).
    fn render_connect_card(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let block = self.panel(self.t("conn_status"));
        let mut rows: Vec<Line> = Vec::new();
        let row = |label: &str, value: Vec<Span<'static>>| {
            let mut spans = vec![
                Span::styled(
                    format!("  {label:<10}"),
                    Style::default().fg(theme.secondary.to_color()),
                ),
                Span::raw(" "),
            ];
            spans.extend(value);
            Line::from(spans)
        };
        rows.push(row(
            self.t("state").as_str(),
            vec![Span::styled(
                self.t("server_running"),
                Style::default()
                    .fg(theme.primary.to_color())
                    .add_modifier(Modifier::BOLD),
            )],
        ));
        rows.push(row(
            self.t("conn_mode").as_str(),
            vec![Span::styled(
                self.mode_label(),
                Style::default()
                    .fg(theme.tertiary.to_color())
                    .add_modifier(Modifier::BOLD),
            )],
        ));
        rows.push(row(
            self.t("conn_port").as_str(),
            vec![Span::styled(
                self.port.to_string(),
                Style::default()
                    .fg(theme.tertiary.to_color())
                    .add_modifier(Modifier::BOLD),
            )],
        ));
        match &self.device {
            Some(device) => {
                rows.push(row(
                    self.t("device").as_str(),
                    vec![
                        Span::styled("● ", Style::default().fg(theme.primary.to_color())),
                        Span::styled(
                            device.name.clone(),
                            Style::default()
                                .fg(theme.primary.to_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                    ],
                ));
                rows.push(row(
                    self.t("latency").as_str(),
                    vec![Span::styled(
                        format!("{} ms", device.latency),
                        Style::default().fg(theme.tertiary.to_color()),
                    )],
                ));
            }
            None => {
                rows.push(row(
                    self.t("device").as_str(),
                    vec![Span::styled(
                        clip_text(self.t("device_not_connected"), 24),
                        Style::default().fg(theme.error.to_color()),
                    )],
                ));
            }
        }
        let mute_span = if self.muted {
            Span::styled(
                format!(" {}", self.t("enabled")),
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.error.to_color())
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!(" {}", self.t("disabled")),
                Style::default()
                    .fg(theme.on_surface.to_color())
                    .bg(theme.surface_variant.to_color()),
            )
        };
        rows.push(row(self.t("muted").as_str(), vec![mute_span]));
        rows.push(row(
            self.t("web_clients").as_str(),
            vec![Span::styled(
                self.web_clients.to_string(),
                Style::default().fg(theme.primary.to_color()),
            )],
        ));
        if !self.ips.is_empty() {
            rows.push(row(
                self.t("local_ips").as_str(),
                vec![Span::styled(
                    self.ips.first().cloned().unwrap_or_default(),
                    Style::default().fg(theme.tertiary.to_color()),
                )],
            ));
        }
        frame.render_widget(Paragraph::new(rows).block(block), area);
    }

    /// Right card: live audio metrics, one datum per row.
    fn render_metric_card(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let block = self.panel(self.t("audio_metrics"));
        let row = |label: &str, value: String, color: Color| {
            Line::from(vec![
                Span::styled(
                    format!("  {label:<10}"),
                    Style::default().fg(theme.secondary.to_color()),
                ),
                Span::raw(" "),
                Span::styled(
                    value,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ])
        };
        if let Some(m) = &self.metrics {
            let lat_color = if m.latency_ms > 200 {
                theme.error.to_color()
            } else {
                theme.primary.to_color()
            };
            let loss_color = if m.packet_loss_rate > 0.05 {
                theme.error.to_color()
            } else {
                theme.tertiary.to_color()
            };
            let rows = vec![
                row(
                    self.t("bitrate").as_str(),
                    format!("{} kbps", m.bitrate / 1000),
                    theme.tertiary.to_color(),
                ),
                row(
                    self.t("sample_rate").as_str(),
                    format!("{} Hz", m.sample_rate),
                    theme.tertiary.to_color(),
                ),
                row(
                    self.t("latency").as_str(),
                    format!("{} ms", m.latency_ms),
                    lat_color,
                ),
                row(
                    self.t("network_latency").as_str(),
                    format!("{} ms", m.network_latency_ms),
                    theme.secondary.to_color(),
                ),
                row(
                    self.t("jitter").as_str(),
                    format!("{:.1} ms", m.jitter_ms),
                    theme.secondary.to_color(),
                ),
                row(
                    self.t("packet_loss").as_str(),
                    format!("{:.1}%", m.packet_loss_rate * 100.0),
                    loss_color,
                ),
                row(
                    self.t("buffer").as_str(),
                    format!("{} ms", m.buffer_duration_ms),
                    theme.secondary.to_color(),
                ),
            ];
            frame.render_widget(Paragraph::new(rows).block(block), area);
        } else {
            let hint = if self.ips.is_empty() {
                self.t("server_addr_hint")
            } else {
                format!(
                    "{}:{}",
                    self.ips.first().cloned().unwrap_or_default(),
                    self.port
                )
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        format!("  {}", self.t("server_addr_hint")),
                        Style::default().fg(theme.secondary.to_color()),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  {hint}"),
                        Style::default()
                            .fg(theme.tertiary.to_color())
                            .add_modifier(Modifier::BOLD),
                    )),
                ])
                .block(block),
                area,
            );
        }
    }

    /// Bottom thin line: system + process resource usage (no heavy panel).
    fn render_resource_row(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let cpu_pct = self.cpu_usage as u32;
        let mem_pct = (self.mem_used as f64 / self.mem_total as f64 * 100.0) as u32;
        let mb = self.proc_mem as f64 / (1024.0 * 1024.0);
        let bar = |pct: u32, c: Color, width: usize| -> Span<'static> {
            let filled = (pct as usize * width / 100).min(width);
            let mut s = String::with_capacity(width);
            s.push('[');
            for i in 0..width {
                s.push(if i < filled { '█' } else { '░' });
            }
            s.push(']');
            Span::styled(s, Style::default().fg(c))
        };
        let w = (area.width as usize / 40).clamp(5, 12);
        let line = Line::from(vec![
            Span::styled(
                format!("  {} ", self.t("sys_cpu")),
                Style::default()
                    .fg(theme.secondary.to_color())
                    .add_modifier(Modifier::BOLD),
            ),
            bar(
                cpu_pct,
                if cpu_pct > 80 {
                    theme.error.to_color()
                } else {
                    theme.primary.to_color()
                },
                w,
            ),
            Span::raw(format!(" {cpu_pct}%   ")),
            Span::styled(
                format!("{} ", self.t("sys_mem")),
                Style::default()
                    .fg(theme.secondary.to_color())
                    .add_modifier(Modifier::BOLD),
            ),
            bar(
                mem_pct,
                if mem_pct > 90 {
                    theme.error.to_color()
                } else {
                    theme.tertiary.to_color()
                },
                w,
            ),
            Span::raw(format!(" {mem_pct}%   ")),
            Span::styled(
                format!(
                    "{} {:.1}%  {} {:.0}MB",
                    self.t("proc_cpu"),
                    self.proc_cpu,
                    self.t("proc_mem"),
                    mb
                ),
                Style::default().fg(theme.secondary.to_color()),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    /// Two side-by-side sparkline panels: input level and network latency.
    fn render_charts(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        self.render_spark_panel(
            frame,
            chunks[0],
            self.t("input_level"),
            &self.level_history,
            100,
            theme.primary.to_color(),
            format!("{}%", self.level.min(100)),
        );
        let (lat_max, lat_peak) = latency_scale(&self.latency_history);
        self.render_spark_panel(
            frame,
            chunks[1],
            self.t("latency"),
            &self.latency_history,
            lat_max,
            theme.tertiary.to_color(),
            format!(
                "{} ms · peak {lat_peak} ms",
                self.metrics.as_ref().map(|m| m.latency_ms).unwrap_or(0)
            ),
        );
    }

    /// One panel: title, sparkline curve and a readout line.
    #[allow(clippy::too_many_arguments)]
    fn render_spark_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: String,
        data: &VecDeque<u64>,
        max: u64,
        color: Color,
        readout: String,
    ) {
        let block = self.panel(title.clone());
        let inner = block.inner(area);
        let h = inner.height as usize;
        if h == 0 {
            return;
        }
        // Split inner: curve on top, readout below
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2), Constraint::Length(1)])
            .split(inner);
        let values: Vec<u64> = data.iter().copied().collect();
        let spark = Sparkline::default()
            .block(self.panel(title))
            .data(&values)
            .max(max.max(1))
            .style(Style::default().fg(color));
        frame.render_widget(spark, parts[0]);
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                readout,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), parts[1]);
    }

    /// Panel block with a colored title and soft border.
    fn panel(&self, title: String) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.surface_variant.to_color()))
            .title(Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(self.theme.primary.to_color())
                    .add_modifier(Modifier::BOLD),
            ))
    }

    /// Cava-style vertical bars: each column's height = spectrum band value.
    /// Color follows the theme gradient bottom (cool) to top (warm), with a
    /// bright peak row on top.
    fn render_spectrum(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let block = self.panel(self.t("spectrum"));
        let inner = block.inner(area);
        let width = inner.width as usize;
        let height = inner.height as usize;
        if width == 0 || height == 0 {
            return;
        }
        let n_cols = width.clamp(4, 64);
        let bands: Vec<f32> = (0..n_cols)
            .map(|i| {
                let src = i * 64 / n_cols;
                self.spectrum.get(src).copied().unwrap_or(0.0).min(1.0)
            })
            .collect();

        let idle = bands.iter().all(|v| *v < 0.01);
        if idle {
            // Faint track rows + centered hint
            let mut lines: Vec<Line> = Vec::with_capacity(height);
            for row in 0..height {
                if row % 2 == 0 {
                    lines.push(Line::from(Span::styled(
                        "░".repeat(width),
                        Style::default().fg(theme.surface_variant.to_color()),
                    )));
                } else {
                    lines.push(Line::from(""));
                }
            }
            let hint = format!("  {} ", self.t("spectrum_wait"));
            let top = (height / 2).saturating_sub(1);
            if let Some(l) = lines.get_mut(top) {
                let pad = width.saturating_sub(hint.chars().count()) / 2;
                *l = Line::from(vec![
                    Span::raw(" ".repeat(pad)),
                    Span::styled(
                        hint.trim().to_string(),
                        Style::default().fg(theme.secondary.to_color()),
                    ),
                ]);
            }
            frame.render_widget(Paragraph::new(lines).block(block), area);
            return;
        }

        let mut lines: Vec<Line> = Vec::with_capacity(height);
        // Peak row: highest band value rendered bright
        let peak = bands.iter().cloned().fold(0.0f32, f32::max);
        lines.push(Line::from(Span::styled(
            "█".repeat((n_cols as f32 * peak) as usize)
                + &" ".repeat(width.saturating_sub((n_cols as f32 * peak) as usize)),
            Style::default()
                .fg(theme.on_surface.to_color())
                .add_modifier(Modifier::BOLD),
        )));
        for row in 1..height {
            let threshold = (height - 1 - row) as f32 / (height - 1).max(1) as f32;
            let t = (height - 1 - row) as f32 / (height - 1).max(1) as f32;
            let color = gradient_at(self.theme.gradient, t).to_color();
            let mut text = String::with_capacity(width);
            for v in &bands {
                text.push(if *v >= threshold { '█' } else { ' ' });
            }
            lines.push(Line::from(Span::styled(text, Style::default().fg(color))));
        }
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    /// Audio parameters: rows with a mini progress bar for gain / buffer,
    /// and [ON]/[OFF] badges for toggles.
    fn render_settings(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let w = area.width as usize;
        let bar = |val: f32, min: f32, max: f32| -> String {
            if w < 40 {
                return String::new();
            }
            let bw = (w - 34).clamp(8, 24);
            let filled = ((val - min) / (max - min)).clamp(0.0, 1.0) * bw as f32;
            let mut s = String::with_capacity(bw);
            for i in 0..bw {
                s.push(if (i as f32) < filled { '█' } else { '░' });
            }
            format!(" [{s}]")
        };
        let badge = |v: bool| -> Span<'static> {
            if v {
                Span::styled(
                    format!(" {} ", self.t("enabled")),
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme.primary.to_color())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!(" {} ", self.t("disabled")),
                    Style::default()
                        .fg(theme.on_surface.to_color())
                        .bg(theme.surface_variant.to_color()),
                )
            }
        };
        let items: Vec<ListItem> = vec![
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("gain"))),
                Span::styled(
                    format!("  {:.1} dB", self.settings.gain),
                    Style::default()
                        .fg(theme.tertiary.to_color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(bar(self.settings.gain, -50.0, 50.0)),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("aec"))),
                Span::raw("  "),
                badge(self.settings.aec_enabled && self.aec_runtime_available),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("noise_reduction"))),
                Span::raw("  "),
                badge(self.settings.ns_enabled),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("dereverb"))),
                Span::raw("  "),
                badge(self.settings.dereverb_enabled),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("agc"))),
                Span::raw("  "),
                badge(self.settings.agc_enabled),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("vad"))),
                Span::raw("  "),
                badge(self.settings.vad_enabled),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("output_buffer"))),
                Span::styled(
                    format!("  {} {}", self.settings.output_buffer_ms, self.t("ms")),
                    Style::default()
                        .fg(theme.tertiary.to_color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(bar(self.settings.output_buffer_ms as f32, 100.0, 1200.0)),
            ])),
        ];
        let list = List::new(items)
            .block(self.panel(self.t("audio_params_title")))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.primary.to_color()),
            )
            .highlight_symbol(">");
        frame.render_stateful_widget(
            list,
            area,
            &mut ratatui::widgets::ListState::default().with_selected(Some(self.selected_setting)),
        );
    }

    /// Processing chain: numbered stages with the pinned AEC lock badge.
    fn render_chain(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let items: Vec<ListItem> = self
            .settings
            .processing_chain
            .iter()
            .enumerate()
            .map(|(i, stage)| {
                let (label, is_aec) = match stage.as_str() {
                    "AEC" => (format!("{} (AEC)", self.t("aec")), true),
                    "NoiseReduction" => (format!("{} (NR)", self.t("noise_reduction")), false),
                    "Dereverb" => (self.t("dereverb").to_string(), false),
                    "Equalizer" => ("Equalizer".to_string(), false),
                    "Amplifier" => (format!("{} (AMP)", self.t("gain")), false),
                    "AGC" => (format!("{} (AGC)", self.t("agc")), false),
                    "VAD" => (format!("{} (VAD)", self.t("vad")), false),
                    other => {
                        // Per-plugin DSP node `Plugin:<id>` (issue #347)
                        if let Some(plugin_id) =
                            other.strip_prefix(micyou_core::micyou_audio::dsp::PLUGIN_NODE_PREFIX)
                        {
                            (format!("Plugin: {plugin_id}"), false)
                        } else {
                            (other.to_string(), false)
                        }
                    }
                };
                let mut spans = vec![
                    Span::styled(
                        format!("{:>2}", i + 1),
                        Style::default().fg(theme.secondary.to_color()),
                    ),
                    Span::raw("  "),
                ];
                if is_aec {
                    spans.push(Span::styled(
                        format!("🔒 {label}"),
                        Style::default()
                            .fg(theme.tertiary.to_color())
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        format!("  {}", self.t("pinned")),
                        Style::default().fg(theme.secondary.to_color()),
                    ));
                } else {
                    spans.push(Span::styled(label, Style::default()));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .block(self.panel(self.t("chain_title")))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.primary.to_color()),
            )
            .highlight_symbol(">");
        frame.render_stateful_widget(
            list,
            area,
            &mut ratatui::widgets::ListState::default().with_selected(Some(self.chain_index)),
        );
    }

    /// Connection settings tab: edit shared server.json prefs.
    fn render_connection(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let mode_label = match self.prefs.mode.as_str() {
            "wifi" => self.t("mode_wifi"),
            "usb" => self.t("mode_usb"),
            "web" => self.t("mode_web"),
            _ => self.t("mode_unknown"),
        };
        let bind_label = if self.prefs.auto_bind {
            self.t("conn_auto")
        } else {
            format!("{} ({})", self.t("conn_manual"), self.prefs.bind_address)
        };
        let val = |v: String| {
            Span::styled(
                v,
                Style::default()
                    .fg(theme.tertiary.to_color())
                    .add_modifier(Modifier::BOLD),
            )
        };
        let items: Vec<ListItem> = vec![
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("conn_mode"))),
                Span::raw("  "),
                val(mode_label),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("conn_port"))),
                Span::raw("  "),
                val(self.prefs.port.to_string()),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("conn_web_port"))),
                Span::raw("  "),
                val(self.prefs.web_port.to_string()),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("conn_bind"))),
                Span::raw("  "),
                val(bind_label),
            ])),
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {}", self.t("conn_device"))),
                Span::raw("  "),
                val(if self.prefs.output_device.is_empty() {
                    self.t("none")
                } else {
                    self.prefs.output_device.clone()
                }),
            ])),
            ListItem::new(""),
            ListItem::new(Line::from(Span::styled(
                format!("  {}", self.t("conn_hint")),
                Style::default().fg(theme.secondary.to_color()),
            ))),
        ];
        let list = List::new(items)
            .block(self.panel(self.t("tab_conn")))
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.primary.to_color()),
            )
            .highlight_symbol(">");
        frame.render_stateful_widget(
            list,
            area,
            &mut ratatui::widgets::ListState::default().with_selected(Some(self.selected_conn)),
        );
    }

    fn render_logs(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme;
        let inner_h = area.height.saturating_sub(2) as usize;
        let total = self.logs.len();
        let max_offset = total.saturating_sub(inner_h.max(1));
        if self.log_offset > max_offset {
            self.log_offset = max_offset;
        }
        let start = total.saturating_sub(self.log_offset + inner_h.max(1));
        let items: Vec<ListItem> = self
            .logs
            .iter()
            .skip(start)
            .map(|l| {
                let (prefix, rest) = log_level_split(l);
                let mut spans: Vec<Span> = Vec::new();
                if let Some(p) = prefix {
                    let color = match p {
                        "[ok]" => theme.primary.to_color(),
                        "[warn]" => theme.tertiary.to_color(),
                        "[err]" => theme.error.to_color(),
                        _ => theme.secondary.to_color(),
                    };
                    spans.push(Span::styled(
                        p.to_string(),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ));
                }
                spans.push(Span::raw(rest.to_string()));
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items).block(self.panel(self.t("logs")));
        frame.render_widget(list, area);
    }
}

/// Pick a nice y-scale for the latency sparkline (50/100/200/500/1000...).
fn latency_scale(hist: &VecDeque<u64>) -> (u64, u64) {
    let peak = hist.iter().copied().max().unwrap_or(0);
    let scale = if peak <= 50 {
        50
    } else if peak <= 100 {
        100
    } else if peak <= 200 {
        200
    } else if peak <= 500 {
        500
    } else {
        ((peak / 1000) + 1) * 1000
    };
    (scale, peak)
}

/// Truncate text to a display width, appending an ellipsis when clipped.
/// Approximates CJK as double-width.
fn clip_text(s: String, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut w = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        let cw = if ch as u32 > 0x2fff { 2 } else { 1 };
        if w + cw > width {
            if w < width {
                out.push('…');
            }
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

/// Split a log line into its level tag (e.g. "[ok]") and the rest.
fn log_level_split(line: &str) -> (Option<&str>, &str) {
    if line.starts_with('[') {
        if let Some(end) = line.find(']') {
            if end + 1 < line.len() {
                let tag = &line[..=end];
                if matches!(tag, "[ok]" | "[warn]" | "[err]" | "[inf]") {
                    return (Some(tag), line[end + 1..].trim_start());
                }
            }
        }
    }
    (None, line)
}

/// Pick a gradient stop for normalized position t in [0, 1].
fn gradient_at(gradient: [Rgba; 8], t: f32) -> Rgba {
    let idx = ((t.clamp(0.0, 1.0)) * 7.0).round() as usize;
    gradient[idx.min(7)]
}

/// Enter raw terminal mode; returns a guard that restores it on drop.
pub fn enter() -> Result<(), String> {
    enable_raw_mode().map_err(|e| e.to_string())?;
    execute!(stdout(), EnterAlternateScreen).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn leave() -> Result<(), String> {
    execute!(stdout(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
    disable_raw_mode().map_err(|e| e.to_string())
}

/// Run the TUI dashboard until the user quits (q / Ctrl+C / mouse click).
/// `state` gives live access to the DSP settings and spectrum flag.
pub fn run_tui(
    rx: Receiver<Event>,
    state: Arc<ServerState>,
    port: u16,
    mode: String,
) -> Result<(), String> {
    enter()?;
    execute!(stdout(), crossterm::event::EnableMouseCapture).map_err(|e| e.to_string())?;

    let settings = state
        .dsp_settings
        .read()
        .map(|s| s.clone())
        .unwrap_or_default();
    let mut app = TuiApp::new(settings, port, mode);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))
        .map_err(|e| e.to_string())?;

    // Catch panics so `leave()` always restores the terminal (a panicking draw
    // would otherwise leave the alternate screen active and "break" the terminal)
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let mut dirty = true;
            loop {
                // Redraw only when something changed: input, server event or
                // a sampled resource value - idle frames cost almost nothing.
                if dirty {
                    terminal
                        .draw(|frame| app.render(frame, &state))
                        .map_err(|e| e.to_string())?;
                    dirty = false;
                }

                if crossterm::event::poll(std::time::Duration::from_millis(100))
                    .map_err(|e| e.to_string())?
                {
                    match event::read().map_err(|e| e.to_string())? {
                        CrosstermEvent::Key(key) => {
                            if handle_key(&mut app, key, &state) {
                                break;
                            }
                            dirty = true;
                        }
                        CrosstermEvent::Mouse(mouse) => {
                            let size = terminal.size().map_err(|e| e.to_string())?;
                            let area = Rect::new(0, 0, size.width, size.height);
                            if handle_mouse(&mut app, mouse, area, &state) {
                                break;
                            }
                            dirty = true;
                        }
                        _ => {}
                    }
                }

                // Drain incoming server events
                while let Ok(ev) = rx.try_recv() {
                    app.on_event(ev);
                    dirty = true;
                }

                // Resource values refresh every 2s - redraw only on change
                if app.sample_system() {
                    dirty = true;
                }
            }
            Ok(())
        }));

    let _ = execute!(stdout(), crossterm::event::DisableMouseCapture);
    leave()?;
    match result {
        Ok(r) => r,
        Err(_) => Err("TUI render panicked; terminal restored".to_string()),
    }
}

/// Handle mouse events: click tabs to switch, click list rows to select,
/// scroll wheel to navigate lists and logs.
fn handle_mouse(app: &mut TuiApp, mouse: MouseEvent, area: Rect, state: &ServerState) -> bool {
    let chunks = split_layout(area);
    let (col, row) = (mouse.column, mouse.row);
    match mouse.kind {
        // Left click
        K::Down(MouseButton::Left) | K::Up(MouseButton::Left) => {
            let is_up = matches!(mouse.kind, K::Up(_));
            // Tab bar row
            if row == chunks[1].y {
                if let Some(tab) = app.tab_hit(col) {
                    app.tab = tab;
                }
                return false;
            }
            // List pages: select the row under the cursor
            let body = chunks[2];
            if row > body.y && row < body.y + body.height - 1 {
                let row_in = (row - body.y - 1) as usize;
                match app.tab {
                    1 if row_in <= 6 => {
                        app.selected_setting = row_in;
                        if is_up {
                            toggle_setting(app, row_in, state);
                        }
                    }
                    2 => {
                        let len = app.settings.processing_chain.len();
                        if row_in < len {
                            app.chain_index = row_in;
                        }
                    }
                    3 if row_in <= 4 => {
                        app.selected_conn = row_in;
                        if is_up {
                            act_conn(app, row_in);
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        K::ScrollDown => {
            match app.tab {
                1 => {
                    app.selected_setting = (app.selected_setting + 1).min(6);
                }
                2 => {
                    let len = app.settings.processing_chain.len();
                    if app.chain_index + 1 < len {
                        app.chain_index += 1;
                    }
                }
                3 => {
                    app.selected_conn = (app.selected_conn + 1).min(4);
                }
                4 => app.log_offset = app.log_offset.saturating_add(1),
                _ => {}
            }
            false
        }
        K::ScrollUp => {
            match app.tab {
                1 => app.selected_setting = app.selected_setting.saturating_sub(1),
                2 => app.chain_index = app.chain_index.saturating_sub(1),
                3 => app.selected_conn = app.selected_conn.saturating_sub(1),
                4 => app.log_offset = app.log_offset.saturating_sub(1),
                _ => {}
            }
            false
        }
        _ => false,
    }
}

fn toggle_setting(app: &mut TuiApp, idx: usize, state: &ServerState) {
    match idx {
        1 if app.aec_runtime_available => {
            app.settings.aec_enabled = !app.settings.aec_enabled;
        }
        2 => app.settings.ns_enabled = !app.settings.ns_enabled,
        3 => app.settings.dereverb_enabled = !app.settings.dereverb_enabled,
        4 => app.settings.agc_enabled = !app.settings.agc_enabled,
        5 => app.settings.vad_enabled = !app.settings.vad_enabled,
        _ => {}
    }
    sync_settings(app, state);
}

fn act_conn(app: &mut TuiApp, idx: usize) {
    match idx {
        // Mode: cycle wifi -> usb -> web -> wifi
        0 => {
            app.prefs.mode = match app.prefs.mode.as_str() {
                "wifi" => "usb".to_string(),
                "usb" => "web".to_string(),
                _ => "wifi".to_string(),
            };
            app.mode.clone_from(&app.prefs.mode);
            sync_server_prefs(app);
        }
        // Bind: toggle auto/manual
        3 => {
            app.prefs.auto_bind = !app.prefs.auto_bind;
            if !app.prefs.auto_bind && app.prefs.bind_address.is_empty() {
                app.prefs.bind_address = "0.0.0.0".to_string();
            }
            sync_server_prefs(app);
        }
        _ => {}
    }
}

fn handle_key(app: &mut TuiApp, key: KeyEvent, state: &ServerState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
        KeyCode::Esc => return true,
        KeyCode::Tab => {
            app.tab = (app.tab + 1) % 5;
        }
        KeyCode::BackTab => {
            app.tab = (app.tab + 4) % 5;
        }
        KeyCode::Left => {
            app.tab = (app.tab + 4) % 5;
        }
        KeyCode::Right => {
            app.tab = (app.tab + 1) % 5;
        }
        KeyCode::Up => match app.tab {
            1 => app.selected_setting = app.selected_setting.saturating_sub(1),
            3 => app.selected_conn = app.selected_conn.saturating_sub(1),
            4 => app.log_offset = app.log_offset.saturating_add(1),
            2 => {
                let chain = &mut app.settings.processing_chain;
                if app.chain_index > 1 {
                    chain.swap(app.chain_index, app.chain_index - 1);
                    app.chain_index -= 1;
                    sync_settings(app, state);
                }
            }
            _ => {}
        },
        KeyCode::Down => match app.tab {
            1 => {
                app.selected_setting = (app.selected_setting + 1).min(6);
            }
            3 => {
                app.selected_conn = (app.selected_conn + 1).min(4);
            }
            4 => app.log_offset = app.log_offset.saturating_sub(1),
            2 => {
                let chain = &mut app.settings.processing_chain;
                if app.chain_index + 1 < chain.len() && app.chain_index >= 1 {
                    chain.swap(app.chain_index, app.chain_index + 1);
                    app.chain_index += 1;
                    sync_settings(app, state);
                }
            }
            _ => {}
        },
        KeyCode::Enter => {
            if app.tab == 1 {
                toggle_setting(app, app.selected_setting, state);
            } else if app.tab == 3 {
                act_conn(app, app.selected_conn);
            }
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            if app.tab == 1 {
                match app.selected_setting {
                    0 => app.settings.gain = (app.settings.gain - 1.0).clamp(-50.0, 50.0),
                    6 => {
                        app.settings.output_buffer_ms =
                            (app.settings.output_buffer_ms.saturating_sub(100)).clamp(100, 1200);
                    }
                    _ => {}
                }
                sync_settings(app, state);
            } else if app.tab == 3 {
                match app.selected_conn {
                    1 => {
                        app.prefs.port = app.prefs.port.saturating_sub(10).max(1024);
                        app.port = app.prefs.port;
                        sync_server_prefs(app);
                    }
                    2 => {
                        app.prefs.web_port = app.prefs.web_port.saturating_sub(10).max(1024);
                        sync_server_prefs(app);
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if app.tab == 1 {
                match app.selected_setting {
                    0 => app.settings.gain = (app.settings.gain + 1.0).clamp(-50.0, 50.0),
                    6 => {
                        app.settings.output_buffer_ms =
                            (app.settings.output_buffer_ms + 100).clamp(100, 1200);
                    }
                    _ => {}
                }
                sync_settings(app, state);
            } else if app.tab == 3 {
                match app.selected_conn {
                    1 => {
                        app.prefs.port = app.prefs.port.saturating_add(10);
                        app.port = app.prefs.port;
                        sync_server_prefs(app);
                    }
                    2 => {
                        app.prefs.web_port = app.prefs.web_port.saturating_add(10);
                        sync_server_prefs(app);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    false
}

/// Persist settings to the shared config file and apply to the running DSP.
fn sync_settings(app: &mut TuiApp, state: &ServerState) {
    if let Err(e) = state.controls().apply_dsp_settings(app.settings.clone()) {
        app.log(format!("[err] {e}"));
    }
}

/// Persist the edited connection settings to the shared server.json.
fn sync_server_prefs(app: &mut TuiApp) {
    if let Err(e) = micyou_core::config::save_server_prefs(&app.prefs) {
        app.log(format!("[err] {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use micyou_core::events::AecStatus;

    #[test]
    fn runtime_failure_does_not_overwrite_aec_preference() {
        let settings = AudioDspSettings {
            aec_enabled: true,
            ..Default::default()
        };
        let mut app = TuiApp::new(settings, 8554, "wifi".to_string());

        app.on_event(Event::AecStatus(AecStatus {
            available: false,
            enabled: false,
            reason: Some(micyou_core::micyou_audio::AecFailure::ReferenceLost),
        }));

        assert!(app.settings.aec_enabled);
        assert!(!app.aec_runtime_available);
    }

    #[test]
    fn log_records_land_on_the_logs_page() {
        let mut app = TuiApp::new(AudioDspSettings::default(), 8554, "wifi".to_string());
        app.on_event(Event::Log("[warn] [Audio] Output stream failed".to_string()));
        assert_eq!(
            app.logs.back().map(String::as_str),
            Some("[warn] [Audio] Output stream failed")
        );
    }
}
