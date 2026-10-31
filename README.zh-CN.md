# Metra

[English](README.md) · **简体中文** · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md)

## 软件介绍

**AI 编程用量，一眼掌握。**

把 Cursor、Codex 和 Claude Code 的额度、Token 与费用留在桌面一角。支持 **macOS、Windows 和 Linux**。

- **随时看，不打断。** 悬浮球自由拖动、吸边收起，也可切换状态栏模式。
- **三个工具，一处查看。** 快速了解可用额度、重置时间和使用情况。
- **按你的习惯来。** 自定义服务顺序、标签、颜色和刷新周期；支持七种界面语言与自动识别。

| 用量详情 | 显示设置 |
| :---: | :---: |
| <img src="docs/assets/readme/usage-demo.png" alt="用量详情" width="320"> | <img src="docs/assets/readme/settings-demo.png" alt="显示设置" width="320"> |

当前界面，使用模拟用量展示，不含真实账号信息。可显示的数据取决于服务及登录方式。

## 使用说明

**[下载最新版本](https://github.com/eyotang/Metra/releases/latest)**

| 系统 | 选择文件 | 安装方式 |
| --- | --- | --- |
| macOS | `*-macos-universal.dmg` | 打开 DMG，将 Metra 拖入 Applications。 |
| Windows x64 | `*-windows-x64-setup.exe` | 运行安装包；也可选择便携 EXE（需已安装 WebView2）。 |
| Linux x64 | `*-linux-x64.deb` | 安装软件包：`sudo apt install ./Metra-*-linux-x64.deb`。 |

macOS：将应用移入 Applications 后，首次启动前执行，清除扩展属性（包括隔离标记）：

```sh
xattr -cr /Applications/Metra.app
```

打开 Metra，登录你使用的工具，点击悬浮球即可查看用量。需要读取 Cursor 精确用量时，在设置中开启 **Cursor 兼容模式**，并确认读取本地凭据的授权。拖动即可调整位置，开启吸边后可收在屏幕边缘。

在设置中调整语言、刷新周期和开机启动。关闭面板后继续后台运行；退出请使用设置或托盘菜单。

[Linux 配置](docs/linux.md) · [Omarchy 插件](integrations/omarchy/README.md)

## 开发贡献指南

准备 **Node.js 24+**、**稳定版 Rust** 和下表中的系统工具。Fork 本仓库并克隆你的 Fork，在项目根目录运行：

| 系统 | 开发工具 |
| --- | --- |
| macOS | Xcode 命令行工具 |
| Windows | Visual Studio C++ 构建工具、Windows SDK、WebView2 |
| Linux | WebKitGTK 4.1、GTK 3、AppIndicator；悬浮窗口需要 X11/XWayland |

```sh
npm ci
npm run dev
```

提交 PR 前验证：

```sh
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
```

修改界面从 `frontend/` 入手，采集逻辑看 `src/providers/`，刷新逻辑看 `src/service.rs`。使用 `METRA_CONFIG_DIR` 隔离测试配置。

每个 PR 聚焦一个问题，说明改动原因、效果和验证方式；界面改动附截图。涉及文案或说明时，同步更新七种语言的翻译和 README。

macOS 本地安装：先运行 `npm run build`，再运行 `npm run install:local`。

[构建、打包与采集器参考](docs/development.md) · [Issues](https://github.com/eyotang/Metra/issues) · [Pull requests](https://github.com/eyotang/Metra/pulls)

[采用 MIT 许可证。](LICENSE)
