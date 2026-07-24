# 仓库与文件规范

> 状态：Active
> 负责人：技术负责人
> 评审人：桌面端负责人、后端/AI 负责人、测试负责人
> 最近更新：2026-07-24
> 更新触发：新增语言、模块、构建系统、契约类型或发布产物

## 目的

让每个文件都有唯一归属、清晰依赖方向和可自动检查的命名，避免按人员或“临时”目录堆放内容。

## 目录和依赖方向

```text
apps ───────┐
services ───┼──> packages/contracts
            └──> 各自允许的共享 package

pet-runtime ──> contracts
ui ───────────> contracts（只允许展示模型）
contracts ────> 不依赖任何业务模块
```

- `apps/` 是可分发客户端；不得被 `packages/` 或 `services/` 反向依赖。
- `services/` 是独立部署单元；API 和 Worker 通过契约、数据库事实与队列消息协作，不导入彼此内部代码。
- `packages/` 只保存真正跨模块且有稳定边界的内容，不建立万能 `common` 或 `utils` 包。
- `assets/` 只保存可再分发、已核验许可证的产品资源；测试素材进入 `tests/fixtures/` 的脱敏索引。
- `infra/` 不保存应用业务逻辑和生产秘密。

## 文件类型归属

| 文件类型 | 位置 | 规则 |
|---|---|---|
| `.ts/.tsx` | desktop、runtime、ui | TypeScript strict；组件 PascalCase，非组件 kebab-case |
| `.rs` | `apps/desktop/src-tauri/` | 原生窗口、SQLite、凭据与受限 Command |
| `.py` | `services/` | API 与 Worker 分部署；模块、函数使用 snake_case |
| `.schema.json` | `packages/contracts/schemas/` | JSON Schema 2020-12，包含 `$id` 和版本 |
| `.yaml` | `packages/contracts/openapi/`、`infra/` | 2 空格缩进；环境差异使用显式配置 |
| SQL 迁移 | 服务所属 `migrations/` | 只追加、带序号；禁止修改已发布迁移 |
| 测试 | 源码旁或 `tests/` | 单元测试靠近模块，跨模块测试放根目录 |
| `.md` | 根、模块或 `docs/` | 遵循 `docs/README.md` 分类和元数据 |
| 二进制资源 | `assets/` | 必须有来源、许可证、哈希和体积说明 |
| 生成文件 | 各包 `generated/` | 文件头标记来源；禁止人工编辑 |

## 配置和环境

- 只提交 `.env.example`，所有值必须是假值或本地安全默认值。
- `development`、`staging`、`production` 使用隔离的数据库、存储、队列和密钥。
- 配置读取顺序和默认值必须有测试；生产环境缺少关键配置时快速失败。
- 版本、功能开关和阈值集中配置并版本化，不散落魔法数字。

## 文件规模与模块边界

- 文件名表达单一职责；出现跨层导入或循环依赖时优先拆边界，而不是增加转发文件。
- 入口文件只做装配，不承载业务规则。
- API 使用 Route → Service → Repository/Adapter；React 使用 Page → Feature → shared UI；Rust Command → domain service → adapter。
- 删除代码时同步删除死文档、过期配置和不再使用的测试数据索引。

## 禁止项

- `misc/`、`temp/`、`new/`、`final/`、`common/` 等无边界目录。
- 在 WebView、Route、队列消息或日志中传递设备私钥、签名 URL和原图内容。
- 用 README 复制 OpenAPI/Schema 字段形成第二事实来源。
- 提交无法说明授权来源的照片、字体、模型或动作模板。

## 验证

CI 最终必须检查格式、类型、依赖方向、Schema/OpenAPI、内部链接、秘密和大文件。新增顶层目录必须通过 ADR 说明其职责与依赖。
