# Epet 本地基础设施

`compose.yaml` 只在本机回环地址开放服务：

- PostgreSQL：`127.0.0.1:5432`
- Redis：`127.0.0.1:6379`
- MinIO S3 API：`127.0.0.1:9000`
- MinIO 控制台：`http://127.0.0.1:9001`

启动和检查：

```powershell
npm run dev:infra
docker compose -f infra/docker/compose.yaml ps
```

停止服务但保留数据：

```powershell
npm run stop:infra
```

数据位于 Docker named volumes。只有明确需要清空全部本地开发数据时，才应在 `down`
后手动删除这些 volumes。
