# Epet Local API

FastAPI 是本地生成流程的入口，提供照片创建/上传/确认/删除、任务创建/查询/SSE/取消/
删除、静态 Q 版预览获取与确认，以及生成包下载接口。启动时会自动创建 PostgreSQL 表
和 MinIO bucket。真实 OpenVINO 任务在 `awaiting_portrait_confirmation` 暂停，只有
调用 `POST /v1/generations/{job_id}/portrait/confirm` 后才会继续动画和打包。

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
