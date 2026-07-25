# Pet Runtime

桌宠窗口与工坊预览共用的运行时边界。该层不访问网络、数据库、系统窗口或任意文件，只消费 Rust 已校验的内存模型并发送领域事件。

## 当前实现（0.2.0 工作区）

- Rust 安全加载器：`apps/desktop/src-tauri/src/package.rs`
- Rust 行为状态机：`apps/desktop/src-tauri/src/behavior.rs`
- TypeScript Atlas 模型与帧推进：`apps/desktop/src/shared/sprite-atlas.ts`
- Canvas 2D 渲染和静态 PNG 降级：`apps/desktop/src/windows/SpriteAtlas.tsx`
- 原生碰撞命中、移动边界与显示器恢复：`apps/desktop/src-tauri/src/windows.rs`

工坊可以通过受限的 `inspect_pet_package` Command 对本地 `.epet` 执行黑盒加载并获得脱敏摘要。当前实现只完成安全校验和内存加载；下载、内容哈希目录的原子安装、角色库事务与导入 UI 仍是后续工作。

## 目标结构

```text
当前代码后续收敛到 pet-runtime/src/
├── loader/       # manifest/atlas 的二次语义校验
├── renderer/     # 纹理缓存、Sprite、锚点、翻转与降帧
├── behavior/     # 表驱动行为状态机和正交运行模式
├── interaction/  # hitbox、点击、拖拽、缩放事件
└── diagnostics/  # 脱敏状态与可复现随机种子
```

`idle` 是必选动作且无 fallback。运行时检查最低版本、帧引用、时长数组、Atlas 边界、锚点和动作 fallback 后才创建纹理。当前 Windows 透明窗口优先使用 Canvas 2D，静态 PNG 始终作为降级保险；后续采用 PixiJS 时不得改变状态语义。非法转换不能静默播放动画；系统休眠/退出、隐藏/暂停、拖拽、点击、睡眠、走路、待机按该优先级处理。
