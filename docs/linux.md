# Linux 原生运行与打包

桌面界面现在由 GPUI / GPUI-KIT 渲染，不需要 WebKitGTK。为支持悬浮球的全局定位和屏幕边缘吸附，Metra 使用 GPUI 的 X11 后端；Wayland 会话需要 XWayland。系统托盘使用 GTK 3 / AppIndicator，独立的无界面采集器不初始化这些桌面库。

Ubuntu 24.04 开发依赖：

```sh
sudo apt install clang cmake pkg-config libgtk-3-dev libayatana-appindicator3-dev libvulkan-dev libxkbcommon-x11-dev libxcb-xfixes0-dev libxcb-shape0-dev libxcb-randr0-dev libfontconfig1-dev libx11-xcb-dev libwayland-dev libasound2-dev
cargo run
cargo test --locked --all-targets
```

GPUI 需要可用的 Vulkan 驱动。GNOME 需要 AppIndicator 扩展才能展示托盘；未提供托盘的桌面请保留默认悬浮球方式。

```sh
node scripts/package-native.mjs
sudo apt install ./release-assets/Metra-*-linux-x64.deb
```

Linux 通过软件包管理器更新，不会在后台覆盖已安装的二进制。窗口定位、多屏缩放、拖动后半隐藏、托盘交互和退出应在目标桌面上验收。

Omarchy 可以直接使用 Quickshell 原生插件和无界面采集器：

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
node integrations/omarchy/install.mjs --binary target/collector/release/metra
```

详见 [Omarchy 安装说明](../integrations/omarchy/README.md)。
