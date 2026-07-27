# 本地照片生成开发

## 首次安装

```powershell
npm install
npm run setup:python
npm run dev:infra
```

等待 `docker compose -f infra/docker/compose.yaml ps` 显示三个服务 healthy。

## 日常启动

打开三个 PowerShell 终端：

```powershell
npm run dev:api
```

```powershell
npm run dev:worker
```

```powershell
npm run dev:desktop
```

桌面端从 `VITE_EPET_API_BASE_URL` 读取服务地址，未设置时使用
`http://127.0.0.1:8000`。创建自定义桌宠后，桌面端会：

1. 将 Rust 已清理、重编码并去除元数据的照片上传到 API；
2. 创建 Redis 队列任务，并用 SSE 接收更新、用轮询补偿断线；
3. 获取产物 URL 和 SHA-256；
4. 由 Rust 下载、校验并安装 `.epet`；
5. 将新角色设为当前桌宠，并把本地草稿标记为完成。

## 本地端口与数据

| 服务 | 地址 | 用途 |
| --- | --- | --- |
| FastAPI | `127.0.0.1:8000` | 桌面接口、SSE、下载代理 |
| PostgreSQL | `127.0.0.1:5432` | 上传和任务权威状态 |
| Redis | `127.0.0.1:6379` | Worker 队列 |
| MinIO | `127.0.0.1:9000` | 照片和 `.epet` 对象 |
| MinIO Console | `127.0.0.1:9001` | 本地对象检查 |

默认凭据只用于回环地址上的本地开发，保存在 `.env.example` 和 Compose 中；不可用于公网。
