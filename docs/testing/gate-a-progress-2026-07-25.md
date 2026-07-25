# Gate A 进度记录（2026-07-25）

> 状态：In Progress，不代表 Gate A 已通过
> 基线提交：`553382ad0d9d63708bd0c09966e1fa1b75cfe17c` 加当前未提交工作区
> 目标：验证阶段 2 自主移动原型的代码路径和单机 Windows 行为

## 环境

- Windows 11 Home，版本 `10.0.26100`，Build `26100`
- NVIDIA GeForce RTX 3060 Laptop GPU
- 当前验证显示器：100% DPI，工作区 `2560 × 1392`
- Release EXE SHA-256：`6C468E60F7AD1361ACF8D13ED2C1D368CCCC9E28DB6BF325B55A8F2A5FC3E9FB`

本记录不包含用户名、设备标识、完整本地路径或桌面截图。

## 自动检查

| 检查 | 结果 |
|---|---|
| TypeScript `tsc --noEmit` | 通过 |
| Vitest | 3 个测试文件、15 项测试通过 |
| Rust 单元测试 | 5 项通过，包含左右边界、工作区底边和负坐标显示器 |
| Rust Clippy `--all-targets -- -D warnings` | 通过 |
| Tauri Release + NSIS | 通过 |
| SQLite 全新安装与 v2→v5 迁移 | 通过 |

## 本机行为证据

### P2-MV-01 自主移动

- 临时开启自主移动，保持显示且未暂停；
- 观察窗口从 `x=1889` 移动到 `x=1809`，用时约 1.6 秒，位移 `-80 px`；
- `y=1168` 保持不变，对应当前工作区底边；
- 期间 SQLite 快照为 `x=1849, y=1168, autonomous_movement=1, last_behavior_state=walk, runtime_version=5`；
- 实测速度约 50 px/s，与配置的 48 logical px/s 和 100% DPI 相符。

结论：低频原生窗口移动、工作区底边约束和周期持久化在本机通过；左右边界反向由 Rust 单元测试覆盖，仍需长时间实机触边观察。

### P2-MV-02 暂停优先级

- 临时设置自主移动开启且暂停开启；
- 观察约 1.3 秒，`DeltaX=0, DeltaY=0`；
- 测试结束后恢复原始运行快照，自主移动为关闭状态。

结论：暂停优先于自主移动的本机路径通过。隐藏和真实拖拽抢占仍需按测试计划手工留证。

### P2-FC-01 原生窗口样式预检查

当前桌宠顶层窗口的扩展样式检查结果：

- `WS_EX_TOOLWINDOW = true`
- `WS_EX_NOACTIVATE = true`
- `WS_EX_TOPMOST = true`（默认设置开启）

结论：任务栏/Alt-Tab 隐藏和不激活所需的原生样式已应用；真实键盘焦点切换矩阵仍需人工操作留证。

## Gate A 剩余项

- 125%、150%、200% DPI，以及混合 DPI 双屏；
- 左右/上下双屏、负坐标布局和显示器热插拔；
- 透明区域命中与可交互区域测试；
- 不抢焦点、任务栏和 Alt-Tab 实机矩阵；
- 隐藏、真实拖拽与移动循环的抢占留证；
- Windows 11 八小时长稳和性能曲线；
- Windows 10 22H2 / ESU 两小时尽力兼容记录；
- 将工作区变更提交后，用提交 SHA 重建并更新最终产物哈希。

上述项目全部完成并经评审前，ADR-0005 保持 Proposed，Gate A 保持未通过。
