use crate::manifest::{expand_tilde, Manifest};
use crate::platform::{self, Backend, Os, Platform};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const INSTALL_TARGETS: [&str; 2] = ["audiocpp_cli", "audiocpp_server"];

pub fn run(manifest: &Manifest, platform: &Platform, backend: Backend) -> Result<()> {
    let prefix = manifest.prefix()?;
    let src = resolve_source(manifest, &prefix)?;
    println!("源码目录: {}", src.display());

    let build_dir = src.join("build").join(format!(
        "acpd-{}-{}",
        backend.name(),
        manifest.install.build_type.to_lowercase()
    ));
    configure(manifest, platform, backend, &src, &build_dir)?;
    build(&build_dir)?;
    install_binaries(&build_dir, &prefix)?;
    Ok(())
}

/// 源码解析顺序：source_dir 显式指定 > 当前目录就是 audio.cpp 仓库 > clone 到 prefix/src。
fn resolve_source(manifest: &Manifest, prefix: &Path) -> Result<PathBuf> {
    if let Some(dir) = &manifest.install.source_dir {
        let dir = expand_tilde(dir)?;
        if !is_audiocpp_root(&dir) {
            bail!("source_dir 不是 audio.cpp 仓库根目录: {}", dir.display());
        }
        return Ok(dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        if is_audiocpp_root(&cwd) {
            println!("检测到当前目录即 audio.cpp 仓库，直接使用（开发机模式）");
            return Ok(cwd);
        }
        // acpd 从 deploy/ 目录运行时，仓库根在上一级
        if let Some(parent) = cwd.parent() {
            if is_audiocpp_root(parent) {
                println!("检测到上级目录即 audio.cpp 仓库，直接使用（开发机模式）");
                return Ok(parent.to_path_buf());
            }
        }
    }

    let src = prefix.join("src").join("audio.cpp");
    if src.join(".git").exists() {
        println!("更新已有源码检出: {}", src.display());
        run_cmd(Command::new("git").current_dir(&src).args(["fetch", "--all", "--tags"]))?;
        run_cmd(Command::new("git").current_dir(&src).args(["checkout", &manifest.install.rev]))?;
        // rev 是分支时快进到远端最新；是 tag/commit 时 pull 会失败，忽略即可
        let _ = Command::new("git")
            .current_dir(&src)
            .args(["pull", "--ff-only"])
            .status();
    } else {
        println!("克隆 {} (rev: {})", manifest.install.repo, manifest.install.rev);
        std::fs::create_dir_all(src.parent().unwrap())?;
        run_cmd(Command::new("git").args([
            "clone",
            &manifest.install.repo,
            src.to_str().context("路径编码错误")?,
        ]))?;
        run_cmd(Command::new("git").current_dir(&src).args(["checkout", &manifest.install.rev]))?;
    }
    Ok(src)
}

fn is_audiocpp_root(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("CMakeLists.txt"))
        .map(|s| s.contains("project(AudioCpp"))
        .unwrap_or(false)
}

/// cmake 参数对齐 scripts/build_metal.sh / build_linux.sh 的默认行为。
fn configure(
    manifest: &Manifest,
    platform: &Platform,
    backend: Backend,
    src: &Path,
    build_dir: &Path,
) -> Result<()> {
    let on_off = |v: bool| if v { "ON" } else { "OFF" };
    let mut cmd = Command::new("cmake");
    cmd.arg("-S").arg(src).arg("-B").arg(build_dir);
    cmd.arg(format!("-DCMAKE_BUILD_TYPE={}", manifest.install.build_type));
    cmd.arg(format!("-DENGINE_ENABLE_METAL={}", on_off(backend == Backend::Metal)));
    cmd.arg(format!("-DENGINE_ENABLE_CUDA={}", on_off(backend == Backend::Cuda)));
    cmd.arg("-DENGINE_ENABLE_VULKAN=OFF");
    cmd.arg("-DENGINE_ENABLE_OPENMP=OFF");
    cmd.arg("-DGGML_OPENMP=OFF");
    cmd.arg("-DENGINE_ENABLE_LLAMAFILE=ON");
    cmd.arg(format!(
        "-DENGINE_ENABLE_NATIVE_CPU={}",
        on_off(manifest.install.native_cpu)
    ));
    cmd.arg("-DENGINE_BUILD_TESTS=OFF");
    cmd.arg("-DENGINE_BUILD_EXAMPLES=OFF");
    cmd.arg("-DENGINE_BUILD_WARMBENCH=OFF");
    if platform.os == Os::MacOs {
        cmd.arg("-DGGML_METAL_EMBED_LIBRARY=ON");
    }
    if platform::find_cmd("ninja").is_some() {
        cmd.args(["-G", "Ninja"]);
    }
    println!("配置 cmake → {}", build_dir.display());
    run_cmd(&mut cmd)
}

fn build(build_dir: &Path) -> Result<()> {
    let jobs = platform::jobs();
    println!("编译 {:?}（{} 并行任务，首次约需几分钟到几十分钟）", INSTALL_TARGETS, jobs);
    let mut cmd = Command::new("cmake");
    cmd.arg("--build")
        .arg(build_dir)
        .args(["--parallel", &jobs.to_string()]);
    for target in INSTALL_TARGETS {
        cmd.args(["--target", target]);
    }
    run_cmd(&mut cmd)
}

fn install_binaries(build_dir: &Path, prefix: &Path) -> Result<()> {
    let bin_dir = prefix.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    for target in INSTALL_TARGETS {
        let from = build_dir.join("bin").join(target);
        let to = bin_dir.join(target);
        std::fs::copy(&from, &to)
            .with_context(|| format!("安装 {} → {} 失败", from.display(), to.display()))?;
        println!("已安装 {}", to.display());
    }
    Ok(())
}

fn run_cmd(cmd: &mut Command) -> Result<()> {
    let rendered = format!("{cmd:?}");
    let status = cmd
        .status()
        .with_context(|| format!("无法启动命令: {rendered}"))?;
    if !status.success() {
        bail!("命令失败 ({status}): {rendered}");
    }
    Ok(())
}
