# Metra

Metra 是一个轻量的跨平台桌面气泡，用来查看 Cursor、Codex 和 Claude Code 的登录状态、额度、重置时间、token 与可用费用信息。

## 功能

- Windows / macOS 透明置顶气泡，支持拖动、屏幕边缘吸附和多显示器。
- 自动发现本机 `cursor-agent` / `agent`、`codex` 与 `claude` CLI。
- Codex 使用官方 App Server 获取多额度窗口和 token 汇总。
- Cursor 未登录时可直接进入官方登录流程；该动作不会隐式开启兼容模式，也不会自动安装 CLI。登录后，用户仍需单独确认才会开启个人兼容模式读取精确用量。Ultra 会分别展示 Cursor Models、Other Models、Grok Bot 周额度和 On-Demand 四部分；Team 等旧套餐继续使用原有金额布局。
- Claude Code 通过 `claude auth status --json` 检测登录，并汇总本地保留会话中的 token 数字。使用 Anthropic API Key 登录时，组织管理员还可显式配置 Admin API Key，读取该 Key actor 的官方 UTC 当日 token 与预估费用；接口不提供剩余额度时不会伪造百分比。
- 悬浮球中的三个 Provider 支持独立开关显示、通过六点图标拖动排序，显示字符和标记颜色也可分别自定义；内置 55 色快捷色板，配置保存在 SQLite。
- 左键展开详情；右键设置刷新间隔、开机启动、兼容模式、重新检测或退出。
- 刷新失败保留最后一次成功数据，并明确标记为过期。

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

## Claude Code 官方 API 用量（可选）

Anthropic 的 Claude Code Analytics 接口仅接受组织级 Admin API Key（`sk-ant-admin...`）；普通 Claude API Key（`sk-ant-api...`）不能查询历史用量，个人账号也无法使用 Admin API。详见 [Claude Code Analytics API](https://platform.claude.com/docs/en/manage-claude/claude-code-analytics-api)。

在启动 Metra 的进程环境中设置：

```text
ANTHROPIC_ADMIN_KEY=sk-ant-admin...
METRA_CLAUDE_API_KEY_NAME=Claude Code Key
```

`METRA_CLAUDE_API_KEY_NAME` 对应 Anthropic Console 中的 API Key 名称；当当天报告中只有一个 API Key actor 时可省略，存在多个时必须设置。Claude Code Analytics 只返回 Key 名称、不返回 Key ID，因此应为该 Key 使用组织内唯一的名称；遇到同名 Key 时 Metra 会拒绝展示，以免把其他 Key 的用量混入。修改环境变量后需重启 Metra。

该接口按 UTC 自然日聚合，数据最多延迟约 1 小时，并提供 token 与预估费用而非 Pro/Max 剩余百分比。Admin API Key 具有组织级权限，请只在受信任设备中通过操作系统环境或密钥管理工具注入，并定期轮换；Metra 不提供明文持久化入口。

## 数据与隐私

- SQLite 配置只保存刷新周期、气泡位置、Provider 顺序与显隐、字符与颜色、自启动和兼容模式授权状态。
- Codex 凭据由已安装的 Codex CLI/App Server 管理，Metra 不直接读取或保存。
- Cursor 个人兼容模式默认关闭。开启后只读查询 Cursor 的 `state.vscdb`，令牌仅存在于单次请求内存中并在使用后清除。
- Cursor 网络请求只允许 HTTPS 的 `api2.cursor.sh` 和 `cursor.com`，禁止跨域重定向。
- Claude Code 本地采集调用登录状态命令，并从 `~/.claude/projects` 的 JSONL 中仅反序列化时间戳、消息 ID 和 usage 字段；不会保存会话正文、API Key 或 Base URL。
- `ANTHROPIC_ADMIN_KEY` 只在 Rust 进程内临时使用，不写入 SQLite、不发送到 WebView、不记录日志，也不会传给 Metra 启动的任何子进程。官方请求固定使用 `https://api.anthropic.com`、禁止重定向；响应中的用户 actor 信息会被忽略，只缓存选定 API Key 的聚合数字。
- 应用不记录邮箱、令牌、会话正文或完整接口响应。

## 发布签名

CI 可生成未签名的 Windows x64 与 macOS Universal 产物。正式分发前应在发布环境配置 Windows 代码签名，以及 Apple Developer ID 签名和 notarization 凭据。

## License

MIT
