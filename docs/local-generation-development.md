# 本地照片生成开发

## 首次安装

```powershell
npm install
npm run setup:python
npm run dev:infra
```

等待 `docker compose -f infra/docker/compose.yaml ps` 显示三个服务 healthy。

如果要使用 Intel Arc 生成真实静态 Q 版预览，首次还需要执行：

```powershell
npm run prepare:model:openvino
```

该命令按 `services/worker/model-sources.json` 的固定 revision 下载约 2.3 GB
SD1.5 FP16 权重和 LoRA，校验 SHA-256、融合 LoRA，再原子导出 OpenVINO IR。
中断后可再次执行并续传。LoRA 目前仅批准本地技术验证，不能随安装包发布。

## 日常启动

打开三个 PowerShell 终端：

```powershell
npm run dev:api
```

```powershell
$env:EPET_GENERATION_PROVIDER = "openvino-gpu" # 真实预览；省略时为 mock
npm run dev:worker
```

```powershell
npm run dev:desktop
```

桌面端从 `VITE_EPET_API_BASE_URL` 读取服务地址，未设置时使用
`http://127.0.0.1:8000`。创建自定义桌宠后，桌面端会：

1. 将 Rust 已清理、重编码并去除元数据的照片上传到 API；
2. 创建 Redis 队列任务，并用 SSE 接收更新、用轮询补偿断线；
3. OpenVINO 路线先下载并校验静态 Q 版 PNG，显示加载时间、推理时间和进程内存峰值；
4. 用户确认预览后，人物任务检测 18 点姿态，拆分头、躯干、双臂和双腿，绑定骨骼，
   再生成动作 Atlas 并打包 `.epet`；
5. 获取产物 URL 和 SHA-256；
6. 由 Rust 下载、校验并安装 `.epet`；
7. 将新角色设为当前桌宠，并把本地草稿标记为完成。

取消操作会写入 PostgreSQL；Worker 在扩散步骤回调中检查取消状态。基准命令：

```powershell
npm run benchmark:model:openvino -- --input C:\path\to\photo.png
```

人物产物使用 `.epet 2.1.0`，并额外携带 `animation/pose.json`，记录每个关键点是否
由 OpenVINO 检出或由模板补全，以及各语义部件的像素覆盖率。猫咪目前仍使用整体形变
Atlas；猫咪关键点模型与身份一致性增强仍属于后续阶段。

## 本地端口与数据

| 服务 | 地址 | 用途 |
| --- | --- | --- |
| FastAPI | `127.0.0.1:8000` | 桌面接口、SSE、下载代理 |
| PostgreSQL | `127.0.0.1:5432` | 上传和任务权威状态 |
| Redis | `127.0.0.1:6379` | Worker 队列 |
| MinIO | `127.0.0.1:9000` | 照片和 `.epet` 对象 |
| MinIO Console | `127.0.0.1:9001` | 本地对象检查 |

默认凭据只用于回环地址上的本地开发，保存在 `.env.example` 和 Compose 中；不可用于公网。
