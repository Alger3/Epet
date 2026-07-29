# Epet

Epet 是一款面向 Windows 的 2D 桌面角色应用。目标是让用户使用一张必选主照片和最多两张可选补充照片，生成猫咪或获授权成年人的 Q 版桌宠；两类主体分别进入专属生成流水线，最终统一产出受限的 Sprite Atlas `.epet` 角色包，由桌面端安全下载、校验、安装、运行和交互。

当前仓库处于 **阶段 5：生成服务与动画资产基础**。阶段 2–4 已完成桌面壳、Sprite Atlas 运行时、`.epet` 安全安装、角色库、本地草稿、照片清理和桌面生成状态 UI；阶段 5 已实现本地 FastAPI、PostgreSQL、Redis、MinIO、CPU/Mock Worker，以及桌面上传、SSE/轮询、产物下载、SHA-256 校验、自动安装和激活闭环。

步骤 5.4 的动画基线和 5.4.1 屏幕边缘趴伏状态已经完成。当前确定性 Mock Worker 会从清理后的照片提取稳定配色，使用猫咪或人物标准部件与骨骼生成 `idle`、`walk`、`sleep`、`tap`、`drag`、`wake`、`perch`、`perch_sleep` 多帧动作，并离线烘焙为 `.epet` v2 Atlas；走路帧按桌面实际移动距离推进，睡眠包含卧姿、闭眼和呼吸。用户把桌宠拖到显示器工作区边缘松手后会吸附并切换为只露出头和前爪/双手的趴伏动画；睡着时拖到边缘会播放闭眼伏在爪/手臂上的 `perch_sleep`，拖回屏幕中央仍继续睡眠，三击才会唤醒。

步骤 5.5 的 Provider 基础层正在实施：Worker 已有统一数据契约、Registry、MockProvider、硬件探测、确定性 Planner、模型清单/校验缓存和能力发布；FastAPI 提供能力与模型操作接口，创建页可展示 CPU/GPU、自动方案、实际 Provider、模型状态、预计速度和不可用原因，并支持自动/手动选择。OpenVINO GPU 静态预览适配器、模型准备脚本和预览确认流程已经接入；CUDA/CPU 真实适配器仍待实现。真实模型未准备或探针未通过时不会把该路线标记为可用，默认开发配置仍显式使用 `mock`。

详细进度见 [PLAN.md](PLAN.md)，已实现的动画契约见 [部件拆分、骨骼动画与 Atlas 流水线](docs/architecture/rigged-atlas-pipeline.md)。

## 冻结的 MVP 边界

- 正式支持 Windows 11 x64；Windows 10 22H2 x64 / ESU 仅尽力兼容。
- MVP 支持猫咪与 Q 版人物两类主体；人物仅限本人或获明确授权的成年人，狗及其他动物不在首版。
- 两类主体各冻结一种画风、独立创建接口/AI 流水线/质量 Gate；桌面同时只激活一个角色。
- 输入为一张主照片和最多两张补充照片。
- 客户端只运行 Sprite Atlas；骨骼、部件形变和生成模型只存在于 Worker 制作阶段。
- 本地 Worker 的 Planner 按 CUDA、OpenVINO GPU、OpenVINO CPU 评估候选；只有模型、运行时和适配器完成验证后才标记为可用。同一任务契约后续可以部署到云端。
- PostgreSQL 任务快照是状态事实来源；Redis 与 SSE 只负责通知。
- 匿名安装 ID 不是身份凭证；公开或云端服务访问必须使用设备密钥与短期令牌。当前无鉴权 API 仅限回环地址本地开发。

变更上述边界必须新建 ADR，并重新评估范围、成本与排期。

## 仓库导航

| 路径 | 职责 |
|---|---|
| `apps/desktop/` | Tauri 2、React/TypeScript、桌宠窗口与本地数据 |
| `services/api/` | FastAPI、本地上传/任务/SSE/删除 API；设备鉴权仍是公开部署前任务 |
| `services/worker/` | Mock/OpenVINO GPU Provider、模型准备、动画渲染、质量检查与打包 |
| `packages/contracts/` | OpenAPI、JSON Schema、错误码和黄金样例 |
| `packages/pet-runtime/` | Sprite Atlas 加载、渲染与行为状态机 |
| `packages/ui/` | 工坊可复用 UI |
| `assets/` | 内置测试猫咪、原创 Q 版人物和动作模板 |
| `infra/` | 本地依赖与各环境部署定义 |
| `tests/` | 跨模块契约、E2E 与脱敏测试索引 |
| `docs/` | 架构、产品、隐私、开发、测试、发布与运行手册 |

完整分类规则见 [docs/README.md](docs/README.md)，代码与文件约定见 [docs/development/repository-conventions.md](docs/development/repository-conventions.md)。

## 架构摘要

```text
React 工坊 ─┐
PixiJS 桌宠 ├─ Tauri/Rust ─ SQLite + 本地角色资源
            └────────────── FastAPI ─ PostgreSQL
                                      ├─ Redis 队列/通知
                                      ├─ MinIO 对象存储
                                      └─ Worker
                                          ├─ 当前：部件/骨骼 Mock → 多帧 Atlas
                                          ├─ 当前：OpenVINO GPU 静态 Q 版预览
                                          ├─ 下一步：CUDA / OpenVINO CPU Provider
                                          └─ 后续：真实立绘与身份质量 Gate
```

关键边界：

1. React 与桌宠 WebView 不直接访问任意文件、密钥或 Shell，只调用受限 Tauri Command。
2. API Route 只做协议适配，业务规则在 Service，外部系统访问在 Repository/Adapter。
3. Worker 消息只携带任务 ID、步骤和期望版本，真实输入从 PostgreSQL 读取。
4. `.epet` 包只允许 JSON、PNG、WebP，安装前必须完成哈希、Schema、路径和资源上限校验。
5. 骨骼、部件层和生成模型不进入桌面运行时；Worker 将动作离线烘焙成多帧 Atlas。

详见 [系统架构](docs/architecture/system-overview.md) 与 [安全边界](docs/architecture/security-boundaries.md)。

## Windows 直接使用

在 Windows 11 x64 安装 Node.js、Rust MSVC 和 Visual Studio 2022 C++ Build Tools 后，于 PowerShell 执行：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

验证通过的 NSIS 安装包输出到：

```text
apps\desktop\src-tauri\target\release\bundle\nsis\
```

也可以手动触发仓库的 `Windows installer` GitHub Actions 工作流并下载 Artifact。完整环境、使用方法、已知限制和实机清单见 [Windows 双角色离线 Alpha](docs/release/windows-offline-alpha.md)。

## 本地生成开发

推荐 Node.js、Rust 和 Python 版本分别记录在 `.node-version`、`rust-toolchain.toml` 与 `.python-version`。首次准备：

```powershell
npm install
npm run setup:python
npm run dev:infra
```

等待 `docker compose -f infra/docker/compose.yaml ps` 显示 PostgreSQL、Redis、MinIO 均为 `healthy`。随后打开三个 PowerShell 终端：

```powershell
npm run dev:api
```

```powershell
npm run dev:worker
```

```powershell
npm run dev:desktop
```

FastAPI 文档位于 `http://127.0.0.1:8000/docs`，MinIO 本地控制台位于 `http://127.0.0.1:9001`。默认账号和端口只用于回环地址开发，不得直接暴露到公网。完整说明见 [本地照片生成开发](docs/local-generation-development.md)。

Worker 启动后会把脱敏能力快照发布到 `GET /v1/capabilities`。当前 `EPET_GENERATION_PROVIDER` 默认值为 `mock`，用于保持既有本地闭环；可设置 `EPET_MODEL_CACHE_DIR` 覆盖默认的 `services/worker/.model-cache`。模型权重目录已从 Git 排除；OpenVINO 模型由固定来源准备脚本生成，而不是从未审查 URL 直接安装。CUDA/CPU 模型源尚未配置。模型缺失、校验失败或探针失败时选项会明确显示不可用，不会静默退回 Mock。

Worker 依赖锁定 `openvino==2026.2.1`。启动时 OpenVINO 探针会在独立子进程中使用内存小模型依次验证 Runtime、Intel GPU 编译和一次确定性推理，默认超时 30 秒，可通过 `EPET_OPENVINO_PROBE_TIMEOUT` 调整。探针不会下载生成模型或上传硬件信息；结果包含可用设备、实际 GPU、运行时/驱动版本、支持精度和编译/推理耗时。

2026-07-28 已在 Intel Arc 140V 8GB、驱动 `32.0.101.8243` 上实测通过：OpenVINO 发现 `CPU/GPU/NPU`，目标 `GPU` 支持 FP16/FP32/INT8，小模型首次编译约 1.31 秒、推理约 1.65 毫秒；SD1.5 + Chibi LoRA 的 512×512、batch 1 img2img 也已完成实机验证。

也可以不启动基础设施，直接执行一次探针：

```powershell
npm run probe:openvino
```

### Intel Arc 静态 Q 版预览（本地技术验证）

当前固定的基础模型为
`stable-diffusion-v1-5/stable-diffusion-v1-5@451f4fe16113bff5a5d2269ed5ad43b0592e9a14`
（CreativeML OpenRAIL-M）。Q 版 LoRA 技术验证候选为
`AlawnCN/Lora@da64e343c16010a9c1c175c4c82b205c2288c304`
中的 `chibi/blindbox_V1Mix.safetensors`（OpenRAIL，文件 SHA-256
`525491e6289d1912839c0bb5e3c3c390fead13c46cdd435c1b6a6ab3ea9ac14f`）。
模型仓库标注了 SD1.5 和触发词 `full body, chibi`，但训练图片来源仍不完整，
因此只允许本地技术验证，尚未通过发布审核。完整记录见
`services/worker/model-sources.json`。

首次准备会下载约 2.3 GB 权重、约 4.6 MB 的 U²-NetP 前景模型和约 8.5 MB
的 OpenVINO FP16 人体姿态模型，融合 LoRA，并导出 512×512、batch 1、
img2img 的 OpenVINO FP16 IR：

```powershell
npm run prepare:model:openvino
```

已有 OpenVINO 模型缓存只需补前景模型时执行：

```powershell
npm run prepare:model:foreground
npm run prepare:model:pose
```

完成后启用真实 Provider：

```powershell
$env:EPET_GENERATION_PROVIDER = "openvino-gpu"
npm run dev:worker
```

桌面端上传照片后会先停在静态 Q 版预览确认页；只有用户确认，任务才继续生成
Atlas、打包 `.epet`、下载、校验、安装并激活。真实 Provider 的打包路径会保留
确认过的透明前景预览图，不再使用 Mock 模板重画人物。生成前后都会执行本地前景
分割；仍像矩形照片/画框的结果会被质量门禁拒绝。`human_avatar` 会进一步检测 18 个
人体关键点，拆分头、躯干、双臂和双腿，绑定骨骼并烘焙 68 帧 Atlas；姿态置信度不足
的点会在 `animation/pose.json` 中明确标为模板补全。`pet_cat` 当前仍使用整体形变，
猫咪语义姿态模型尚未接入。可单独运行冷启动、重复执行和取消基准：

```powershell
npm run benchmark:model:openvino -- --input C:\path\to\photo.png
```

报告写入 `services/worker/benchmarks/openvino-arc-140v.json`。Arc 140V 实测冷加载约 21.22 秒、冷次推理约 2.40 秒、热次推理约 2.31 秒；相同种子的两次 PNG 字节一致，取消请求约 2.02 秒被观察到。其中
`peak_process_memory_mb` 是进程宿主内存峰值，不等同于 Level Zero GPU 显存占用；
GPU 峰值仍需 Intel GPA、PresentMon 或等价驱动工具补测。

`dev:web` 可以在浏览器中预览 React/PixiJS，但不包含 Tauri Command、原生窗口、照片本地持久化和自动安装能力。根命令状态如下：

| 命令 | 状态 | 说明 |
|---|---|---|
| `npm run setup:python` | 可用 | 安装 FastAPI、PostgreSQL、Redis、MinIO、Pillow 等 Python 依赖 |
| `npm run dev:infra` | 可用 | 启动本地 PostgreSQL、Redis 和 MinIO |
| `npm run stop:infra` | 可用 | 停止基础设施容器但保留 named volumes |
| `npm run dev:api` | 可用 | 启动本地 FastAPI、上传/SSE/任务/删除、能力、模型管理和产物下载接口 |
| `npm run dev:worker` | 可用 | 启动 Planner、MockProvider 和已安装的 OpenVINO GPU Provider；真实模型未准备时不会伪装为可用 |
| `npm run probe:openvino` | 可用 | 在隔离子进程中验证 OpenVINO Intel GPU 编译和小模型推理 |
| `npm run prepare:model:openvino` | 可用 | 固定版本下载、校验、融合 LoRA，并导出 OpenVINO FP16 img2img 模型 |
| `npm run benchmark:model:openvino -- --input <photo>` | 模型准备后可用 | 记录 512×512 batch 1 的冷加载、重复执行、进程内存峰值和取消结果 |
| `npm run dev:desktop` | 可用 | 启动 Tauri 主窗口、桌宠窗口和托盘 |
| `npm run dev:web` | 可用 | 仅浏览器预览 React/PixiJS，不含原生窗口能力 |
| `npm run test` | 可用 | 角色目录、运行状态和壳配置快速测试 |
| `npm run test:e2e` | 可用 | 窗口、权限、迁移、切换命令和素材哈希契约测试；Windows 交互 E2E 待补 |
| `npm run lint` | 可用但依赖平台工具链 | TypeScript 与 Rust 格式/Clippy |
| `npm run build:desktop` | Windows 可用 | 构建 Windows NSIS；发布签名尚未配置 |
| `npm run inspect:epet -- <path> [sha256]` | 可用 | 使用桌面 Rust 加载器检查生成包 |

常用验证（`local_e2e.py` 需要先启动基础设施、API 和 Worker）：

```powershell
npm run test
npm run lint
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
python services/api/tests/local_e2e.py
```

Windows 11 基础 Gate A 已完成；新增窗口模型、移动同步或渲染路径后，仍需按 [阶段 2 测试计划](docs/testing/phase-2-desktop-shell-test-plan.md) 重跑适用的 DPI、多屏、焦点、托盘退出和长稳项目。

## 协作

- 开始工作前阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。
- 安全问题按 [SECURITY.md](SECURITY.md) 报告。
- 每项功能同时满足测试、失败恢复、日志脱敏、文档同步和目标 Windows 验证，才符合 Definition of Done。
- 真实用户照片、访问令牌、签名 URL、对象键、大模型权重和生产密钥不得提交到仓库。

## 文档优先级

发生冲突时按以下顺序处理：

1. 已接受 ADR；
2. 版本化 OpenAPI / JSON Schema 契约；
3. `PLAN.md` 中冻结的 MVP 决策；
4. 模块 README 与设计文档；
5. 示例和注释。

发现冲突时不要静默选择，需通过 ADR 或契约变更记录消除冲突。
