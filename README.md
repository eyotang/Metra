# Metra

**English** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md)

## Overview

**Your AI coding usage, at a glance.**

Keep Cursor, Codex and Claude Code limits, tokens and costs in a small desktop companion. Available on **macOS, Windows and Linux**.

- **Always within reach.** Drag the floating bubble anywhere, dock it at the edge, or use the status bar.
- **One place for three tools.** See available quotas, reset times and usage without switching apps.
- **Make it yours.** Customize provider order, labels, colors and refresh intervals. Seven interface languages, with automatic detection.

| Usage details | Display settings |
| :---: | :---: |
| <img src="docs/assets/readme/usage-demo.png" alt="Usage details" width="320"> | <img src="docs/assets/readme/settings-demo.png" alt="Display settings" width="320"> |

Current interface with simulated usage data; no real account information. Available data varies by provider and sign-in method.

## Get started

**[Download the latest release](https://github.com/eyotang/Metra/releases/latest)**

| Platform | Choose | Install |
| --- | --- | --- |
| macOS | `*-macos-universal.dmg` | Open the DMG and drag Metra to Applications. |
| Windows x64 | `*-windows-x64-setup.exe` | Run the installer; or use the portable EXE (requires WebView2). |
| Linux x64 | `*-linux-x64.deb` | Install the package: `sudo apt install ./Metra-*-linux-x64.deb`. |

On macOS, after moving the app to Applications, run this before the first launch to remove the quarantine attribute:

```sh
sudo xattr -dr com.apple.quarantine /Applications/Metra.app
```

Launch Metra, sign in to the tools you use, then click the bubble to see usage. Enable **Cursor compatibility** in Settings if you want exact Cursor usage; it requires your consent to read local credentials. Drag to reposition; enable auto-dock to tuck it against an edge.

Use Settings to change the language, refresh interval or launch-at-startup. Closing a panel keeps Metra running; quit from Settings or the tray menu.

[Linux setup](docs/linux.md) · [Omarchy widget](integrations/omarchy/README.md)

## Develop & contribute

Install **Node.js 24+**, **stable Rust** and the platform tools below. Fork this repository and clone your fork, then run from its root:

| Platform | Tools |
| --- | --- |
| macOS | Xcode command line tools |
| Windows | Visual Studio C++ build tools, Windows SDK, WebView2 |
| Linux | WebKitGTK 4.1, GTK 3, AppIndicator; X11/XWayland for floating windows |

```sh
npm ci
npm run dev
```

Before submitting a PR:

```sh
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
```

Start in `frontend/` for UI, `src/providers/` for collectors, and `src/service.rs` for refresh logic. Use `METRA_CONFIG_DIR` for isolated test settings.

Open a focused PR explaining the problem, the change and how you verified it. Include screenshots for UI changes; update all seven translation catalogs and README editions when affected.

For a local macOS install, build with `npm run build`, then run `npm run install:local`.

[Build, packaging and collector reference](docs/development.md) · [Issues](https://github.com/eyotang/Metra/issues) · [Pull requests](https://github.com/eyotang/Metra/pulls)

[MIT licensed.](LICENSE)
