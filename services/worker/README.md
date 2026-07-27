# Epet CPU/Mock Worker

Worker 通过 Redis `BLPOP` 消费任务，从 MinIO 读取清理后的主照片，并使用 Pillow 生成
一个确定性多帧 Sprite Atlas。它从照片提取稳定配色，然后按主体选择猫咪短毛模板或
Q 版人物模板，将标准部件绑定到骨骼并离线生成 `idle`、`walk`、`sleep`、`tap`、
`drag`、`wake`。JSON 序列化、渲染顺序、PNG 参数、ZIP 条目顺序和时间戳均固定，因此
相同照片、草稿名和主体会得到字节级一致的测试 `.epet`。

启动 API 和基础设施后，在另一个终端运行：

```powershell
npm run dev:worker
```

生成包使用 `.epet` manifest v2，并携带：

- `animation/layers.json`：标准化部件、绑定骨骼、旋转中心与层级；
- `animation/rig.json`：猫咪或人物标准骨骼与锚点；
- `animation/clips.json`：动作帧数、通道、事件和时间/距离相位；
- `animation/render-profile.json`：确定性画布、超采样、排序与 PNG 参数；
- `atlas/pet.json` 与 `atlas/pet.png`：桌面运行时实际加载的烘焙结果。

这不是最终 AI 图像生成器：照片目前只用于派生测试角色配色，不会保留照片外观或完成
身份一致的 Q 版重绘。真实立绘和部件 Mask 将由下一阶段 Provider 生成，再复用本动画流水线。
