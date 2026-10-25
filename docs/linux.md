# Linux

桌面界面由 Tauri 2 / WebKitGTK 4.1 渲染。悬浮球定位与吸边依赖 X11 / XWayland，Wayland 会话请使用 `GDK_BACKEND=x11 npm run dev`；Quickshell 插件则由宿主原生定位。无界面采集器不初始化任何桌面库。

Ubuntu 24.04 开发环境：

```sh
sudo apt install build-essential clang cmake pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libxdo-dev
npm ci
GDK_BACKEND=x11 npm run dev
npm run frontend:build
cargo test --locked --all-targets
node scripts/package-native.mjs
```

不再依赖 Vulkan。GNOME 需要 AppIndicator 扩展显示托盘；未提供托盘的桌面请保留默认悬浮球。Linux X11 为兼容的合成器（如 KDE）设置原生模糊提示，并使用圆角区域匹配悬浮球和面板；不支持该提示的桌面仍使用半透明底色。Windows 和 macOS 使用各自的系统材质。

## Omarchy

[Omarchy 集成](../integrations/omarchy/README.md)使用原生 Quickshell 组件和独立 JSON 采集器，沿用既有 SQLite 配置。无需 WebKitGTK、GTK、XWayland 或 Node 前端依赖即可构建采集器：

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
METRA_COLLECTOR_BINARY=target/collector/release/metra node scripts/package-omarchy.mjs
```

`--usage-json` 输出单个快照；`--watch-json` 持续输出，并接受 stdin 的 `refresh` 指令。stdin 关闭后退出。可用 `node scripts/verify-omarchy.mjs target/collector/release/metra` 检查协议。
