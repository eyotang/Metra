# Metra

[English](README.md) · **简体中文** · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra 是一个轻量的跨平台桌面气泡，用来查看 Cursor、Codex 和 Claude Code 的登录状态、额度、重置时间、token 与可用费用信息。

## 功能

- Windows / macOS 透明置顶气泡，支持自由拖动和多显示器；屏幕边缘吸附可在右键菜单中开启，默认关闭。
- 开启自动吸边后，首次拖到屏幕边缘并松手就会立即缩成 32 px 半隐藏状态，这次拖动后无需等待闲置；移入、聚焦或点击会立即完整显示，唤醒后的普通再次半隐藏仍保留闲置延时。
- 自动发现本机 `cursor-agent` / `agent`、`codex` 与 `claude` CLI。
- Codex 使用官方 App Server 获取多额度窗口和 token 汇总。
- Cursor 未登录时可直接进入官方登录流程；该动作不会隐式开启兼容模式，也不会自动安装 CLI。登录后，用户仍需单独确认才会开启个人兼容模式读取精确用量。Ultra 会分别展示 Cursor Models、Other Models、Grok Bot 周额度和 On-Demand 四部分；Team 等旧套餐继续使用原有金额布局。
- Claude Code 通过 `claude auth status --json` 检测登录；可用时只读汇总本地会话中的 token 数字。API Key 或自定义网关不提供订阅额度时，界面会如实显示“暂无额度”，不会伪造百分比。
- 悬浮球中的三个 Provider 支持独立开关显示、通过六点图标拖动排序，显示字符和标记颜色也可分别自定义；内置 55 色快捷色板，配置保存在 SQLite。
- 左键展开详情，并通过弹窗中的刷新图标更新用量；右键菜单顶部直接提供语言下拉框，同时可设置刷新间隔、开机启动、兼容模式、重新检测或退出，不再重复放置“立即更新”。
- 刷新失败保留最后一次成功数据，并明确标记为过期。
- 界面支持简体中文（`zh-CN`）、English、日本語与한국어；“自动检测”会跟随操作系统 / 浏览器语言，手动选择后立即生效并在重启后保留。

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="Metra 宣传海报：深色蓝黄光效中的 AI 用量悬浮球与经过脱敏的真实产品界面，展示 Cursor、Codex、Claude Code 和默认 C、X、A 字符。" width="100%">
  </a>
</p>

## 开发

需要 Rust stable、Node.js 22+、npm，以及 Tauri 对应平台的系统依赖。

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

发布构建：

```text
npm run build
```

Windows 产物位于 `src-tauri/target/release` 和 `src-tauri/target/release/bundle`。macOS 通用包使用：

```text
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run build -- --target universal-apple-darwin
```

## 数据与隐私

- SQLite 配置只保存刷新周期、界面语言偏好、气泡完整位置、自动吸边开关、Provider 顺序与显隐、字符与颜色、自启动和兼容模式授权状态；临时半隐藏坐标不会持久化。
- Codex 凭据由已安装的 Codex CLI/App Server 管理，Metra 不直接读取或保存。
- Cursor 个人兼容模式默认关闭。开启后只读查询 Cursor 的 `state.vscdb`，令牌仅存在于单次请求内存中并在使用后清除。
- Cursor 网络请求只允许 HTTPS 的 `api2.cursor.sh` 和 `cursor.com`，禁止跨域重定向。
- Claude Code 采集只调用登录状态命令，并从 `~/.claude/projects` 的 JSONL 中反序列化时间戳、消息 ID 和 usage 数字；不读取消息正文、API Key 或 Base URL。
- 应用不记录邮箱、令牌、会话正文或完整接口响应。

## 发布签名

CI 可生成未签名的 Windows x64 与 macOS Universal 产物。正式分发前应在发布环境配置 Windows 代码签名，以及 Apple Developer ID 签名和 notarization 凭据。

## License

MIT
