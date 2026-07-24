# Epet 文档中心

本目录保存“为什么、是什么、如何验证和如何运维”。可执行协议放在 `packages/contracts/`，实现细节放在各模块源码与 README，避免同一事实在多处重复维护。

## 文档分类

| 分类 | 路径 | 回答的问题 | 主要责任人 |
|---|---|---|---|
| 架构 | `architecture/` | 系统如何拆分，边界和决策是什么 | 技术负责人 |
| 产品 | `product/` | 为谁解决什么问题，范围与验收是什么 | 产品负责人 |
| 隐私 | `privacy/` | 数据如何收集、使用、保留和删除 | 产品 + 安全负责人 |
| 开发 | `development/` | 如何组织代码、文档、评审和完成任务 | 各模块负责人 |
| 测试 | `testing/` | 测什么、在哪测、证据如何归档 | 测试负责人 |
| 发布 | `release/` | 如何版本化、签名、灰度和回滚 | 发布负责人 |
| 运行手册 | `runbooks/` | 告警发生后如何确认、止损、恢复 | 当值负责人 |
| 模板 | `templates/` | 新文档必须包含哪些结构 | 文档维护者 |

下列内容不放在 `docs/`：

- OpenAPI、JSON Schema、机器错误码：`packages/contracts/`；
- 模块安装、命令和依赖：对应模块 `README.md`；
- 测试代码：模块 `tests/` 或根 `tests/`；
- 生成报告：`reports/` 或 CI Artifact，仓库只保留脱敏基线与索引；
- 密钥、真实照片、生产数据：禁止进入仓库。

## 文档类型和命名

| 类型 | 命名 | 状态规则 |
|---|---|---|
| 索引 README | `README.md` | 只做导航和边界，不堆叠设计细节 |
| ADR | `ADR-0001-kebab-case.md` | `Proposed → Accepted → Superseded/Rejected`，接受后不改结论 |
| 架构设计 | `<subject>.md` | `Draft → Active → Deprecated` |
| 产品规范 | `<feature>-requirements.md` | 必须包含范围、状态、错误与验收 |
| 运行手册 | `<incident>.md` | 必须可按步骤执行，并记录最近演练日期 |
| 测试计划 | `<scope>-test-plan.md` | 必须说明环境、样本、阈值和证据位置 |
| 发布报告 | `v<semver>-<channel>.md` | 发布后只追加勘误，不改历史结论 |
| 契约 | `*.yaml` / `*.schema.json` | 机器可校验；破坏性变化提升版本 |

文件和目录统一使用小写 `kebab-case`；标准缩写 `ADR`、`API`、`SSE` 保留大写仅用于标题。日期使用 `YYYY-MM-DD`，时间戳使用 UTC RFC 3339，版本使用 SemVer。

## 必填元数据

除索引 README、根 `PLAN.md` 和机器契约外，Markdown 文档标题后必须包含：

```text
> 状态：Draft | Proposed | Active | Accepted | Deprecated | Superseded | Rejected
> 负责人：角色或团队（禁止长期使用“团队”）
> 评审人：角色列表
> 最近更新：YYYY-MM-DD
> 更新触发：哪些代码、契约、指标或流程变化时必须更新
```

尚未指派具体姓名时使用明确角色，并在里程碑启动前替换。文档正文至少包含目的、范围、不在范围、正文、验证/验收和相关链接；不适用项写明“不适用”及原因，不能直接省略。

## 状态含义

- `Draft`：正在编写，不可作为实现依据。
- `Proposed`：内容完整，等待指定评审或实测。
- `Active`：现行规范或操作说明。
- `Accepted`：ADR 已批准并具有约束力。
- `Deprecated`：仍可查阅但不得用于新实现。
- `Superseded`：已被另一文档替代，必须链接替代项。
- `Rejected`：方案不采用，保留原因避免重复讨论。

## 更新和评审

1. 同一变更中更新代码、契约、测试和文档。
2. 链接应使用相对路径；标题不使用含义不明的“其他”“杂项”“临时”。
3. 事实写入唯一来源，其他文档通过链接引用。
4. 评审必须验证内容与实现/配置一致，不能只检查措辞。
5. 每季度清理过期文档；每次发布复核架构、隐私、运行手册与兼容矩阵。

## 事实来源

| 事实 | 唯一来源 |
|---|---|
| 冻结 MVP 决策 | `PLAN.md` 与已接受 ADR |
| HTTP API | `packages/contracts/openapi/openapi.yaml` |
| 宠物包与事件结构 | `packages/contracts/schemas/` |
| 数据库当前结构 | 版本化迁移；文档只解释意图 |
| 生产任务状态 | PostgreSQL 任务快照 |
| 发布是否通过 | 对应发布候选报告和 Gate 证据 |
| 用户数据处理 | `privacy/data-lifecycle.md` 与实际配置 |
