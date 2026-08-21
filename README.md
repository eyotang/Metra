# Metra

**English** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra is a lightweight cross-platform desktop bubble for checking Cursor, Codex, and Claude Code sign-in status, usage limits, reset times, token totals, and available spend.

## Features

- A transparent, always-on-top bubble for Windows and macOS with free dragging and multi-display support. Optional edge docking is available from the context menu and is off by default.
- With auto-dock enabled, the first release at a screen edge immediately enters a 32 px partially hidden peek state—there is no idle delay after that drag. Hovering, focusing, or clicking reveals the full bubble at once; after it is revealed, the normal idle delay still applies before it peeks again.
- Automatically discovers local `cursor-agent` / `agent`, `codex`, and `claude` CLIs.
- Uses the official Codex App Server to read multiple limit windows and token totals.
- Opens Cursor's official sign-in flow when needed without silently enabling compatibility mode or installing a CLI. Reading exact personal usage still requires separate consent. Ultra shows Cursor Models, Other Models, Grok Bot weekly limits, and On-Demand separately; legacy plans such as Team keep the existing monetary layout.
- Checks Claude Code sign-in through `claude auth status --json` and, when available, aggregates token counts from local sessions in read-only mode. When signed in with an Anthropic API key, an organization administrator can explicitly configure an Admin API key to retrieve the selected key actor's official UTC-day token usage and estimated cost. If an API key or custom gateway does not expose remaining quota, Metra reports that no limit data is available instead of inventing a percentage.
- Lets you show or hide each provider, reorder providers with the six-dot handle, and customize both bubble labels and marker colors. The built-in 55-color palette is stored in SQLite with the rest of the settings.
- Opens details with a left click, where the refresh icon updates usage. The context menu starts with a direct language selector and also controls the refresh interval, launch at startup, compatibility mode, rescanning, and quitting; it does not duplicate the refresh action.
- Keeps the last successful result after a refresh failure and clearly marks it as stale.
- Supports English, Simplified Chinese (`zh-CN`), Japanese, and Korean. **Auto detect** follows the OS/browser language; a manual choice takes effect immediately and is remembered after restart.

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="Metra promotional artwork showing the AI usage bubble and privacy-redacted Cursor, Codex, and Claude Code usage panels." width="100%">
  </a>
</p>

## Development

You need Rust stable, Node.js 22+, npm, and the Tauri system dependencies for your platform.

```text
npm install
npm run check
npm run verify:i18n
npm test
npm run dev
```

Create a release build with:

```text
npm run build
```

Windows artifacts are written to `src-tauri/target/release` and `src-tauri/target/release/bundle`. To create a universal macOS build:

```text
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run build -- --target universal-apple-darwin
```

## Optional official Claude Code API usage

Anthropic's Claude Code Analytics API accepts only an organization-level Admin API key (`sk-ant-admin...`). A regular Claude API key (`sk-ant-api...`) cannot query historical usage, and individual accounts cannot use the Admin API. See the [Claude Code Analytics API documentation](https://platform.claude.com/docs/en/manage-claude/claude-code-analytics-api).

Set the following variables in the environment of the process that launches Metra:

```text
ANTHROPIC_ADMIN_KEY=sk-ant-admin...
METRA_CLAUDE_API_KEY_NAME=Claude Code Key
```

`METRA_CLAUDE_API_KEY_NAME` must match the API key name in Anthropic Console. It may be omitted when the daily report contains only one API key actor; it is required when more than one is present. Claude Code Analytics returns key names rather than key IDs, so give the key a name that is unique within the organization. Metra refuses to show results when the name is ambiguous, preventing another key's usage from being mixed in. Restart Metra after changing either environment variable.

The API aggregates usage by UTC calendar day, data may lag by up to about one hour, and it reports tokens and estimated cost rather than a remaining Pro/Max percentage. Admin API keys have organization-level privileges: inject them only on trusted devices through the operating-system environment or a secret manager, and rotate them regularly. Metra provides no plaintext persistence mechanism for these keys.

## Data and privacy

- SQLite settings contain only the refresh interval, interface-language preference, full bubble position, auto-dock toggle, provider order and visibility, custom labels and colors, launch-at-startup preference, and compatibility-mode consent. Temporary peek coordinates are not persisted.
- Codex credentials remain managed by the installed Codex CLI/App Server. Metra does not read or store them directly.
- Cursor personal compatibility mode is off by default. When enabled, Metra reads Cursor's `state.vscdb` without writing to it; the token exists only in memory for a single request and is cleared afterward.
- Cursor network requests are restricted to HTTPS endpoints on `api2.cursor.sh` and `cursor.com`, with cross-origin redirects rejected.
- Local Claude Code collection runs only the sign-in status command and deserializes timestamps, message IDs, and usage fields from JSONL files under `~/.claude/projects`. It does not read session message text or persist API keys or base URLs.
- `ANTHROPIC_ADMIN_KEY` is used only transiently inside the Rust process. It is not written to SQLite, sent to the WebView, logged, or passed to any subprocess started by Metra. Official requests are fixed to `https://api.anthropic.com` with redirects rejected; user actor information in responses is ignored, and only aggregate numbers for the selected API key are cached.
- Metra does not log email addresses, tokens, message content, or complete API responses.

## Release signing

CI can produce unsigned Windows x64 and macOS Universal artifacts. Before official distribution, configure Windows code signing plus Apple Developer ID signing and notarization credentials in the release environment.

## License

MIT
