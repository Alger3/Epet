# 桌面壳架构

> 状态：Active
> 负责人：桌面端负责人
> 评审人：技术负责人、安全负责人、测试负责人
> 最近更新：2026-07-24
> 更新触发：窗口生命周期、Tauri Command、SQLite 状态、托盘或显示器恢复规则变化

## 目的

定义阶段 2 桌面壳的进程、窗口、权限、状态和恢复边界，使工坊、桌宠与托盘共享一个 Rust 事实来源，并可独立验证。

## 范围

包含 Tauri 进程、React 工坊、PixiJS 桌宠 WebView、系统托盘、单实例、开机启动、SQLite 运行状态和 Windows 多屏坐标恢复。不包含云端 API、宠物包安全加载、完整行为状态机和发布签名。

## 组件与依赖

```text
workshop WebView ──受限 invoke──┐
pet-overlay WebView ─受限 invoke├─> Rust Command / Window Service
tray menu ───────内部领域调用───┘          │
                                           ├─> Tauri 窗口与事件
                                           ├─> SQLite runtime_state
                                           └─> Autostart / Win32 工作区
```

- `src/windows/` 只负责窗口内 UI；不保存权威运行设置。
- `commands.rs` 校验调用窗口和输入，再委托窗口/状态函数。
- `windows.rs` 是坐标换算、生命周期恢复和原生窗口样式边界。
- `state.rs` 串行更新内存快照和 SQLite；成功落库后才广播。
- `tray.rs` 只发送领域动作并从实际状态刷新勾选项。
- SQL 迁移只追加在 `src-tauri/migrations/`，不得修改已发布迁移。

## 窗口与生命周期

| 对象 | 标签/ID | 关闭行为 | 权限与恢复 |
|---|---|---|---|
| 工坊 | `workshop` | 隐藏到托盘 | 可调用设置命令，无文件、Shell、网络插件权限 |
| 桌宠 | `pet-overlay` | 隐藏；异常销毁最多重建一次 | 无文件、Shell、网络、对话框权限 |
| 托盘 | `main-tray` | 不适用 | 打开、显隐、穿透、暂停、重置、开机启动、退出 |

单实例插件必须在其他插件之前注册；第二次启动只唤醒现有工坊。只有托盘“退出”或系统终止才设置退出标记并结束进程。`--autostart` 启动时隐藏工坊，桌宠与托盘按持久状态恢复。

## 状态一致性

`runtime_state` 是单行 SQLite 快照，当前 `runtime_version = 2`。写入流程固定为：校验调用者 → 执行原生动作 → SQLite upsert → 更新内存快照 → 广播 `runtime-state-changed` → 同步托盘。失败时返回错误，不由 WebView 猜测成功。

位置写入使用 250 ms generation 防抖。开机启动由操作系统注册项保存，不混入 SQLite；查询与修改仍通过 Rust Command，以便校验只能由工坊调用并同步托盘。

## 坐标与多显示器

状态同时保存：显示器标识、物理左上坐标、工作区宽高、DPI scale factor、宠物逻辑尺寸、脚底在工作区内的归一化 `x/y`。

恢复顺序：

1. 显示器标识仍存在时选择原显示器；
2. 标识失效时选择工作区尺寸差最小的显示器；
3. 无历史尺寸时回退主屏，再回退首个显示器；
4. 使用脚底归一化锚点换算新物理坐标；无锚点时放在右下安全区；
5. 按 Win32 工作区钳制，避免覆盖任务栏或落到不可见区域。

DPI 改变时重新执行恢复；移动/系统重定位后防抖保存。React 不做逻辑像素与物理像素换算。

## 安全边界

两个 capability 均不授予网络、Shell、对话框、剪贴板或任意文件访问。自定义 Command 还要校验 `WebviewWindow.label()`，不能仅依赖前端隐藏按钮。CSP 默认仅允许自身资源、Tauri IPC 和内置图片。

## 失败与降级

- 桌宠创建或恢复失败：记录诊断并打开工坊；异常重建最多一次。
- 保存位置失败：保留最后成功快照，下一次移动再重试。
- 原显示器丢失：移动到匹配屏或主屏安全区。
- 开机启动注册失败：保持原状态并向工坊/托盘记录错误。
- 内置宠物始终本地可用，不依赖云端。

## 验证与验收

自动检查覆盖状态边界、窗口配置和能力最小化；目标 Windows 的 DPI、多屏、焦点、托盘、退出与长稳必须执行 [阶段 2 测试计划](../testing/phase-2-desktop-shell-test-plan.md)。在证据归档前，本设计为已实现架构，不代表 Gate A 已通过。

## 相关链接

- [系统架构](system-overview.md)
- [安全边界](security-boundaries.md)
- [Tauri Command 登记表](tauri-command-registry.md)
- [ADR-0004](decisions/ADR-0004-overlay-window-model.md)
- [PLAN 阶段 2](../../PLAN.md)
