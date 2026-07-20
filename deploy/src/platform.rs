use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Metal,
    Cuda,
    Cpu,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Metal => "metal",
            Backend::Cuda => "cuda",
            Backend::Cpu => "cpu",
        }
    }
}

pub struct Platform {
    pub os: Os,
    pub arch: &'static str,
}

pub fn detect() -> Result<Platform> {
    let os = match std::env::consts::OS {
        "macos" => Os::MacOs,
        "linux" => Os::Linux,
        other => bail!("不支持的操作系统: {other}（acpd 只支持 macOS / Linux）"),
    };
    Ok(Platform {
        os,
        arch: std::env::consts::ARCH,
    })
}

pub fn resolve_backend(requested: &str, os: Os) -> Result<Backend> {
    match (requested, os) {
        ("auto", Os::MacOs) | ("metal", Os::MacOs) => Ok(Backend::Metal),
        ("auto", Os::Linux) => Ok(if find_cmd("nvcc").is_some() {
            Backend::Cuda
        } else {
            Backend::Cpu
        }),
        ("cuda", Os::Linux) => Ok(Backend::Cuda),
        ("cpu", _) => Ok(Backend::Cpu),
        (b, _) => bail!("backend '{b}' 与当前平台不兼容（macOS: metal/cpu, Linux: cuda/cpu, 或 auto）"),
    }
}

/// PATH 中查找可执行文件，返回完整路径。
pub fn find_cmd(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

pub fn jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
}
