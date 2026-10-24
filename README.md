# Metra

**English** · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Metra is a native desktop monitor for Cursor, Codex and Claude Code usage, built in Rust with **GPUI and GPUI-KIT 0.8.0**. The application renders without a browser or WebView.

- Frosted, translucent application windows use native platform blur; the system tray retains its platform presentation. Linux blur depends on compositor support.
- A draggable, always-on-top floating bubble remains the default. Optional edge snapping folds the bubble to a 32px strip.
- Status bar mode cycles enabled providers every five seconds. macOS displays text usage; Windows uses numeric tray icons; Linux supports both where the panel host permits them.
- Details show login state, quota windows, reset times, token totals and available costs. Settings preserve provider visibility, order, labels, colors, language, refresh interval and startup preferences.
- Existing SQLite settings remain in `dev.metra.desktop/settings.db` under the system configuration directory. The four interface languages and existing Cursor compatibility consent are preserved.
- Cursor, Codex and Claude collectors remain independent of GPUI. Failed refreshes retain the last successful data and mark it stale. Cursor credentials are read only after explicit compatibility consent.
- Omarchy's native Quickshell widget continues to use the same line-oriented JSON collector.

## Development

Install a current stable Rust toolchain, then:

```sh
cargo run
cargo test --locked --all-targets
cargo build --release --locked
```

Use `cargo run -- --settings` to open settings immediately. `METRA_CONFIG_DIR` selects an isolated settings directory for development and UI checks. The app stays in the background when panels are closed; exit through the tray or settings menu.

On macOS, install Xcode command line tools. Linux requires GTK 3 / AppIndicator for the tray, X11 or XWayland for positioned overlays, and a Vulkan graphics driver for GPUI. See [Linux setup](docs/linux.md). Windows requires the Visual Studio C++ build tools and Windows SDK.

The collector can be built without any desktop dependency:

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

The watch process accepts `refresh` lines on stdin and exits when stdin closes. Its JSON schema remains compatible with the [Omarchy integration](integrations/omarchy/README.md).

Use suitable GPUI-KIT controls before introducing custom controls. Settings use the library's button groups, list items, switches, inputs, selectors, preset color swatches in a popover, and scrollbars. Keep the circular floating overlay, edge snapping and platform tray integration custom where the library does not provide an equivalent; Omarchy's host requires its native Quickshell widget. Custom layouts may compose GPUI elements without replacing standard control behavior.

## Packaging and releases

Node.js 24 is needed only for packaging helpers and the Omarchy installer; app development itself uses Cargo. No npm packages are required.

```sh
node scripts/package-native.mjs                 # Native host package
node scripts/package-native.mjs --universal     # macOS arm64 + Intel DMG
node integrations/omarchy/install.mjs           # Install the Omarchy widget
```

Packages appear under `release-assets/`. macOS packages contain the complete app and embedded resources. Windows produces a per-user NSIS installer and portable executable; packaging requires NSIS (`makensis`, or the `MAKENSIS` environment variable). Linux produces a Debian package. Installation and rendering must be verified on the actual target platform; a Cargo check alone does not verify native window behavior.

The updater still reads the existing GitHub `latest.json`. Installed Windows copies verify the existing Minisign public key before running an explicitly requested installer update. Portable Windows, macOS and Linux open the exact detected release for manual download. Background update failures never interrupt usage monitoring. Signing secrets remain in CI; the separate `tools/release-sign` tool accepts the existing `TAURI_SIGNING_PRIVATE_KEY` secret names for compatibility.

## Code layout

| Path | Responsibility |
| --- | --- |
| `src/app.rs`, `src/appearance.rs` | Native GPUI windows, GPUI-KIT controls and Metra styling |
| `src/platform.rs`, `src/tray.rs` | Native overlay positioning, startup registration and system tray |
| `src/providers/`, `src/service.rs` | Account discovery and background usage collection |
| `src/settings.rs`, `src/model.rs` | SQLite settings and stable data model |
| `src/headless.rs`, `src/updater.rs` | JSON transport and framework-independent signed updates |
| `assets/` | Embedded icons, native fonts, translations and update public key |
| `tests/` | Collector, model, settings and signature contracts |

[MIT License](LICENSE). GPUI-KIT and GPUI are Apache-2.0; the embedded Inter font retains its OFL license in `assets/fonts/`.
