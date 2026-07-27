# Epet CPU/Mock Worker

Worker 通过 Redis `BLPOP` 消费任务，从 MinIO 读取清理后的主照片，并使用 Pillow 生成
一个静态 Sprite Atlas。它固定 JSON 序列化、ZIP 条目顺序、时间戳和压缩方式，因此相同
照片与草稿名会得到字节级一致的测试 `.epet`。

启动 API 和基础设施后，在另一个终端运行：

```powershell
npm run dev:worker
```

这不是最终 AI 图像生成器；目前四个动作复用同一个确定性静态帧，用来验证上传、队列、
打包、校验、下载、安装和激活的完整链路。
