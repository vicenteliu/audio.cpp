use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_REPO: &str = "git@github.com:vicenteliu/audio.cpp.git";

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Manifest {
    pub install: InstallCfg,
    pub storage: StorageCfg,
    pub models: Vec<ModelCfg>,
    pub server: ServerCfg,
    pub service: ServiceCfg,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InstallCfg {
    pub prefix: String,
    pub repo: String,
    /// 本地源码目录；设置后跳过 git clone（开发机模式）。
    pub source_dir: Option<String>,
    pub rev: String,
    pub backend: String,
    pub native_cpu: bool,
    pub build_type: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageCfg {
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCfg {
    pub id: String,
    pub family: String,
    pub task: String,
    #[serde(default)]
    #[allow(dead_code)] // M2 models sync 使用
    pub archive: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // M2 models sync 使用
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerCfg {
    pub host: String,
    pub port: u16,
    pub threads: u32,
    pub lazy_load: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServiceCfg {
    pub manager: String,
    pub name: String,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            install: InstallCfg::default(),
            storage: StorageCfg::default(),
            models: Vec::new(),
            server: ServerCfg::default(),
            service: ServiceCfg::default(),
        }
    }
}

impl Default for InstallCfg {
    fn default() -> Self {
        Self {
            prefix: "~/.audiocpp".into(),
            repo: DEFAULT_REPO.into(),
            source_dir: None,
            rev: "main".into(),
            backend: "auto".into(),
            native_cpu: true,
            build_type: "Release".into(),
        }
    }
}

impl Default for ServerCfg {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8088,
            threads: 1,
            lazy_load: true,
        }
    }
}

impl Default for ServiceCfg {
    fn default() -> Self {
        Self {
            manager: "auto".into(),
            name: "audiocpp-server".into(),
        }
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("读取清单失败: {}", path.display()))?;
            let manifest: Manifest = toml::from_str(&text)
                .with_context(|| format!("解析清单失败: {}", path.display()))?;
            Ok(manifest)
        } else {
            eprintln!("提示: 未找到 {}，使用内置默认配置", path.display());
            Ok(Manifest::default())
        }
    }

    pub fn prefix(&self) -> Result<PathBuf> {
        expand_tilde(&self.install.prefix)
    }
}

pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir().context("无法确定 home 目录")?;
        Ok(home.join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}
