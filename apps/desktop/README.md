# Desktop

Windows 桌面客户端，包含 React 工坊、PixiJS 桌宠 WebView 与 Tauri/Rust 受信核心。阶段 2 桌面壳已实现，目标 Windows 的 Gate A 实机验证尚未完成。

## 当前结构

```text
desktop/
├── src/
│   ├── main.tsx                 # 按 URL 装配窗口入口
│   ├── windows/Workshop.tsx     # 工坊与阶段 2 设置
│   ├── windows/PetOverlay.tsx   # PixiJS 桌宠渲染与拖动
│   └── shared/                  # 运行状态类型与 IPC hook
├── src-tauri/
│   ├── src/commands.rs          # 调用窗口/输入校验和领域入口
│   ├── src/state.rs             # SQLite 单行快照与迁移执行
│   ├── src/windows.rs           # 生命周期、DPI/多屏和 Win32 样式
│   ├── src/tray.rs              # 托盘菜单与状态同步
│   ├── migrations/              # 只追加的 SQLite 迁移
│   └── capabilities/            # 按窗口分离的最小权限
└── tests/e2e/                   # 壳配置与后续 Windows E2E
```

## 边界

- React 只保存页面临时状态；任务、宠物库、设备凭据状态和运行设置从 Rust 读取。
- 桌宠 WebView 默认无网络、对话框、Shell 和任意文件权限。
- Command 不直接堆叠业务逻辑；校验窗口和输入后调用 domain service。
- Pixi ticker 只驱动帧动画；行为计时与系统窗口移动使用独立时钟。
- 本地大文件按内容哈希保存，SQLite 只保存索引和元数据。

## 测试重点

状态机、资源安全加载、坐标转换、DPI/多屏恢复、窗口生命周期、能力越权、离线恢复和数据库迁移。Windows 11 结果是发布阻断项，Windows 10 仅记录兼容性。

## 工具链

- Node.js 24.15.0 / npm 11；
- Rust 1.97.1 stable；
- Tauri 2.11、React 19、TypeScript 7、PixiJS 8、Vite 8；
- SQLite 由 `rusqlite` bundled feature 构建，运行时不依赖用户另装 SQLite。

版本以根目录的版本文件、`package-lock.json` 和 `Cargo.lock` 为准，不从本文复制升级。

## 开发命令

在仓库根目录执行：

```bash
npm install
npm run dev:web
npm run test
npm run test:e2e
```

Windows 完整桌面开发：

```powershell
npm run dev:desktop
npm run build:desktop
```

Ubuntu 24.04 若要本机构建 Tauri，需先安装：

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libdbus-1-dev \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

浏览器预览不会模拟置顶、穿透、系统托盘、不抢焦点、Win32 工作区或开机启动，不能作为 Gate A 证据。

## 状态与迁移

应用数据目录下的 `epet.sqlite3` 保存运行快照。迁移按 `0001-*.sql` 递增并只追加；已进入发布版本的迁移不得编辑或重排。Schema v2 同时保存物理坐标和脚底归一化锚点，React 不自行换算 DPI。

## 相关文档

- [桌面壳架构](../../docs/architecture/desktop-shell.md)
- [Command 登记](../../docs/architecture/tauri-command-registry.md)
- [阶段 2 测试计划](../../docs/testing/phase-2-desktop-shell-test-plan.md)
