# API Service

FastAPI 控制面，负责设备鉴权、上传、生成命令、任务快照、SSE 通知入口、资源下载授权和删除请求。

## 目标结构

```text
api/
├── app/
│   ├── routes/          # HTTP/SSE 协议适配，不直连存储或队列
│   ├── services/        # 归属、幂等、状态转换和业务规则
│   ├── repositories/    # PostgreSQL 持久化
│   ├── adapters/        # Redis、对象存储、令牌、时钟
│   ├── models/          # 内部领域模型，不复制生成的契约类型
│   └── main.py          # 装配和生命周期
├── migrations/          # 只追加的版本化迁移
└── tests/               # 单元、集成、越权和迁移测试
```

## 强制规则

- OpenAPI 是 HTTP 契约事实来源；先改契约和迁移，再实现 Route → Service → Repository。
- 每个资源同时验证令牌主体和设备归属；对外 404/403 策略不得泄露资源存在性。
- 所有创建、确认、重试、取消和删除命令幂等，冲突请求体返回稳定错误码。
- PostgreSQL 快照先提交，之后再向 Redis 发布通知。
- 数据库只保存对象键，不保存长期签名 URL；日志不记录对象键、令牌或完整路径。

本地开发必须提供 CPU/Mock Worker 路径，使没有 GPU 或云资源的开发者可调试完整协议。
