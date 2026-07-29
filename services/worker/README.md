# Epet Generation Worker

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
- `animation/pose.json`：真实人物关键点来源、置信度和语义部件覆盖率；
- `animation/clips.json`：动作帧数、通道、事件和时间/距离相位；
- `animation/render-profile.json`：确定性画布、超采样、排序与 PNG 参数；
- `atlas/pet.json` 与 `atlas/pet.png`：桌面运行时实际加载的烘焙结果。

MockProvider 仍只从照片派生测试角色配色。OpenVINO 人物路线会先生成透明 Q 版预览；
用户确认后，使用 `human-pose-estimation-0001` 检测关键点，将像素拆为头、躯干、
双臂和双腿，绑定骨骼并烘焙 Atlas，同时写入 `animation/pose.json`。当前身份条件是
分割前景 img2img 参考，并非 IP-Adapter/FaceID；猫咪路线也暂未接入语义关键点模型。
