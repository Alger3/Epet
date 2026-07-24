# Windows 双角色离线 Alpha 使用与构建

> 状态：Active
> 负责人：桌面端负责人
> 评审人：测试负责人、发布负责人
> 最近更新：2026-07-24
> 更新触发：Windows 构建脚本、安装方式、内置角色、签名或已知限制变化

## 目的

让测试者在 Windows 11 x64 构建、安装和使用 Epet 0.2.0 离线 Alpha，并为 Gate A 留下可复核结果。

## 本版范围

- 内置橘猫“橘子”和原创成年人 Q 版人物“小栎”；
- 角色库切换与 SQLite v3 持久化；
- 透明置顶窗口、拖拽、滚轮缩放、显示/隐藏、暂停和鼠标穿透；
- 托盘、单实例、开机启动、多显示器恢复和 NSIS 安装包；
- 全程离线，不读取或上传用户照片。

照片转 Q 版角色、云端 API、GPU Worker、签名证书和自动更新服务不属于 0.2.0 离线 Alpha。界面会明确显示该限制，不返回伪造的生成成功状态。

## 方式一：在 Windows 本机构建

环境：Windows 11 x64、Node.js 24.15.0、Rust 1.97.1 MSVC、Visual Studio 2022 Build Tools 的“使用 C++ 的桌面开发”工作负载，以及 WebView2 Runtime。

在 PowerShell 中进入仓库根目录：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build-windows.ps1
```

脚本依次执行锁定依赖安装、TypeScript/Rust 检查、测试和 Tauri NSIS 构建。成功后打印安装包绝对路径和 SHA-256，默认目录为：

```text
apps\desktop\src-tauri\target\release\bundle\nsis\
```

仅在依赖已安装或临时排障时使用 `-SkipInstall` / `-SkipTests`；跳过测试的包不能作为 Gate 或发布证据。

## 方式二：GitHub Actions 构建

推送代码后，在 Actions 中手动运行 `Windows installer`，或创建 `v*` 标签。下载 `epet-windows-x64-*` Artifact，核对同包内 `SHA256SUMS.txt` 后再安装。当前工作流产物未做商业代码签名，Windows SmartScreen 可能提示未知发布者。

## 安装后使用

1. 启动 Epet，工坊和透明桌面角色窗口同时出现。
2. 在“内置角色库”选择“橘子”或“小栎”；选择立即生效并在下次启动恢复。
3. 拖动角色改变位置，滚轮改变大小；主窗口可暂停、隐藏或开启鼠标穿透。
4. 开启穿透后通过托盘菜单关闭穿透，避免无法点击角色。
5. 关闭工坊只隐藏主窗口；从托盘选择“退出 Epet”才完全退出。

## Windows 必测清单

- [ ] 两个内置角色均可选择，切换后 Overlay 立即更新且重启恢复。
- [ ] 100%、125%、150%、200% DPI 下角色不模糊到不可用、不跑出工作区。
- [ ] 单屏、双屏、主屏切换和显示器热插拔后位置可恢复。
- [ ] Overlay 不进入任务栏/Alt-Tab 主列表，点击与自主动画不抢键盘焦点。
- [ ] 拖拽、缩放、暂停、显示/隐藏和托盘穿透恢复入口有效。
- [ ] 连续运行 8 小时无崩溃、持续内存增长或明显位置抖动。
- [ ] NSIS 安装、覆盖安装和卸载完成，旧 SQLite v2 状态可迁移到 v3。

结果填写到 [阶段 2 测试计划](../testing/phase-2-desktop-shell-test-plan.md) 和 [Windows 兼容矩阵](../testing/windows-compatibility-matrix.md)。未完成上述实机证据前不得宣称 Gate A 或公开发布通过。

## 回滚与清理

安装失败时保留旧安装包与 `%APPDATA%\com.epet.desktop\epet.sqlite3` 的副本。0.2.0 首次启动会将旧 `active_pet_id` 兼容迁移为 `active_character_id`；不要用旧版本反复覆盖写入已经迁移的数据库。需要完全重置时先退出托盘进程，再备份并删除应用数据目录。
