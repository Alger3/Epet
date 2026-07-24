# UI Package

工坊使用的无业务可复用 React 组件、主题和可访问性基础设施。

- 允许：Button、Dialog、ProgressStage、ErrorNotice、设计 token、键盘/焦点工具。
- 禁止：生成任务请求、Tauri Command 调用、宠物库状态、页面路由和领域规则。
- 组件必须覆盖默认、加载、禁用、错误与键盘操作；用户文案由业务层传入。
- 组件文件使用 PascalCase，测试和 Story/示例与组件同目录。
