# 内置原创 Q 版人物

本目录保存可随 Epet 离线版分发的原创人物桌宠素材。角色不对应任何真实人物或公众人物，用于验证 `human_avatar` 角色切换、透明窗口、持久化和 Windows 安装包。

- `human-avatar.png`：RGBA 透明标准立绘和静态降级图。
- `animation-atlas.png`：阶段 5.4 人物六动作、52 帧内置 Atlas。
- `animation.json`：Atlas 帧、动作、时长和距离相位运行时定义。
- `human-walk-sheet.png`：8 帧手脚交替、围巾跟随的步态来源。
- `human-sleep-sheet.png`：8 帧闭眼蜷坐/侧卧来源。
- `metadata.json`：机器可读的主体类型、尺寸、哈希和生成说明。
- `LICENSE.md`：素材来源与再分发边界。

原始绿幕中间文件不进入仓库或安装包。替换成新素材时必须更新哈希、尺寸、许可说明和视觉回归证据。

走路与睡眠来源图以 `human-avatar.png` 为角色参考，使用 OpenAI 内置图像生成工具生成于
2026-07-27；生成时使用纯绿色色键背景，再通过 imagegen skill 的
`remove_chroma_key.py` 做软遮罩、去绿溢色和 1px 边缘收缩。没有使用用户照片或第三方素材。
生成约束保持短棕发、圆框眼镜、锈红围巾、深绿外套、棕裤和短靴；最终 Atlas 通过
`python scripts/build-builtin-animation-assets.py` 确定性重建。

`animation-atlas.png` SHA-256：

```text
add29f28f7a6cef189289ac6b195401aa5502718e6b50670467497c5f92de53b
```
