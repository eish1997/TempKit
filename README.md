# TempKit

QuickGen 桌面临时收纳盒项目：快速收纳图片、使用预设处理、管理便签并取出结果。Windows 宿主使用 Rust / Tauri，界面使用 HTML、CSS 和 JavaScript。

## 开发入口

- [项目文档索引](docs/quickgen/README.md)
- [Windows 构建和运行说明](desktop/README.md)
- [架构](docs/quickgen/architecture.md)
- [协作约定](AGENTS.md)

在 Windows 安装 Rust、Visual Studio C++ 工具链、Node.js 和 WebView2 后，检查 `desktop/scripts/desktop.cmd` 中的本机工具路径，再运行：

```powershell
cd desktop
npm.cmd ci
.\scripts\desktop.cmd build
```

输出为 `desktop/src-tauri/target/release/quickgen-desktop.exe`。默认构建包含本地页面资源，可独立运行；开发入口为 `.\scripts\desktop.cmd dev`。

仓库不包含用户数据、凭据、运行证据、依赖缓存或已编译交付包。文档中的历史本地证据路径仅供追溯，部分文件不随仓库发布。模型服务需自行配置，密钥由宿主安全存储管理。
