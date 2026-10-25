# Metra

[English](README.md) · **简体中文** · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra 使用 **Tauri 2、TypeScript 和 Rust** 展示 Cursor、Codex 与 Claude Code 的登录状态、额度、重置时间、Token 和费用。Windows 通过 WebView2 渲染，macOS 使用 WKWebView，Linux 使用 WebKitGTK；Rust 负责账户发现、采集、配置和签名更新。

- 保留 56px 悬浮球、拖动、置顶与吸边；启用吸边后松手即收为 32px 窄胶囊，悬停展开，离开再次收起。支持鼠标和键盘打开详情。
- Windows 使用原生 Acrylic 毛玻璃，macOS 使用深色系统 HUD 毛玻璃；前端补充透明染色、柔和高光和细边框。Windows 拖动期间暂时关闭 Acrylic，松手恢复，减少系统合成器卡顿。Linux X11 恢复兼容合成器的原生模糊提示，并提供半透明回退。
- 状态栏每五秒轮播启用的服务；Windows 使用数字托盘图标，macOS 显示文字，Linux 的显示取决于面板支持。
- 保留服务排序、显示开关、自定义标签、颜色、四种语言、刷新周期、开机启动和 Cursor 兼容模式确认。
- 延用系统配置目录下的 `dev.metra.desktop/settings.db`，迁移旧窗口坐标并限制在显示器工作区域内。Cursor 凭据仍需用户明确同意后才会读取。
- 保留独立 Rust 采集器和 Omarchy 的 Quickshell 插件，JSON 协议不变。

## 开发

安装稳定版 Rust、Node.js 24+ 和对应系统的 Tauri 开发依赖后运行：

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR` 可指定隔离的测试配置目录。关闭面板后继续后台运行，通过托盘或设置退出。Tauri 配置和 Cargo 工程保留在仓库根目录。开发使用 `npm run dev` 启动 Vite 和桌面应用；`npm run build` 会嵌入前端资源，直接 `cargo run` 不会启动前端服务器。

macOS 需要 Xcode 命令行工具。Windows 需要 Visual Studio C++ 构建工具、Windows SDK 和 WebView2（安装包会在缺失时引导安装）。Linux 需要 WebKitGTK 4.1、GTK 3、AppIndicator，定位悬浮窗口需要 X11/XWayland，详见 [Linux 说明](docs/linux.md)。

无界面采集器不依赖 Node、Tauri 或 WebView：

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

持续模式接收标准输入中的 `refresh`，输入关闭后退出。详见 [Omarchy 集成](integrations/omarchy/README.md)。

## 打包与更新

```sh
npm ci
node scripts/package-native.mjs
# macOS 通用安装包：
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

Tauri 负责前端嵌入和平台打包，脚本将产物以原有发布文件名放入 `release-assets/`：Windows 为用户级 NSIS 安装包及便携 EXE，macOS 为 DMG，Linux 为 Debian 包。便携 Windows 版本需要系统已安装 WebView2；NSIS 工具由 Tauri 管理。支持 `CARGO_TARGET_DIR`。

更新继续读取现有 GitHub `latest.json` 并使用原有 Minisign 公钥。Windows 安装版仅在用户点击更新后验证签名并启动安装器；便携版、macOS、Linux 打开对应版本下载页。签名工具继续接受现有 CI Secret 名称。

通用控件使用 Web Awesome（按钮、开关、复选框、输入框、下拉、弹层、进度条），图标使用 Lucide；组件与图标全部本地打包。Lit 保持控件节点稳定，后台刷新保留输入焦点和弹层。每个产品刷新期间显示独立动画，完成后立即更新结果。

CI 在 Windows、macOS、Linux 分别运行 Rust 和 Chromium/WebKit 交互检查，覆盖键盘、缩放、弹层边界、离线资源及逐产品刷新动画，并保存分平台截图。原生毛玻璃、混合 DPI 拖动、托盘与安装升级需要对应操作系统的实机验收。

## 代码结构

- `frontend/`：Lit 页面、Web Awesome 标准控件、Lucide 图标和玻璃表面样式。
- `src/app.rs`、`src/geometry.rs`：Tauri 命令、事件、窗口、拖动与吸边。
- `src/platform.rs`、`src/tray.rs`：系统材质、开机启动、Tauri 托盘。
- `src/providers/`、`src/service.rs`：账户发现和后台采集。
- `src/settings.rs`、`src/model.rs`：SQLite 配置和数据结构。
- `src/headless.rs`、`src/updater.rs`：JSON 采集和签名更新。
- `tauri.conf.json`、`capabilities/`：安装包、内容安全策略及本地窗口权限。
- `tests/`、`tests/ui/`：Rust 契约测试及浏览器交互测试。

项目使用 [MIT License](LICENSE)。Tauri 使用 MIT/Apache-2.0；Inter 字体的 OFL 许可保留在 `assets/fonts/`。
