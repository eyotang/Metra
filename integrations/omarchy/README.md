# Metra for Omarchy

Native Quickshell bar widget for Omarchy 4's plugin API. It reuses Metra's
existing Rust collectors without starting GTK, WebKit, a tray icon, or XWayland.

The fixed-width bar label shows one provider at a time (`X 92%`). Multiple
visible providers slide horizontally every five seconds; scrolling the mouse
wheel switches manually. Hovering the label or card pauses the carousel.
Each provider's headline uses its most constrained quota window; the card
lists every window. The interval is adjustable in the plugin settings.

Hover over the bar label for 180 ms to open the usage card. Moving into the
card keeps it open; leaving both areas closes it after 320 ms, allowing time
to cross the gap. Click also opens it; middle-click or the refresh button
requests an update. The panel never takes keyboard focus on hover.

Percentages show **remaining** allowance. No quota is shown as `—`; it is not
treated as 0%. Failed refreshes keep the last successful values marked stale.
Provider order and visibility, refresh interval, and explicit Cursor
compatibility consent are read from Metra's existing settings. Cursor
compatibility is never implicitly enabled. One collector serves all monitors.

## Install

Requires Linux x86_64, Node.js 24+, and Omarchy with `omarchy-shell` / Quickshell
plugins (`qs.Ui.PopupCard`, `qs.Ui.BarWidget`). Older Waybar-based Omarchy
releases are not supported by this integration.

From source:

```bash
npm ci
cargo build --release --locked --no-default-features --target-dir target/collector
node integrations/omarchy/install.mjs --binary target/collector/release/metra
```

Or extract `Metra-<version>-omarchy-x64.tar.gz` from Releases, verify its
SHA-256, enter the extracted directory, and run `node install.mjs`.
Installation copies the executable and QML to
`~/.config/omarchy/plugins/metra.usage/`, backs up `shell.json`, and enables
the widget using Omarchy's plugin CLI. Existing layout entries are preserved.
The UI follows Omarchy theme colors, typography, scale and bar position.
The current widget text is Simplified Chinese; the cross-platform app's
language setting does not yet change the native widget.

The desktop bubble can be exited from its own context menu once the widget
is running. Installing the widget does not change the app's autostart setting.

To remove it from the bar: `omarchy plugin disable metra.usage`.
To update it, rerun the installer with the new build or extracted release.
Source installs and release packaging take the plugin version from
`Cargo.toml`; extracted releases retain that stamped version. The installer
prints the installed version, and CI checks that the source manifest also
matches the application version.

## Verification

`npm run verify:omarchy` checks presentation rules. After building on Linux,
`node scripts/verify-omarchy.mjs target/release/metra` also checks JSON
output without a display, manual refresh acknowledgement and stdin shutdown.
On an Omarchy desktop, `npm run verify:omarchy-ui` uses fixture data in a
separate test shell to verify carousel timing, pause/resume, reverse navigation
and fixed width without changing account data or desktop configuration.

## Collector interface (Linux)

`metra --usage-json` emits one JSON payload and exits. `metra --watch-json`
emits newline-delimited payloads and refreshes at the configured interval;
write `refresh\n` to its stdin for a manual update (at most once per ten
seconds). Closing stdin exits the collector after any in-flight query.
Only usage and presentation settings are emitted, using the app's existing
payload schema. No API key, token, or conversation text is exported.
