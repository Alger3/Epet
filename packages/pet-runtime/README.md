# Pet Runtime

桌宠窗口与工坊预览共用的 TypeScript/PixiJS 运行时。该包不访问网络、数据库、系统窗口或任意文件，只消费已校验的内存模型和发送领域事件。

## 目标结构

```text
pet-runtime/src/
├── loader/       # manifest/atlas 的二次语义校验
├── renderer/     # 纹理缓存、Sprite、锚点、翻转与降帧
├── behavior/     # 表驱动行为状态机和正交运行模式
├── interaction/  # hitbox、点击、拖拽、缩放事件
└── diagnostics/  # 脱敏状态与可复现随机种子
```

`idle` 是必选动作且无 fallback。运行时检查最低版本、帧引用、时长数组、Atlas 边界、锚点和动作 fallback 后才创建纹理。非法转换不能静默播放动画；系统休眠/退出、隐藏/暂停、拖拽、点击、睡眠、走路、待机按该优先级处理。
