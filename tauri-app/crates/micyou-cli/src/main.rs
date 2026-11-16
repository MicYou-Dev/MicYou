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

mod commands;
mod events;
mod logger;
mod plugin_cmds;
mod serve;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "micyou-cli",
    version,
    about = "MicYou CLI - turn your Android device into a PC microphone",
    long_about = "MicYou CLI runs the audio server with minimal memory footprint.\n\
                  Use `micyou-cli serve` to start the server, or `micyou-cli --help` for all commands."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动音频服务（纯日志模式，适合终端、systemd 和脚本）
    Serve {
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
        /// 禁用每秒打印的音频电平（level）与网络统计（stats）日志，避免终端刷屏
        #[arg(long, short = 'q', help = "静默模式：不打印每秒更新的音频电平与网络统计日志")]
        quiet: bool,
    },
    /// 显示当前服务状态
    Status,
    /// 停止服务
    Stop,
    /// 列出音频输出设备
    Devices,
    /// 读取或修改 DSP 设置
    Settings {
        #[command(subcommand)]
        action: SettingsAction,
    },
    /// 处理链路管理
    Chain {
        #[command(subcommand)]
        action: ChainAction,
    },
    /// 平台虚拟麦克风状态（PipeWire / BlackHole / VB-Cable）
    Mics {
        /// 安装虚拟麦克风驱动（仅 Windows）
        #[arg(long)]
        install: bool,
    },
    /// 列出 ADB 设备
    AdbDevices,
    /// 读取或修改共享服务器连接设置（port / mode / bindAddress / autoBind / outputDevice）
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// 显示配置文件路径
    Config,
    /// 插件开发工具：校验 / 打包 / 生成骨架
    Plugin {
        #[command(subcommand)]
        action: plugin_cmds::PluginAction,
    },
}

#[derive(Subcommand)]
enum SettingsAction {
    /// 读取设置（不指定 key 时输出全部）
    Get {
        /// 设置键名，如 gain / nsEnabled / outputBufferMs
        key: Option<String>,
    },
    /// 修改设置，如 `micyou-cli settings set gain 10`
    Set {
        /// 设置键名
        key: String,
        /// 值（数字 / 布尔 / 字符串）
        value: String,
    },
    /// 输出当前设置的 JSON 路径
    Path,
}

#[derive(Subcommand)]
enum ChainAction {
    /// 显示当前处理链路
    List,
    /// 设置处理链路顺序，如 `micyou-cli chain set AEC,NoiseReduction,Dereverb`
    Set {
        /// 逗号分隔的链路项
        chain: String,
    },
}

#[derive(Subcommand)]
enum ServerAction {
    /// 显示当前服务器连接设置
    Get,
    /// 修改服务器设置，如 `micyou-cli server set port 8554` / `micyou-cli server set mode usb`
    Set {
        /// 键名：port / webPort / mode / bindAddress / autoBind / outputDevice
        key: String,
        /// 值（数字 / 布尔 / 字符串）
        value: String,
    },
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "linux")]
    micyou_core::platform::alsa::filter_alsa_stderr();

    let cli = Cli::parse();
    logger::init(match cli.command {
        Commands::Serve { quiet: false, .. } => log::LevelFilter::Info,
        _ => log::LevelFilter::Warn,
    });
    let result = match cli.command {
        Commands::Serve {
            port,
            mode,
            device,
            bind,
            quiet,
        } => {
            let args = serve::ServeArgs {
                port,
                mode,
                device,
                bind,
                quiet,
            };
            serve::run(args).await
        }
        Commands::Status => {
            commands::cmd_status();
            Ok(())
        }
        Commands::Stop => {
            commands::cmd_stop();
            Ok(())
        }
        Commands::Devices => {
            commands::cmd_devices();
            Ok(())
        }
        Commands::Settings { action } => match action {
            SettingsAction::Get { key } => commands::cmd_settings_get(key),
            SettingsAction::Set { key, value } => commands::cmd_settings_set(key, value),
            SettingsAction::Path => {
                println!("{}", micyou_core::config::settings_path().display());
                Ok(())
            }
        },
        Commands::Chain { action } => match action {
            ChainAction::List => {
                commands::cmd_chain_list();
                Ok(())
            }
            ChainAction::Set { chain } => {
                let items: Vec<String> = chain
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if items.is_empty() {
                    Err("empty chain".to_string())
                } else {
                    commands::cmd_chain_set(items)
                }
            }
        },
        Commands::Mics { install } => {
            if install {
                #[cfg(target_os = "windows")]
                {
                    commands::cmd_mics_install().await
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Err("install is only supported on Windows (VB-CABLE)".to_string())
                }
            } else {
                commands::cmd_mics();
                Ok(())
            }
        }
        Commands::AdbDevices => {
            commands::cmd_adb_devices();
            Ok(())
        }
        Commands::Server { action } => match action {
            ServerAction::Get => {
                commands::cmd_server_get();
                Ok(())
            }
            ServerAction::Set { key, value } => commands::cmd_server_set(&key, &value),
        },
        Commands::Plugin { action } => {
            plugin_cmds::run(action)
        }
        Commands::Config => {
            commands::cmd_config_path();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
