mod builder;
mod config;
mod doctor;
mod manifest;
mod platform;

use anyhow::Result;
use clap::{Parser, Subcommand};
use manifest::Manifest;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "acpd", version, about = "audio.cpp 自动安装部署工具")]
struct Cli {
    /// 部署清单路径
    #[arg(long, global = true, default_value = "deploy.toml")]
    manifest: PathBuf,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 检查平台、工具链与环境
    Doctor,
    /// 拉取源码并编译安装 audiocpp_cli / audiocpp_server
    Build,
    /// 全流程安装：doctor → build → 模型同步 → 配置生成 → 服务托管
    Install,
    /// 模型下载/校验/打包 (M2)
    Models,
    /// 由清单渲染 server.json
    ConfigRender,
    /// launchd/systemd 服务管理 (M3)
    Service,
    /// 汇总状态 (M3)
    Status,
    /// 升级：更新源码 → 重编译 → 重启 (M3)
    Upgrade,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let manifest = Manifest::load(&cli.manifest)?;
    let platform = platform::detect()?;

    match cli.command {
        Cmd::Doctor => {
            doctor::run(&manifest, &platform)?;
        }
        Cmd::Build => {
            let backend = doctor::run(&manifest, &platform)?;
            builder::run(&manifest, &platform, backend)?;
        }
        Cmd::Install => {
            let backend = doctor::run(&manifest, &platform)?;
            builder::run(&manifest, &platform, backend)?;
            config::render(&manifest, backend)?;
            println!();
            println!("M1 安装完成（二进制 + 配置）。模型同步与服务托管在 M2/M3 落地，");
            println!("当前请手动放置模型后用 prefix/bin/audiocpp_server --config prefix/etc/server.json 启动。");
        }
        Cmd::ConfigRender => {
            let backend = platform::resolve_backend(&manifest.install.backend, platform.os)?;
            config::render(&manifest, backend)?;
        }
        Cmd::Models => {
            println!("models pack/sync 属于 M2（见 deploy/DESIGN.md 里程碑），尚未实现。");
        }
        Cmd::Service | Cmd::Status | Cmd::Upgrade => {
            println!("该命令属于 M3（见 deploy/DESIGN.md 里程碑），尚未实现。");
        }
    }
    Ok(())
}
