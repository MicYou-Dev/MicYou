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

mod events;
mod i18n;
mod logger;
mod serve;
mod theme;
mod tui;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "micyou-tui",
    version,
    about = "MicYou interactive terminal audio server"
)]
struct Args {
    /// 音频服务器端口（UDP 端口自动 +1，默认读共享 server.json）
    #[arg(long)]
    port: Option<u16>,
    /// 服务模式：wifi | usb | web（默认读共享 server.json）
    #[arg(long, value_parser = ["wifi", "usb", "web"])]
    mode: Option<String>,
    /// 指定输出音频设备名称
    #[arg(long)]
    device: Option<String>,
    /// 绑定地址
    #[arg(long)]
    bind: Option<String>,
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "linux")]
    micyou_core::platform::alsa::filter_alsa_stderr();

    let args = Args::parse();
    let result = serve::run(serve::ServeArgs {
        port: args.port,
        mode: args.mode,
        device: args.device,
        bind: args.bind,
    })
    .await;

    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
