# Development reference

## Development

Install stable Rust (GPUI requires the current stable toolchain), Node.js 24+
and your platform tools, then:

```sh
npm ci
npm run dev
npm run dev -- --settings
npm run check
npm test
npm run test:ui
npm run build
```

`METRA_CONFIG_DIR` selects isolated test settings. Closing a panel keeps Metra
running. The native UI lives in `desktop-gpui/` and shares the root collector,
SQLite settings and updater. `npm run dev` builds the native executable directly.

macOS requires Xcode command line tools. Windows requires Visual Studio C++
build tools and the Windows SDK. Linux requires Clang, CMake, pkg-config,
Fontconfig, XKB, Wayland development libraries and a Vulkan driver; positioned
overlays use X11/XWayland. See [Linux setup](linux.md).

The collector builds without Node, Tauri, GTK or a webview:

```sh
cargo build --release --locked --no-default-features --target-dir target/collector
target/collector/release/metra --usage-json
target/collector/release/metra --watch-json
```

Watch mode accepts `refresh` lines on stdin and exits when stdin closes. See the [Omarchy integration](../integrations/omarchy/README.md).

## Packaging and updates

For local macOS installation, first run `npm run setup:local-signing` once, then build the `.app` and run `npm run install:local`. It backs up the previous app, preserves settings, clears extended attributes and verifies the local signature.

The setup creates a persistent self-signed `Metra Local Development` identity in the default user Keychain, restricted to use by `codesign`. It does not add a trusted system root and is not a Developer ID for public distribution. Installation pins the designated requirement to `dev.metra.desktop` and the certificate fingerprint; missing or ambiguous identities fail instead of falling back to ad-hoc signing. `METRA_LOCAL_SIGNING_IDENTITY` can select another unique certificate name. Keep the same certificate and private key across updates.

Cursor compatibility mode prefers the CLI access token (`cursor-access-token` / `cursor-user` in the macOS Keychain, otherwise the CLI `auth.json` in `~/.cursor` on macOS, `%APPDATA%/Cursor` on Windows or `$XDG_CONFIG_HOME/cursor` on Linux). It skips expired JWTs and falls back to the desktop token when CLI credentials are unavailable or usage authentication rejects them. CLI Keychain reads are cached for 10 seconds. Refused reads wait 10 minutes before another probe, with file and desktop fallback still available.

On macOS, the desktop fallback accesses Cursor's item through Security.framework. Choose Always Allow on the first system prompt to remember access for Metra. Imported tokens from either source are stored in Metra's `dev.metra.desktop.cursor-token` login Keychain item and zeroized process memory. Restart reuse requires a matching SHA-256 fingerprint of the local token. A changed or malformed record is removed before reimport. Cursor's decryption key stays in its original Keychain item. Keep that item's access restricted to authorized apps.

On Windows, the desktop fallback opens Cursor's state database and `Local State` read-only. Metra uses current-user DPAPI and authenticated AES-GCM for the encrypted desktop format. The token cache stays in memory. If Metra cannot decrypt a local token, it shows the read error and keeps the last successful usage reading. When every queried usage endpoint returns HTTP 401, Metra clears the rejected token cache and retries with the desktop credential once.

Removing the local desktop token or disabling compatibility mode clears the desktop token cache.

```sh
npm ci
node scripts/package-native.mjs
# macOS universal DMG:
rustup target add aarch64-apple-darwin x86_64-apple-darwin
node scripts/package-native.mjs --universal
```

`scripts/package-native.mjs` delegates to the GPUI packager. It builds the native
binary, then reuses the existing Tauri bundler metadata and NSIS hooks for macOS
and Windows; Linux uses a native Debian package without GTK/WebKit dependencies.
Public names remain unchanged: macOS DMG, Windows per-user NSIS and portable EXE,
and Linux Debian package. No WebView runtime is required. `CARGO_TARGET_DIR`
selects the Rust build directory; bundler staging is `target/gpui-package/`.

macOS packaging retains both `.app` and DMG. `npm run install:local` installs
`target/gpui-package/release/bundle/macos/Metra.app`; pass a different `.app` path
as an argument when installing a universal build. Distribution packages use
`APPLE_SIGNING_IDENTITY` when supplied, otherwise an ad-hoc signature. The local
installer still requires the persistent signing identity described above.
`npm run build:portable` builds the release executable in
`desktop-gpui/target/release/`; `npm run package` creates named release assets.

Metra checks the GitHub Releases API for a newer stable version and shows its release notes. The Windows x64 installer and its `.sig` are downloaded from that tagged release when the user requests an update. Metra verifies the installer with the embedded Minisign public key, then launches the NSIS installer with `/S /NS` for a silent installation without creating new shortcuts. Windows portable, macOS and Linux open the release page for manual installation.

The standalone `tools/release-sign` reads `METRA_SIGNING_PRIVATE_KEY` and the optional `METRA_SIGNING_PRIVATE_KEY_PASSWORD`. Release CI maps its existing stored Minisign credential to those inputs so installed versions can keep trusting the same key. The release job still publishes `latest.json` for already-installed v0.6.0 and older clients.

Controls use GPUI Kit, with embedded fonts and locale data. Provider refreshes,
settings saves, sign-in and update operations run off the UI thread. The runtime
owns state independently of panel windows, preserving edits across refreshes.

Native CI checks provider contracts, seven-language formatting, lifecycle,
bubble input, offscreen recovery and legacy configuration migration on Linux,
macOS and Windows. Packaging validates Windows PE resources/DPI metadata and
macOS identity, signing, architecture and DMG integrity. Windows installers retain
the v0.2.2 silent-upgrade acceptance test. Real mixed-DPI dragging still requires
hardware acceptance. See [migration acceptance](migrations/gpui-parity.md) for
current evidence and remaining checks.

The previous Tauri/WebView implementation remains as a regression reference:
`npm run dev:legacy`, `npm run build:legacy`, `npm run frontend:build` and
`npm run test:legacy-ui` (after `npx playwright install chromium webkit`). Its
native acceptance scripts exercise the reference implementation, not GPUI.
Normal npm build/package commands produce GPUI artifacts.

## Code layout

| Path | Responsibility |
| --- | --- |
| `desktop-gpui/src/` | GPUI views, tray runtime and native window adapters |
| `src/geometry.rs`, `src/platform.rs` | Shared geometry, startup registration and native input |
| `src/providers/`, `src/service.rs` | Account discovery and usage collection |
| `src/settings.rs`, `src/settings_change.rs`, `src/model.rs` | SQLite settings, commands and data model |
| `src/headless.rs`, `src/updater.rs` | JSON transport and signed updates |
| `tauri.conf.json`, `packaging/` | Installer metadata and compatibility hooks |
| `scripts/package-gpui*.mjs`, `scripts/smoke-gpui.mjs` | Native packaging and lifecycle acceptance |
| `frontend/`, `src/app.rs`, `tests/ui/` | Legacy shell and browser regression reference |

[MIT License](../LICENSE). GPUI/GPUI Kit use Apache-2.0 and ICU4X uses Unicode-3.0;
notices are bundled in `assets/THIRD_PARTY_NOTICES.txt`. Inter retains its OFL
license in `assets/fonts/`.
