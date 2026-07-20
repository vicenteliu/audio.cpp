# acpd — audio.cpp 自动安装部署工具（设计文档）

> 2026-07-19 起步。私有工具，只存在于 fork（vicenteliu/audio.cpp），不上游化。
> 目标：在任何一台新的 macOS / Linux 机器上，一条命令完成 audio.cpp 的安装、模型下载、配置和服务托管。

## 1. 已定决策

| 决策点 | 选择 | 含义 |
|---|---|---|
| 二进制来源 | 目标机源码编译 | 工具负责工具链检测 + 拉 fork + 驱动 cmake，无需维护预编译基础设施 |
| 模型分发 | 预转换 + 集中托管 | 开发机上用 model_manager 转换一次 → 打包上传自有存储；**目标机零 Python/torch 依赖** |
| 管理范围 | 全栈托管 | 编译安装 + 模型同步 + 配置生成 + launchd/systemd 服务 + 健康检查 + 升级 |
| 代码位置 | fork 内 `deploy/` | 新目录不会与上游 merge 冲突 |

## 2. 形态

单一 Rust 二进制 `acpd`（audio.cpp deploy），围绕一份声明式清单 `deploy.toml` 工作：
清单描述"这台机器应该是什么状态"，`acpd` 负责把机器收敛到那个状态（幂等，可重复执行）。

```
acpd doctor                    # 检查平台/工具链/磁盘，报告缺什么
acpd install                   # 全流程：doctor → 拉源码 → 编译 → 装模型 → 生成配置 → 装服务 → 起服务 → 健康检查
acpd build                     # 只做拉源码 + 编译 + 安装二进制
acpd models sync               # 按清单下载/校验/解包模型（断点续传 + sha256）
acpd models pack <dir>         # (开发机用) 把转换好的模型目录打成 tar.zst + 输出清单条目
acpd config render             # 由 deploy.toml 渲染出 server.json
acpd service install|start|stop|status|logs
acpd upgrade                   # git pull fork → 重编译 → 滚动重启 → 健康检查
acpd status                    # 汇总视图：版本/服务状态/健康端点/模型完整性
```

## 3. deploy.toml（核心抽象）

```toml
[install]
prefix   = "~/.audiocpp"                              # 安装根：bin/ models/ etc/ logs/
repo     = "git@github.com:vicenteliu/audio.cpp.git"
rev      = "main"          # 可 pin tag/commit，升级即改这里
backend  = "auto"          # auto → macOS=metal, Linux=检测到 nvcc 则 cuda 否则 cpu
native_cpu = true          # 本机部署 true；如果二进制会被拷去别的机器则 false

[storage]
base_url = "https://huggingface.co/<你的私有repo>/resolve/main"   # 也支持任意 http(s)/file://

[[models]]
id      = "index-tts2"
family  = "index_tts2"
archive = "IndexTTS-2.tar.zst"
sha256  = "<由 acpd models pack 生成>"

[[models]]
id      = "moss-tts-local"
family  = "moss_tts_local"
archive = "MOSS-TTS-Local-Transformer-v1.5.tar.zst"
sha256  = "..."

[server]                    # 渲染成 server.json；voice preset 的参考音频也按 URL 下载
host = "127.0.0.1"
port = 8088
threads = 1
lazy_load = true

[service]
manager = "auto"            # auto → macOS=launchd, Linux=systemd; 也可 none
name    = "audiocpp-server"
```

## 4. 模型分发流程（关键设计）

```
开发机（有 .venv/torch）:
  model_manager 下载+转换  →  acpd models pack <模型目录>
                              → 产出 <名字>.tar.zst + sha256 + 清单片段
                              → 上传到托管存储（HF 私有 repo / S3 / 家里服务器均可）
目标机:
  acpd models sync  →  按清单下载（断点续传）→ sha256 校验 → 解包到 prefix/models/
```

选 tar.zst：模型权重（safetensors/GGUF）压缩率有限但 zstd 解压极快，且单文件便于校验和续传。

## 5. 模块结构

```
deploy/
  DESIGN.md            # 本文档
  Cargo.toml           # crate: audiocpp-deploy, bin: acpd
  deploy.example.toml
  src/
    main.rs            # clap CLI 入口与子命令分发
    manifest.rs        # deploy.toml 解析（serde）
    platform.rs        # OS/arch/后端检测（metal/cuda/cpu）
    doctor.rs          # 工具链与环境检查
    builder.rs         # git 拉取 + cmake 配置/编译 + 安装到 prefix
    models.rs          # 下载(续传)/sha256/解包/pack
    config.rs          # server.json 渲染
    service.rs         # launchd plist / systemd unit 生成与控制
    health.rs          # /health /v1/models 探测
    status.rs          # 汇总状态
```

依赖（按里程碑引入）：clap(derive)、anyhow、serde+toml+serde_json、dirs、
M2 起：reqwest(blocking+rustls)、sha2、indicatif、tar、zstd。
原则：全静态可分发，`cargo build --release` 出单文件。

## 6. 服务托管细节

- macOS：`~/Library/LaunchAgents/com.vicente.audiocpp.plist`，`KeepAlive=true`（崩溃拉起）、`RunAtLoad=true`（登录自启）；日志重定向到 `prefix/logs/`
- Linux：systemd user unit（`~/.config/systemd/user/audiocpp-server.service`，`Restart=on-failure`）；服务器场景可选 system 级
- scheduler（FastAPI）暂不纳入 v1 托管，留接口：`[service.extra]` 数组可挂任意附属进程，M4 考虑

## 7. 里程碑

- **M1（先做）**：`doctor` + `build` + `install --skip-models`——macOS Metal 与 Linux CPU 路径打通，本机可用
- **M2**：`models pack/sync` + `config render`——补上模型托管链路，端到端"一条命令部署"成立
- **M3**：`service *` + `status` + `upgrade`——全栈托管闭环
- **M4**：Linux CUDA 验证、scheduler 托管、（可选）预编译缓存加速

## 8. 与后续 Rust 计划的关系

acpd 是独立价值的第一步：不管以后 FFI 绑定（rust-refactor-plan）做不做，部署自动化都立刻有用。
如果以后做 `audiocpp-sys`，acpd 的 builder.rs（cmake 驱动逻辑）可以直接复用为 build.rs 的雏形。
