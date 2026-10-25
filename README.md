# Metra

**English** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra monitors Cursor, Codex and Claude Code usage with **Tauri 2, TypeScript and Rust**. Windows renders through WebView2, macOS through WKWebView, and Linux through WebKitGTK. Rust handles local account discovery, usage collection, settings and signed updates.

- A draggable, always-on-top 56px bubble folds into a 32px edge capsule when auto-dock is enabled. Hover expands it, leaving folds it again, and clicking or pressing Enter opens details.
- All platforms use opaque surfaces without system glass, backdrop blur or translucent panels. Browser antialiasing draws the bubble, edge capsule and panel outlines without a native hard clip.
- Status bar mode cycles visible providers every five seconds. macOS shows text; Windows shows numeric tray icons; Linux depends on the panel host.
- Details show account status, quota windows, reset countdowns, token totals and costs. Settings retain provider order, visibility, labels, colors, four languages, refresh interval and launch-at-startup.
- Existing `dev.metra.desktop/settings.db` remains in the system configuration directory. Saved positions are migrated to Tauri physical coordinates and clamped to monitor work areas.
- Cursor credentials are read only after explicit compatibility consent. Failed refreshes preserve successful data and mark it stale.
- The independent collector and Omarchy Quickshell widget retain their JSON protocol.

## Development

Install stable Rust, Node.js 24+ and your platform's Tauri prerequisites, then:

```sh
npm ci
npm run dev
npm run dev -- -- --settings
npm run frontend:build
npm run check
cargo test --locked --all-targets
npx playwright install chromium webkit
npm run test:ui
npm run build -- --no-bundle -- --locked
```

`METRA_CONFIG_DIR` selects an isolated test settings directory. Closing a panel keeps Metra running; quit through the tray or settings. The Tauri config and Cargo package live at the repository root so existing collector commands retain their paths. Use `npm run dev` for development and `npm run build` for embedded frontend assets; plain `cargo run` does not start Vite.

macOS requires Xcode command line tools. Windows requires Visual Studio C++ build tools, the Windows SDK and WebView2 (the installer bootstraps it when missing). Linux requires WebKitGTK 4.1, GTK 3 and AppIndicator; positioned overlays use X11/XWayland. See [Linux setup](docs/linux.md).

The collector builds without Node, Tauri, GTK or a webview:

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

Watch mode accepts `refresh` lines on stdin and exits when stdin closes. See the [Omarchy integration](integrations/omarchy/README.md).

## Packaging and updates

```sh
npm ci
node scripts/package-native.mjs
# macOS universal DMG:
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

Tauri builds and bundles the frontend, icons and platform runtime integration. The helper copies artifacts into `release-assets/` with the existing public release names: a macOS DMG, a Windows per-user NSIS installer plus portable EXE, or a Linux Debian package. Windows portable builds require an installed WebView2 runtime. Tauri manages NSIS tooling. `CARGO_TARGET_DIR` is supported.

The updater retains the existing GitHub `latest.json` and Minisign public key. Windows installed copies verify the signature before launching an explicitly requested installer update. Portable copies, macOS and Linux open the detected release for manual download. The separate `tools/release-sign` tool accepts the existing `TAURI_SIGNING_PRIVATE_KEY` secret names. Secrets are never included in the webview.

Common controls use Web Awesome (buttons, switches, checkboxes, text fields, selects, popovers and progress), with Lucide icons. All components and icons are bundled locally. Lit keeps control nodes stable during background updates. Each provider has its own refresh animation and publishes results as soon as it finishes.

CI runs Rust and Chromium/WebKit UI checks on Windows, macOS and Linux using simulated IPC. Checks cover keyboard access, scaled windows, popup bounds, offline assets and per-provider progress, and retain screenshots for each OS. Native Windows checks inspect actual composited pixels for clear corners, smooth outlines and opaque surfaces over different desktop colors. macOS checks the tray template property; Windows installers are tested against the legacy silent upgrade contract. Mixed-DPI dragging still requires hardware acceptance.

## Code layout

| Path | Responsibility |
| --- | --- |
| `frontend/` | Lit views, Web Awesome controls, Lucide icons, opaque surfaces and styling |
| `src/app.rs`, `src/geometry.rs` | Tauri commands, events, windows, dragging and edge geometry |
| `src/platform.rs`, `src/tray.rs` | Startup registration, native input and Tauri tray |
| `src/providers/`, `src/service.rs` | Account discovery and usage collection |
| `src/settings.rs`, `src/model.rs` | SQLite settings and stable data model |
| `src/headless.rs`, `src/updater.rs` | JSON transport and signed updates |
| `tauri.conf.json`, `capabilities/` | Bundles, CSP and local webview permissions |
| `tests/`, `tests/ui/` | Rust contracts and browser interaction tests |

[MIT License](LICENSE). Tauri uses MIT/Apache-2.0; Inter retains its OFL license in `assets/fonts/`.
