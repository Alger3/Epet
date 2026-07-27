# Epet

Epet 是一款面向 Windows 的 2D 桌面角色应用。目标是让用户使用一张必选主照片和最多两张可选补充照片，生成猫咪或获授权成年人的 Q 版桌宠；两类主体分别进入专属生成流水线，最终统一产出受限的 Sprite Atlas `.epet` 角色包，由桌面端安全下载、校验、安装、运行和交互。

当前仓库处于 **阶段 5：生成服务与动画资产基础**。阶段 2–4 已完成桌面壳、Sprite Atlas 运行时、`.epet` 安全安装、角色库、本地草稿、照片清理和桌面生成状态 UI；阶段 5 已实现本地 FastAPI、PostgreSQL、Redis、MinIO、CPU/Mock Worker，以及桌面上传、SSE/轮询、产物下载、SHA-256 校验、自动安装和激活闭环。

步骤 5.4 的动画基线已经完成。当前确定性 Mock Worker 会从清理后的照片提取稳定配色，使用猫咪或人物标准部件与骨骼生成 `idle`、`walk`、`sleep`、`tap`、`drag`、`wake` 多帧动作，并离线烘焙为 `.epet` v2 Atlas；走路帧按桌面实际移动距离推进，睡眠包含卧姿、闭眼和呼吸。它仍不会把照片真正重绘为对应身份的 Q 版角色，下一步是接入 OpenVINO/CUDA/CPU 真实模型 Provider。项目不会把程序化 Mock 产物描述为 AI 生成成功。

详细进度见 [PLAN.md](PLAN.md)，已实现的动画契约见 [部件拆分、骨骼动画与 Atlas 流水线](docs/architecture/rigged-atlas-pipeline.md)。

## 冻结的 MVP 边界

- 正式支持 Windows 11 x64；Windows 10 22H2 x64 / ESU 仅尽力兼容。
- MVP 支持猫咪与 Q 版人物两类主体；人物仅限本人或获明确授权的成年人，狗及其他动物不在首版。
- 两类主体各冻结一种画风、独立创建接口/AI 流水线/质量 Gate；桌面同时只激活一个角色。
- 输入为一张主照片和最多两张补充照片。
- 客户端只运行 Sprite Atlas；骨骼、部件形变和生成模型只存在于 Worker 制作阶段。
- 本地 Worker 按 CUDA、OpenVINO GPU、OpenVINO CPU 选择生成 Provider；同一任务契约后续可以部署到云端。
- PostgreSQL 任务快照是状态事实来源；Redis 与 SSE 只负责通知。
- 匿名安装 ID 不是身份凭证；公开或云端服务访问必须使用设备密钥与短期令牌。当前无鉴权 API 仅限回环地址本地开发。

变更上述边界必须新建 ADR，并重新评估范围、成本与排期。

## 仓库导航

| 路径 | 职责 |
|---|---|
| `apps/desktop/` | Tauri 2、React/TypeScript、桌宠窗口与本地数据 |
| `services/api/` | FastAPI、本地上传/任务/SSE/删除 API；设备鉴权仍是公开部署前任务 |
| `services/worker/` | CPU/Mock Worker、后续模型 Provider、动画渲染、质量检查与打包 |
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
                                          ├─ 下一步：OpenVINO / CUDA / CPU Provider
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

`dev:web` 可以在浏览器中预览 React/PixiJS，但不包含 Tauri Command、原生窗口、照片本地持久化和自动安装能力。根命令状态如下：

| 命令 | 状态 | 说明 |
|---|---|---|
| `npm run setup:python` | 可用 | 安装 FastAPI、PostgreSQL、Redis、MinIO、Pillow 等 Python 依赖 |
| `npm run dev:infra` | 可用 | 启动本地 PostgreSQL、Redis 和 MinIO |
| `npm run stop:infra` | 可用 | 停止基础设施容器但保留 named volumes |
| `npm run dev:api` | 可用 | 启动本地 FastAPI、上传/SSE/任务/删除和产物下载接口 |
| `npm run dev:worker` | 可用 | 启动确定性多帧骨骼 Mock Worker；生成动画 Atlas，但当前不执行真实 Q 版重绘 |
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
