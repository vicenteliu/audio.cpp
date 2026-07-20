use crate::manifest::Manifest;
use crate::platform::{self, Backend, Os, Platform};
use anyhow::{bail, Result};
use std::process::Command;

struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
    required: bool,
}

pub fn run(manifest: &Manifest, platform: &Platform) -> Result<Backend> {
    let backend = platform::resolve_backend(&manifest.install.backend, platform.os)?;
    let mut checks: Vec<Check> = Vec::new();

    let os_name = match platform.os {
        Os::MacOs => "macOS",
        Os::Linux => "Linux",
    };
    println!("平台: {} {} · 后端: {}", os_name, platform.arch, backend.name());
    println!();

    for tool in ["git", "cmake"] {
        checks.push(cmd_check(tool, tool, true));
    }
    checks.push(cmd_check("ninja (可选, 加速构建)", "ninja", false));

    match platform.os {
        Os::MacOs => {
            let metal_ok = Command::new("xcrun")
                .args(["--sdk", "macosx", "--find", "metal"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            checks.push(Check {
                name: "Xcode Metal 工具链 (xcrun --find metal)",
                ok: metal_ok,
                detail: if metal_ok {
                    "可用".into()
                } else {
                    "缺失 — 安装 Xcode 或 Command Line Tools".into()
                },
                required: backend == Backend::Metal,
            });
        }
        Os::Linux => {
            checks.push(gcc_version_check());
            if backend == Backend::Cuda {
                checks.push(cmd_check("nvcc (CUDA Toolkit)", "nvcc", true));
            }
        }
    }

    // prefix 可写性
    let prefix = manifest.prefix()?;
    let prefix_ok = std::fs::create_dir_all(&prefix).is_ok();
    checks.push(Check {
        name: "安装目录可写",
        ok: prefix_ok,
        detail: prefix.display().to_string(),
        required: true,
    });

    let mut missing_required = 0;
    for c in &checks {
        let mark = if c.ok {
            "✓"
        } else if c.required {
            missing_required += 1;
            "✗"
        } else {
            "-"
        };
        println!("  {} {:<42} {}", mark, c.name, c.detail);
    }
    println!();

    if missing_required > 0 {
        bail!("{missing_required} 项必需依赖缺失，请先安装后重试");
    }
    println!("环境检查通过。");
    Ok(backend)
}

fn cmd_check(name: &'static str, cmd: &str, required: bool) -> Check {
    match platform::find_cmd(cmd) {
        Some(path) => Check {
            name,
            ok: true,
            detail: path.display().to_string(),
            required,
        },
        None => Check {
            name,
            ok: false,
            detail: "未找到".into(),
            required,
        },
    }
}

fn gcc_version_check() -> Check {
    // README 要求 Linux 上 GCC 13+；clang 也可但以 gcc 为主路径。
    let output = Command::new("g++").arg("-dumpversion").output();
    match output {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let major: u32 = ver.split('.').next().unwrap_or("0").parse().unwrap_or(0);
            Check {
                name: "g++ 13+",
                ok: major >= 13,
                detail: format!("检测到 g++ {ver}"),
                required: true,
            }
        }
        _ => Check {
            name: "g++ 13+",
            ok: false,
            detail: "未找到 g++".into(),
            required: true,
        },
    }
}
