# Ubuntu / Linux 悬浮球

Linux 适配使用 GTK 3 + WebKitGTK，复用现有悬浮球、详情面板、右键菜单和边缘吸附逻辑。启动时将 GDK 后端限制为 X11；Ubuntu Wayland 会话需要 XWayland。原生 Wayland 暂不支持，因为悬浮球需要全局窗口坐标和程序控制窗口位置，参见 [Tauri 的 Wayland 窗口定位问题](https://github.com/tauri-apps/tauri/issues/14913)。

已接入 Linux 鼠标按键检测，避免按住拖动时被误判为松开；查询在 GTK 主线程执行，失败时沿用前端的拖动恢复逻辑。托盘提供详情、设置、刷新、显示/隐藏及退出操作；托盘创建失败会记录日志，但不会阻止悬浮球启动。GNOME 需要启用 AppIndicator 扩展才能显示托盘；没有托盘时可使用悬浮球右键菜单，再次启动 Metra 可找回被隐藏的悬浮球。

## 开发与构建

先安装 Node.js 24 或更新版本、Rust stable（含 Cargo），以及 Ubuntu 原生依赖：

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf xwayland xdg-utils
npm ci
npm run dev
```

构建 Debian 安装包：

```bash
npm run build:linux
```

产物在 `src-tauri/target/release/bundle/deb/`。可通过 `sudo apt install ./src-tauri/target/release/bundle/deb/*.deb` 安装，再从应用菜单启动 Metra。Linux 目前不启用应用内更新，升级时手动安装新的 `.deb`。

CI 使用 Ubuntu 22.04 编译、运行现有测试并构建 `.deb`。推送与项目版本一致的 `vX.Y.Z` 标签时，Windows、macOS 和 Linux 会并行构建；Linux 的 `Metra-X.Y.Z-linux-x64.deb` 与 `.sha256` 文件会随三端产物一起进入 Release 草稿。任一平台失败、缺少产物或校验和不符都会阻止正式发布。普通分支提交或手动运行工作流只做 CI 检查，不发布新版本。

## 显示环境

- X11 会话直接运行；Wayland 会话必须提供有效的 `DISPLAY` 和 XWayland。
- 如果全局配置了 `GDK_BACKEND=wayland`，使用 `GDK_BACKEND=x11 npm run dev` 或 `GDK_BACKEND=x11 metra`。GDK 的环境变量与允许的后端必须兼容。
- SSH、无桌面会话或禁用 XWayland 的 Wayland 会话不能运行当前悬浮球。
- Cursor 编辑器登录通过 `xdg-open` 打开 `cursor://` 链接，需要 Cursor 正确注册协议处理程序。

## 桌面验收

编译和 CI 不能替代桌面验收。在 Ubuntu X11 和 Ubuntu Wayland/XWayland 上分别检查：

1. 启动后显示透明无边框悬浮球，置顶且不出现在任务栏；单击打开详情，右键打开设置。
2. 按住拖动，停顿超过一秒仍保持拖动；松开后不误开详情，下一次单击正常。
3. 开启边缘吸附，左右两侧吸附、闲置收起、悬停展开以及从收起状态拖动均正常。
4. 多屏和 100% / 200% 缩放下，悬浮球与详情面板定位正确；重启恢复位置。
5. 托盘显示/隐藏、刷新、退出与语言切换正常；关闭 AppIndicator 扩展后悬浮球仍可使用。
6. 开启登录自启后重新登录，确认只出现一个实例；关闭自启后不再自动启动。

本次已通过临时 Node 24 环境完成 TypeScript 检查、Vite 构建，以及悬浮球点击、吸附、可见性、Cursor 登录、国际化、菜单布局、包管理和更新发布契约检查。随后已安装 Node 24.21.0、pnpm 11.22.0、Rust 1.98.1（含 rustfmt / Clippy）及 Ubuntu 原生开发包；TypeScript 检查与 Linux Rust 编译通过，101 项 Rust 测试通过，5 项需要真实账号或发布产物的测试按默认配置跳过。本机 Debian release 打包和正式发布的 Linux 产物校验步骤已通过；GitHub 三端 CI 尚未触发，完整桌面验收仍需按清单进行。

## GTK 小窗口尺寸

Ubuntu 的 GTK/WebKit 在 `resizable=false` 时会按 WebKit 的自然尺寸把悬浮球扩大为 200×200 逻辑像素，导致实际窗口与吸边算法使用的 56×56 / 32×56 尺寸不一致。Linux 悬浮球现在启用 GTK 的程序调整尺寸，并将最小、最大尺寸同时固定为当前目标尺寸；展开和收起时同步更新这两个约束，窗口仍不能被自由缩放。

已在本机 200% 缩放的 XWayland 桌面验证：修复前窗口为 400×400 物理像素，修复后为 112×112，右侧坐标 2960 + 宽度 112 = 屏幕宽度 3072。收起状态也已验证：64×112，右侧坐标 3008 + 宽度 64 = 3072。窗口尺寸更新会等待原生尺寸生效，再重新定位，避免窗口管理器按旧宽度限制右侧坐标。Rust 回归测试、吸边和点击契约检查及 TypeScript 检查均通过；新增覆盖尺寸延迟生效、拖动取消和尺寸超时的测试。拖动、多屏等完整交互仍需按上述清单验收。
