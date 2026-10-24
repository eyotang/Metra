# Metra

[English](README.md) · **简体中文** · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra 是基于 **Rust、GPUI 和 GPUI-KIT 0.8.0** 的原生桌面用量监控工具，用来查看 Cursor、Codex 和 Claude Code 的登录状态、额度、重置时间、Token 与可用费用信息。界面不再依赖浏览器或 WebView。

- 默认仍为透明置顶悬浮球，支持拖动、多显示器和可选的屏幕边缘吸附；吸边后可缩为 32px 半隐藏状态。
- 所有系统均提供“悬浮球 / 状态栏”展示方式，选择保存到 SQLite，重启恢复。状态栏每 5 秒轮播已启用服务；macOS 显示文字额度，Windows 使用数字托盘图标，Linux 随托盘宿主展示文字或数字。
- 详情展示登录状态、额度窗口、重置时间、Token 和费用。设置保留服务显隐、排序、简称、55 色色板、语言、刷新间隔与开机启动。
- 沿用系统配置目录中 `dev.metra.desktop/settings.db` 的现有配置，以及已确认的 Cursor 兼容模式授权。界面继续支持简体中文、English、日本語和한국어。
- 用量采集和存储已与界面框架解耦。刷新失败保留上一次成功数据并标记过期；Cursor 凭据仅在用户明确确认兼容模式后读取。
- Omarchy Quickshell 插件继续复用原来的 JSON 采集接口，可单独编译不含任何桌面依赖的采集器。

## 开发与验证

安装当前稳定版 Rust，然后运行：

```sh
cargo run
cargo test --locked --all-targets
cargo build --release --locked
```

`cargo run -- --settings` 会直接打开设置；`METRA_CONFIG_DIR` 可以指定独立的测试配置目录。关闭面板后应用继续在后台运行，可通过托盘或设置退出。

macOS 需要 Xcode 命令行工具。Windows 需要 Visual Studio C++ 构建工具和 Windows SDK。Linux 需要 X11 / XWayland、Vulkan 驱动，以及托盘使用的 GTK 3 / AppIndicator，详见 [Linux 说明](docs/linux.md)。

无界面采集器：

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

持续输出模式接受标准输入中的 `refresh` 指令，并在输入关闭时退出。JSON 结构与 [Omarchy 原生插件](integrations/omarchy/README.md) 保持兼容。

## 打包与更新

应用开发使用 Cargo；只有打包辅助脚本和 Omarchy 安装器需要 Node.js 24，无需安装 npm 依赖。

```sh
node scripts/package-native.mjs                 # 当前系统的原生安装包
node scripts/package-native.mjs --universal     # macOS Apple Silicon + Intel DMG
node integrations/omarchy/install.mjs           # 安装 Omarchy 插件
```

产物位于 `release-assets/`。Windows 生成 NSIS 安装包和便携 EXE，打包需要 `makensis`，也可通过 `MAKENSIS` 指定路径；Linux 生成 `.deb`。macOS 包包含完整应用及资源。各系统的窗口、托盘和安装行为仍需对应平台的实际验收。

更新检查继续读取 GitHub 中原有的 `latest.json`。Windows 安装版会在用户点击安装后校验原有 Minisign 签名，再启动安装程序；便携版、macOS 和 Linux 打开检测到版本的 Release 页面进行手动下载。后台检查失败不会打断用量查看。签名工具独立放在 `tools/release-sign`，为兼容现有 CI，继续接受 `TAURI_SIGNING_PRIVATE_KEY` 系列 Secret 名称。

## 代码结构

- `src/app.rs`、`src/appearance.rs`：GPUI 原生窗口、GPUI-KIT 控件与 Metra 视觉样式。
- `src/platform.rs`、`src/tray.rs`：窗口定位、开机启动及系统托盘。
- `src/providers/`、`src/service.rs`：本机账户发现与后台用量采集。
- `src/settings.rs`、`src/model.rs`：SQLite 配置和稳定数据结构。
- `src/headless.rs`、`src/updater.rs`：JSON 输出及独立于界面框架的签名更新。
- `assets/`、`tests/`：嵌入资源及采集、配置、模型和签名验证。

项目使用 [MIT License](LICENSE)。GPUI-KIT 与 GPUI 使用 Apache-2.0；嵌入的 Inter 字体保留 `assets/fonts/` 内的 OFL 许可。
