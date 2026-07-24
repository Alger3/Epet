# Epet

Epet 是一款面向 Windows 的 2D 桌面宠物应用。用户使用一张必选主照片和最多两张可选补充照片生成猫咪形象；云端产出受限的 Sprite Atlas 宠物包，桌面端负责安全下载、校验、运行和交互。

当前仓库处于 **阶段 2：桌面壳实现与验证**。内置离线宠物、双窗口、托盘、单实例、SQLite 运行状态和多屏恢复已经进入代码；阶段 0 的 Windows 窗口 Gate A、AI 一致性 Gate B 以及阶段 1 的 Mock API/基础 CI 尚未完成，不能把代码完成误写成 Gate 通过。

## 冻结的 MVP 边界

- 正式支持 Windows 11 x64；Windows 10 22H2 x64 / ESU 仅尽力兼容。
- MVP 只支持猫、一种画风、单只桌宠。
- 输入为一张主照片和最多两张补充照片。
- 客户端只运行 Sprite Atlas；生成质量不足时降级为模板轻动画。
- PostgreSQL 任务快照是状态事实来源；Redis 与 SSE 只负责通知。
- 匿名安装 ID 不是身份凭证，访问必须使用设备密钥与短期令牌。

变更上述边界必须新建 ADR，并重新评估范围、成本与排期。

## 仓库导航

| 路径 | 职责 |
|---|---|
| `apps/desktop/` | Tauri 2、React/TypeScript、桌宠窗口与本地数据 |
| `services/api/` | FastAPI、设备鉴权、上传和生成 API |
| `services/worker/` | AI、后处理、质量检查与打包 Worker |
| `packages/contracts/` | OpenAPI、JSON Schema、错误码和黄金样例 |
| `packages/pet-runtime/` | Sprite Atlas 加载、渲染与行为状态机 |
| `packages/ui/` | 工坊可复用 UI |
| `assets/` | 内置测试宠物和动作模板 |
| `infra/` | 本地依赖与各环境部署定义 |
| `tests/` | 跨模块契约、E2E 与脱敏测试索引 |
| `docs/` | 架构、产品、隐私、开发、测试、发布与运行手册 |

完整分类规则见 [docs/README.md](docs/README.md)，代码与文件约定见 [docs/development/repository-conventions.md](docs/development/repository-conventions.md)。

## 架构摘要

```text
React 工坊 ─┐
PixiJS 桌宠 ├─ Tauri/Rust ─ SQLite + 本地宠物资源
            └────────────── FastAPI ─ PostgreSQL
                                      ├─ Redis 队列/通知
                                      ├─ 对象存储
                                      └─ GPU Worker
```

关键边界：

1. React 与桌宠 WebView 不直接访问任意文件、密钥或 Shell，只调用受限 Tauri Command。
2. API Route 只做协议适配，业务规则在 Service，外部系统访问在 Repository/Adapter。
3. Worker 消息只携带任务 ID、步骤和期望版本，真实输入从 PostgreSQL 读取。
4. `.epet` 包只允许 JSON、PNG、WebP，安装前必须完成哈希、Schema、路径和资源上限校验。

详见 [系统架构](docs/architecture/system-overview.md) 与 [安全边界](docs/architecture/security-boundaries.md)。

## 开发状态与启动

当前可运行的是桌面端切片；API、Worker 与 Mock API 仍是后续阶段工作。推荐 Node.js、Rust 和 Python 版本分别记录在 `.node-version`、`rust-toolchain.toml` 与 `.python-version`。

```bash
npm install
npm run dev:web
npm run test
npm run test:e2e
npm run build:desktop
```

`dev:web` 在浏览器中预览两个窗口的前端；Windows 上使用 `npm run dev:desktop` 启动完整 Tauri 桌面壳。根命令状态如下：

| 命令 | 状态 | 说明 |
|---|---|---|
| `npm run dev:desktop` | 可用 | 启动 Tauri 主窗口、桌宠窗口和托盘 |
| `npm run dev:web` | 可用 | 仅浏览器预览 React/PixiJS，不含原生窗口能力 |
| `npm run test` | 可用 | 桌面状态和壳配置快速测试 |
| `npm run test:e2e` | 可用 | 当前执行壳契约测试，Windows 交互 E2E 待补 |
| `npm run lint` | 可用但依赖平台工具链 | TypeScript 与 Rust 格式/Clippy |
| `npm run build:desktop` | Windows 可用 | 构建 Windows NSIS；发布签名尚未配置 |
| `dev:api` / `dev:worker` | 未实现 | 属于后续生成服务阶段，不提供占位成功脚本 |

Ubuntu 仅用于前端和交叉静态检查；完整 Tauri 本机构建需要 WebKitGTK/D-Bus 等系统开发包。Windows 11 x64 的 DPI、多屏、焦点、托盘退出和八小时稳定性结果必须按 [阶段 2 测试计划](docs/testing/phase-2-desktop-shell-test-plan.md) 留证后，才能宣称 Gate A 通过。

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
