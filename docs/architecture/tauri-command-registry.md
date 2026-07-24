# Tauri Command 与 Capability 登记表

> 状态：Active
> 负责人：桌面端负责人
> 评审人：安全负责人
> 最近更新：2026-07-24
> 更新触发：新增、删除或扩大任何 Command/capability

所有自定义 Command 都在 Rust 中再次校验调用窗口标签；下表的“调用窗口”是强制授权边界，不是 UI 约定。运行状态变更只有在原生动作和 SQLite 写入成功后才广播。
| `set_active_character` | `workshop` | 白名单格式 `characterId: string` | 已安装 `characters` 行、Overlay、运行快照 | 校验存在性、激活、显示并落库 | 只记录稳定错误类别 | 迁移/权限/切换测试 |

| Command | 调用窗口 | 输入 Schema | 资源范围 | 副作用 | 脱敏日志 | 测试 |
|---|---|---|---|---|---|---|
| `get_runtime_state` | `workshop`, `pet-overlay` | 无 | 单行运行快照 | 无 | 不记录快照内容 | 状态/壳测试 |
| `set_pet_visible` | `workshop` | `visible: bool` | 桌宠窗口、运行快照 | 显示/隐藏并落库 | 只记录错误类别 | Windows 生命周期 |
| `set_paused` | `workshop` | `paused: bool` | 运行快照 | 暂停状态落库并广播 | 只记录错误类别 | 状态测试 |
| `set_click_through` | `workshop` | `clickThrough: bool` | 桌宠窗口、运行快照 | 切换原生鼠标穿透并落库 | 只记录错误类别 | Windows 命中测试 |
| `reset_pet_position` | `workshop` | 无 | 显示器、桌宠窗口、运行快照 | 回到主屏安全区并落库 | 不记录显示器详细拓扑 | 多屏矩阵 |
| `adjust_pet_scale` | `workshop`, `pet-overlay` | 有限 `delta: f64`，绝对值 ≤ 0.25 | 桌宠窗口、运行快照 | 以脚底为锚点缩放并落库 | 只记录校验失败 | 边界/多 DPI |
| `begin_pet_drag` | `pet-overlay` | 无 | 当前桌宠窗口 | 调用原生拖动，250 ms 防抖保存 | 不记录绝对坐标 | 拖动/坐标测试 |
| `show_workshop` | `workshop`, `pet-overlay` | 无 | 工坊窗口 | 显示、取消最小化并聚焦 | 只记录错误类别 | 生命周期测试 |
| `get_autostart_enabled` | `workshop` | 无 | 当前应用开机注册项 | 无 | 不记录注册路径 | Windows 设置测试 |
| `set_autostart_enabled` | `workshop` | `enabled: bool` | 当前应用开机注册项 | 注册/撤销并同步托盘 | 只记录错误类别 | 可撤销性测试 |

`workshop` 与 `pet-overlay` capability 当前都只有 `core:event:default`；不含网络、对话框、Shell、剪贴板、全局快捷键或任意文件访问。开机启动插件不直接暴露给 WebView，由受校验的自定义 Command 封装。禁止使用通配 capability。
