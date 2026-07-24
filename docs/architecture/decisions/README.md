# 架构决策记录

ADR 记录难以逆转、跨模块或会改变范围/成本/安全边界的决策。已接受 ADR 不修改历史结论；新事实通过新的 ADR 取代，并在双方添加链接。

## 索引

| ADR | 状态 | 主题 |
|---|---|---|
| [ADR-0000](ADR-0000-template.md) | Template | ADR 模板 |
| [ADR-0001](ADR-0001-sprite-atlas-runtime.md) | Accepted | 客户端只运行 Sprite Atlas |
| [ADR-0002](ADR-0002-postgresql-job-snapshot.md) | Accepted | PostgreSQL 任务快照为事实来源 |
| [ADR-0003](ADR-0003-device-credential-auth.md) | Accepted | 设备密钥与短期令牌鉴权 |
| [ADR-0004](ADR-0004-overlay-window-model.md) | Accepted | 透明置顶悬浮窗口产品模型 |
| [ADR-0005](ADR-0005-compact-overlay-implementation.md) | Proposed | 小尺寸 Overlay 与原生窗口移动实现 |

## 何时必须创建 ADR

- 改变 `PLAN.md` 冻结决策或阶段 Gate；
- 新增部署单元、数据库、队列、渲染格式或身份方案；
- 扩大 Tauri/WebView/CI 权限；
- 引入难以迁移的外部服务或持久数据格式；
- 采用与既有架构方向不同的降级或兼容策略。

编号在合并前分配且不可复用。一个 ADR 只做一个决策。
