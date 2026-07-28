# 内置测试宠物

本目录保存阶段 2 无需联网即可运行的内置橘猫资源，用于验证桌面窗口、托盘和恢复能力，不代表阶段 3 的正式 `.epet` 包格式。

## 文件分类

| 文件 | 类型 | 用途 |
|---|---|---|
| `cat-idle.png` | RGBA PNG，1254×1254 | 工坊预览和静态降级 Sprite |
| `animation-atlas.png` | RGBA PNG | 阶段 5.4/5.4.1 猫咪八动作、68 帧内置 Atlas |
| `animation.json` | JSON | Atlas 帧、动作、时长和距离相位运行时定义 |
| `cat-walk-sheet.png` | RGBA PNG | 8 帧真实交替步态来源 |
| `cat-sleep-sheet.png` | RGBA PNG | 8 帧闭眼卧姿来源 |
| `cat-perch-sheet.png` | RGBA PNG | 8 帧边缘露头与前爪趴伏来源 |
| `cat-perch-sleep-sheet.png` | RGBA PNG | 8 帧边缘闭眼伏爪与呼吸来源 |
| `metadata.json` | 机器可读来源记录 | 生成工具、提示词、尺寸、SHA-256 和后处理 |
| `LICENSE.md` | 授权说明 | 当前分发边界与发布前待办 |

## 来源与处理

图片由 OpenAI 内置图像生成工具原创生成，没有使用真实用户照片或第三方参考图。原始提示词与处理方法记录在 `metadata.json`；绿色色键背景经软边缘去除和去绿溢色，Tauri 图标从最终透明图派生。

`cat-idle.png` SHA-256：

```text
69d4d4b78490a4432f13c6aa41ff680980d76b11822469c09672b89a46f3c8e7
```

## 修改规范

走路与睡眠来源图以 `cat-idle.png` 为角色参考，使用 OpenAI 内置图像生成工具生成于
2026-07-27；生成时使用纯绿色色键背景，再通过 imagegen skill 的
`remove_chroma_key.py` 做软遮罩、去绿溢色和 1px 边缘收缩。没有使用用户照片或第三方素材。
最终 Atlas 通过 `python scripts/build-builtin-animation-assets.py` 确定性重建；待机、点击、
拖拽和唤醒使用同一角色资源的受控变换，走路和睡眠使用专用姿态帧。

关键生成约束：保持橙色虎斑、奶油色口鼻/胸口/爪、琥珀色眼睛和卷尾；走路为 4×2
交替步态，睡眠为 4×2 闭眼卧姿；禁止文字、阴影、额外肢体和重复尾巴。

`animation-atlas.png` SHA-256：

```text
733e5d5ba9e8780a8cfd3d7ac3bbf240664185fd6dc2c042891e602953a60d5e
```

- 不手工覆盖二进制后保留旧哈希；任何修改必须提升 `metadata.json` 版本并重算哈希。
- 新增资源必须提供来源、生成/转换步骤、尺寸、许可证和可再分发结论。
- 正式宠物包、动作清单和 Schema 属于阶段 3，应生成到受契约校验的 `.epet` 流程中。
- 仓库级许可证尚未选择，公开分发前必须完成 `LICENSE.md` 中的待办。
