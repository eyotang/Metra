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
- Starting with v0.1.39, supported distributions—including Windows Portable—check GitHub Releases for signed updates shortly after launch and every 24 hours while running. Windows installer builds update in-app after confirmation; macOS and Windows Portable open the detected version's Release page for manual download.
- Supports English, Simplified Chinese (`zh-CN`), Japanese, and Korean. **Auto detect** follows the OS/browser language; a manual choice takes effect immediately and is remembered after restart.

<p align="center">
  <a href="docs/assets/readme/metra-readme-hero.png">
    <img src="docs/assets/readme/metra-readme-hero-1920.jpg" alt="Metra promotional artwork showing the AI usage bubble and privacy-redacted Cursor, Codex, and Claude Code usage panels." width="100%">
  </a>
</p>

## Development

You need Rust stable, Node.js 22.13+, npm or pnpm, and the Tauri system dependencies for your platform.

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

On Windows, fully quit every running Metra instance first, then open PowerShell in the repository root and run. The script stops with a clear message while Metra is running, preventing locked output files:

```powershell
npm install --global pnpm@11.22.0
pnpm install --frozen-lockfile
pnpm run build:portable
```

The project pins `pnpm@11.22.0` in `package.json`, and the first command installs that exact version without depending on Corepack. The checked-in `pnpm-workspace.yaml` explicitly includes the root package, preventing pnpm's `packages field missing or empty` error. The project Cargo config also disables user-level rustc wrappers, so a global `sccache` setting is not used. The portable executable is written to `src-tauri/target/release/Metra-<version>-portable.exe`. To create Windows installers instead, run `pnpm run build`; those artifacts are written to `src-tauri/target/release/bundle`.

For a universal macOS build, use the dedicated command:

```text
pnpm run build:macos-universal
# or
npm run build:macos-universal
```

This command pins Cargo and rustc to the same rustup `stable` toolchain, installs both macOS targets, disables `sccache`, and invokes Tauri for `universal-apple-darwin`. Running `pnpm run build -- --target universal-apple-darwin` manually adds an extra `--`; it can reach Cargo/rustc and make the universal target be parsed as a Rust target specification. A build can also fail when Homebrew Rust shadows rustup and Cargo and rustc come from different toolchains. The dedicated command avoids both problems.

## Update checks and installation (v0.1.39 and later)

Supported distributions, including Windows Portable, read `latest.json` from the latest GitHub Release shortly after launch and every 24 hours while running. If GitHub cannot be reached, Metra stays quiet and waits for the next scheduled check instead of displaying a connection error or retrying repeatedly. When an update is available, Metra shows the new version and an update prompt; the selected update action begins only after the user explicitly confirms.

Because v0.1.38 does not contain the updater, existing installed and Portable users must move to v0.1.39 manually once. After that bootstrap update, all supported distributions can detect subsequent releases automatically: Windows NSIS installations can update in-app, while macOS and Windows Portable continue with manual downloads.

Windows NSIS installations verify the Tauri updater signature, download the installer, and update in-app. On macOS, Metra still detects new versions through `latest.json`, but the action opens the detected version's GitHub Release page so the user can download the signed and notarized DMG manually. In-place macOS installation is temporarily disabled while the upstream [Tauri updater safety issue #3505](https://github.com/tauri-apps/plugins-workspace/issues/3505) remains unresolved. Windows Portable also detects new versions automatically, but it never downloads or replaces the running executable; its action opens the detected version's Release page so the user can download the new Portable EXE manually.

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

Pushing a `v*` tag triggers the release-artifact workflow. Official macOS distribution requires `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` in the release environment.

Updater artifacts require a matching signing-key pair. Never commit the private key: upload it to the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret and keep a separate secure offline backup. When the key is encrypted, store its password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; keep the public key in `src-tauri/tauri.conf.json`. Clients cannot accept future updates if the private key is lost. The release build signs the Windows NSIS updater and macOS `.app.tar.gz` bundle, while the DMG and Windows portable executable remain manual-download artifacts.

The workflow rejects macOS artifacts unless the bundle is Universal for both `arm64` and `x86_64`, signed with `Developer ID Application`, contains the hardened runtime flag, passes Gatekeeper assessment, and validates its notarization ticket for both the `.app` and `.dmg`.

CI creates the GitHub Release as a draft first, uploads the installers, signatures, manual-download packages, and `latest.json`, then verifies that the manifest, URLs, versions, signatures, and required assets agree. Only after every validation passes does CI publish the draft and mark it as the latest release, so clients never discover a partially assembled update.

Local ad-hoc macOS builds remain useful for architecture and packaging checks, but they are for testing only and are not suitable for public release.

## License

MIT
