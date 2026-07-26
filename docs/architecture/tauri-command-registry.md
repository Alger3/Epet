# Tauri Command 与 Capability 登记表

> 状态：Active
> 负责人：桌面端负责人
> 评审人：安全负责人
> 最近更新：2026-07-26
> 更新触发：新增、删除或扩大任何 Command/capability

所有自定义 Command 都在 Rust 中再次校验调用窗口标签；下表的“调用窗口”是强制授权边界，不是 UI 约定。运行状态变更只有在原生动作和 SQLite 写入成功后才广播。

| Command | 调用窗口 | 输入 Schema | 资源范围 | 副作用 | 脱敏日志 | 测试 |
|---|---|---|---|---|---|---|
| `get_runtime_state` | `workshop`, `pet-overlay` | 无 | 单行运行快照 | 无 | 不记录快照内容 | 状态/壳测试 |
| `get_workshop_snapshot` | `workshop` | 无 | 本地草稿、清洗照片和任务快照 | 只读 | 不记录照片内容 | SQLite/恢复测试 |
| `create_character_draft` | `workshop` | 主体类型、人物授权确认 | SQLite、单个草稿目录 | 创建不可变主体类型草稿并记录授权版本 | 不记录授权主体身份 | 草稿/授权测试 |
| `save_draft_photo` | `workshop` | 草稿 ID、槽位、清洗后图片、归一化裁剪 | 单个草稿照片目录、SQLite | 限额复解码、重新编码、哈希并原子更新槽位 | 不记录文件名或图片内容 | 图片安全/槽位回归测试 |
| `remove_draft_photo` | `workshop` | 草稿 ID、照片槽位 | 单个草稿照片和索引 | 事务删除槽位；无引用时清理内容寻址文件 | 只记录错误类别 | 槽位独立性测试 |
| `start_draft_generation` | `workshop` | 草稿 ID | 单个草稿任务状态 | 阶段 5 未配置时持久化可重试 `service_unavailable` | 不记录图片内容 | 状态恢复测试 |
| `cancel_character_draft` | `workshop` | 草稿 ID | 单个草稿照片和任务状态 | 原子取消并清理照片，可在中断后恢复 | 只记录稳定 ID/错误类别 | 取消恢复测试 |
| `delete_character_draft` | `workshop` | 草稿 ID | 单个草稿目录和索引 | 移入回收目录、事务删除、失败回滚 | 只记录稳定 ID/错误类别 | 删除恢复测试 |
| `rename_installed_character` | `workshop` | 角色 ID、自定义名称 | 单个角色索引 | 更新本地显示名 | 不记录新名称 | 输入边界测试 |
| `inspect_pet_package` | `workshop` | 本地 `.epet` 路径、可选 SHA-256 | 用户指定的单个包文件 | 只读并返回校验摘要 | 不记录文件内容 | 包安全语料 |
| `get_character_definition` | `workshop`, `pet-overlay` | 安全角色 ID | 当前 `.epet` 包和角色索引 | 完整性复验并返回运行时 Atlas/动作/命中区定义 | 不记录资源内容 | 安装到运行时测试 |
| `list_character_library` | `workshop` | 无 | `characters` / `character_versions` 索引 | 只读 | 不记录角色清单 | SQLite 迁移/索引测试 |
| `install_pet_package_from_url` | `workshop` | HTTPS URL、必选小写 SHA-256 | 应用缓存临时文件、角色库、SQLite | 限流下载、完整校验、原子安装并更新索引 | 签名 URL 不落库、不记录 | 安全/安装/回滚测试 |
| `install_local_pet_package` | `workshop` | 本地 `.epet` 路径、可选 SHA-256 | 用户指定包、角色库、SQLite | 完整校验、原子安装并更新索引 | 不记录文件内容 | 安装/回滚测试 |
| `activate_character_version` | `workshop` | 安全角色 ID、SHA-256 | 单个角色版本索引 | 切换当前包版本，旧版本保留 | 只记录错误类别 | 版本回滚测试 |
| `delete_character_version` | `workshop` | 安全角色 ID、SHA-256 | 非当前旧版本目录与索引 | 删除指定旧版本 | 只记录稳定 ID/错误类别 | 版本删除测试 |
| `delete_installed_character` | `workshop` | 安全角色 ID | 非内置角色及全部版本 | 活动角色先隐藏并切回内置角色，再以回收目录和事务删除；失败可恢复 | 只记录稳定 ID/错误类别 | 删除/外键/恢复测试 |
| `set_active_character` | `workshop` | 白名单格式 `characterId: string` | 当前已接入渲染器的内置 `characters` 行、Overlay、运行快照 | 校验运行时可用性、激活、显示并落库 | 只记录稳定错误类别 | 迁移/权限/切换测试 |
| `set_pet_visible` | `workshop` | `visible: bool` | 桌宠窗口、运行快照 | 显示/隐藏并落库 | 只记录错误类别 | Windows 生命周期 |
| `set_paused` | `workshop` | `paused: bool` | 运行快照 | 暂停状态落库并广播 | 只记录错误类别 | 状态测试 |
| `set_click_through` | `workshop` | `clickThrough: bool` | 桌宠窗口、运行快照 | 切换原生鼠标穿透并落库 | 只记录错误类别 | Windows 命中测试 |
| `set_always_on_top` | `workshop` | `alwaysOnTop: bool` | 桌宠窗口、运行快照 | 切换原生窗口层级并落库 | 只记录错误类别 | Windows 层级/迁移测试 |
| `set_autonomous_movement` | `workshop` | `enabled: bool` | 桌宠窗口、运行快照 | 启停低频移动循环并落库 | 不记录绝对坐标 | 状态/边界/多屏测试 |
| `set_sleep_after_minutes` | `workshop` | `0/1/5/10/20/30` 分钟 | 运行快照 | 更新无操作睡眠阈值 | 不记录输入活动 | 状态/迁移测试 |
| `reset_pet_position` | `workshop` | 无 | 显示器、桌宠窗口、运行快照 | 回到主屏安全区并落库 | 不记录显示器详细拓扑 | 多屏矩阵 |
| `adjust_pet_scale` | `workshop`, `pet-overlay` | 有限 `delta: f64`，绝对值 ≤ 0.25 | 桌宠窗口、运行快照 | 以脚底为锚点缩放并落库 | 只记录校验失败 | 边界/多 DPI |
| `begin_pet_drag` | `pet-overlay` | 无 | 当前桌宠窗口 | 调用原生拖动，250 ms 防抖保存 | 不记录绝对坐标 | 拖动/坐标测试 |
| `trigger_pet_tap` | `pet-overlay` | 无 | 行为状态与唤醒点击计数 | 点击反馈；睡眠中三击唤醒 | 不记录鼠标位置 | 状态机测试 |
| `restore_pet_focus` | `pet-overlay` | 无 | 前台窗口句柄 | 恢复桌宠点击前的前台窗口 | 不持久化句柄 | 焦点探针 |
| `show_workshop` | `workshop`, `pet-overlay` | 无 | 工坊窗口 | 显示、取消最小化并聚焦 | 只记录错误类别 | 生命周期测试 |
| `get_autostart_enabled` | `workshop` | 无 | 当前应用开机注册项 | 无 | 不记录注册路径 | Windows 设置测试 |
| `set_autostart_enabled` | `workshop` | `enabled: bool` | 当前应用开机注册项 | 注册/撤销并同步托盘 | 只记录错误类别 | 可撤销性测试 |

`workshop` 与 `pet-overlay` capability 当前都只有 `core:event:default`；不含直接网络、对话框、Shell、剪贴板、全局快捷键或任意文件访问。HTTPS 角色包下载和开机启动插件都不直接暴露给 WebView，只能通过上表中受窗口标签与输入约束的 Rust Command。禁止使用通配 capability。
