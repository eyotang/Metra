# Linux

桌面界面使用 GPUI / GPUI Kit，通过 Vulkan 渲染，无需 WebKitGTK 或 GTK。
悬浮球定位与吸边使用 X11 / XWayland；Wayland 会话中存在 `DISPLAY` 时程序
自动选择 XWayland。纯 Wayland 的悬浮定位尚不支持，Quickshell 插件可由宿主
原生定位。无界面采集器不初始化桌面库。

Ubuntu 24.04 开发环境：

```sh
sudo apt install build-essential clang cmake pkg-config libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libssl-dev libasound2-dev libvulkan-dev mesa-vulkan-drivers
npm ci
npm run dev
npm run check
npm test
npm run test:ui
npm run package
```

安装 Debian 包：`sudo apt install ./Metra-*-linux-x64.deb`。系统需要可用的 Vulkan
驱动。托盘使用 StatusNotifierItem；GNOME 需要支持该协议的扩展。未提供托盘的
桌面会保留悬浮球作为入口。原生界面使用不透明内容和透明窗口圆角。

## Omarchy

[Omarchy 集成](../integrations/omarchy/README.md)使用原生 Quickshell 组件和独立 JSON 采集器，沿用既有 SQLite 配置。无需 WebKitGTK、GTK、XWayland 或 Node 前端依赖即可构建采集器：

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
METRA_COLLECTOR_BINARY=target/collector/release/metra node scripts/package-omarchy.mjs
```

`--usage-json` 输出单个快照；`--watch-json` 持续输出，并接受 stdin 的 `refresh` 指令。stdin 关闭后退出。可用 `node scripts/verify-omarchy.mjs target/collector/release/metra` 检查协议。
