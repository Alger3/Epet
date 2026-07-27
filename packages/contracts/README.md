# Contracts

客户端、API 与 Worker 的共享协议事实来源。该包不得依赖任何业务模块。

## 目录

```text
contracts/
├── openapi/openapi.yaml       # HTTP/SSE API
├── schemas/                   # 宠物包、Atlas、pet_spec、事件
├── errors/error-codes.yaml    # 稳定机器错误码注册表
├── examples/                  # 合法源样例和非法安全样例
└── generated/                 # 从契约生成的 TS/Python 类型（后续创建）
```

## 版本规则

- OpenAPI 路径以 `/v1` 分组；兼容增加不改变主版本，删除/改义需新 API 版本。
- JSON Schema 使用 2020-12，每份包含稳定 `$id`、`title` 和 `schema_version`。
- `.epet` 的 `schema_version` 控制结构兼容，`package_version` 控制单个宠物包修订，`min_runtime_version` 控制加载门槛。运行时兼容 v1 静态包；v2 增加 `subject_kind`、LayerBundle、Rig、AnimationClips、RenderProfile 和距离相位动作。
- 生成类型只改来源契约后重新生成，禁止人工编辑。

## 语义校验

Schema 只能检查结构。加载器还必须检查：manifest 动作帧与时长数量相同、所有帧存在、Atlas 矩形不越界、文件清单与压缩包完全一致、哈希/大小匹配、路径安全、许可证存在和资源上限。v2 还会检查主体与 Rig 一致、部件绑定引用、Clip 与动作帧数/相位一致，以及 RenderProfile 与画布一致。

破坏性变更必须带 ADR、迁移策略、旧客户端行为和契约测试。当前契约是阶段 1 基线，未经实现/测试前状态视为 Draft。
