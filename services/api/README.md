# Epet Local API

FastAPI 是本地生成流程的入口，提供照片创建/上传/确认/删除、任务创建/查询/SSE/删除，
以及生成包下载接口。启动时会自动创建 PostgreSQL 表和 MinIO bucket。

从仓库根目录启动：

```powershell
npm run setup:python
npm run dev:api
```

健康检查与交互文档：

- `http://127.0.0.1:8000/health`
- `http://127.0.0.1:8000/docs`

当前是 local-only 开发模式，不要求 Bearer Token；生产部署前必须补回设备认证、限流和
对象存储直传策略。
