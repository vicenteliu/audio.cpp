use crate::manifest::Manifest;
use crate::platform::Backend;
use anyhow::{Context, Result};
use serde_json::json;

/// 由 deploy.toml 渲染 server.json 到 prefix/etc/。
/// voice preset（参考音频下载）在 M2 随模型托管一起实现。
pub fn render(manifest: &Manifest, backend: Backend) -> Result<()> {
    let prefix = manifest.prefix()?;
    let models: Vec<_> = manifest
        .models
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "family": m.family,
                "path": prefix.join("models").join(&m.id),
                "task": m.task,
                "mode": "offline",
            })
        })
        .collect();

    let config = json!({
        "host": manifest.server.host,
        "port": manifest.server.port,
        "backend": backend.name(),
        "device": 0,
        "threads": manifest.server.threads,
        "lazy_load": manifest.server.lazy_load,
        "models": models,
    });

    let etc = prefix.join("etc");
    std::fs::create_dir_all(&etc)?;
    let path = etc.join("server.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config)?)
        .with_context(|| format!("写入 {} 失败", path.display()))?;
    println!("已生成 {}", path.display());
    if manifest.models.is_empty() {
        println!("提示: 清单中没有 [[models]] 条目，server.json 的模型列表为空");
    }
    Ok(())
}
