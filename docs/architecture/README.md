# 架构文档

| 文档 | 作用 |
|---|---|
| [system-overview.md](system-overview.md) | 容器、职责、数据流、状态所有权和降级 |
| [desktop-shell.md](desktop-shell.md) | 阶段 2 窗口、托盘、SQLite、DPI/多屏与失败恢复 |
| [security-boundaries.md](security-boundaries.md) | 信任区、权限、鉴权、文件和发布密钥 |
| [data-models.md](data-models.md) | 本地/服务端实体、状态机与迁移规则 |
| [tauri-command-registry.md](tauri-command-registry.md) | Command 与窗口 capability 审批清单 |
| [decisions/](decisions/) | 不可逆或跨模块选择的 ADR |

架构文档解释边界和原因，不复制 OpenAPI 字段或数据库当前列。实现结构变化先更新设计；不可逆选择通过 ADR。
