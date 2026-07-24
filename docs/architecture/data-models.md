# 数据模型与状态所有权

> 状态：Proposed
> 负责人：技术负责人
> 评审人：桌面端负责人、后端负责人、隐私负责人
> 最近更新：2026-07-24
> 更新触发：本地/服务端表、状态机、迁移或数据保留变化

本文定义逻辑实体和约束；实际列、索引和默认值以版本化迁移为事实来源。

## 客户端 SQLite

| 表 | 作用 | 关键约束 |
|---|---|---|
| `characters` | 已安装角色索引 | `subject_kind` 为 `pet_cat` 或 `human_avatar`；包哈希唯一；状态含 installing/ready/deleting/broken |
| `generation_jobs` | 云端任务本地投影与草稿恢复 | `remote_job_id` 唯一；`subject_kind` 不可变；只接受更高 `version` |
| `app_settings` | 版本化设置 | key 白名单；敏感凭据只存引用，不存私钥 |
| `runtime_state` | 当前角色、显示器、位置和运行模式 | `active_character_id`；单行/明确主键；写入由 Rust 运行状态统一串行化 |

角色大文件不进 SQLite。安装顺序是临时下载/校验 → 内容哈希目录原子移动 → SQLite 事务；已有 `pets` 行迁移为 `characters` 并标记 `pet_cat`，升级失败保留旧数据库和资源。

## 服务端 PostgreSQL

| 实体 | 作用 | 关键约束 |
|---|---|---|
| devices | 公钥、凭据状态和风险元数据 | 安装 ID 不是凭据；私钥永不上传 |
| uploads | 上传角色、内容元数据、状态与归属 | `subject_kind` 不可变；每个任务恰好一张 primary，最多两张补充图 |
| generation_jobs | 当前权威快照 | 单调 version、合法状态约束、所属设备、不可变 `subject_kind` |
| generation_attempts | 立绘/动作/步骤不可变尝试 | 引用输入、模型、工作流、配置和种子版本 |
| resources | 对象键、大小、哈希、生命周期类别 | 不保存长期签名 URL |
| idempotency_records | 设备 + 接口 + 幂等键及请求摘要 | 至少保留 7 天；同键不同体冲突 |
| deletion_requests | 删除状态与非敏感清单摘要 | requested/processing/completed/failed，可查询 |
| audit_events | 安全和删除审计 | 不含图片内容、对象键和自由文本 |

## 生成状态机

```text
created → validating → generating_portrait
→ awaiting_portrait_confirmation → generating_actions
→ postprocessing → quality_check → packaging → ready

processing → cancel_requested → canceled
processing → failed
awaiting_portrait_confirmation → expired
```

`ready` 是云端最终状态；`installed` 只存在客户端。上传有独立状态机，不能把 `uploading` 塞入生成状态。每次转换记录时间、阶段、版本、错误码、可重试性和尝试 ID。

## 迁移规范

迁移只追加且带序号。发布顺序采用 expand → 新旧代码兼容 → 数据回填/切换 → 后续 contract。不可逆迁移必须先备份、在 staging 从上一版本演练，并证明旧客户端能完成已有任务。
