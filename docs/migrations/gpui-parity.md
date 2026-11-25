# GPUI desktop migration acceptance

Branch: `feat/gpui-desktop-migration`. Baseline: committed v0.5.18 (`8a55f80`).
Worktree: `/home/eyotang/Work/ai/Metra-gpui`. Concurrent, uncommitted provider
cache changes in the original worktree are not part of this baseline. The new shell must not become the
release default until every item below is implemented and verified. An unchecked
item is outstanding, not an intentionally removed feature.

## Architecture

- [x] Preserve the collector-only entry point and existing SQLite settings.
- [x] Extract the shared settings command representation from Tauri.
- [ ] Build independent `desktop-gpui` entry point on all three desktop platforms.
- [ ] Single-instance lock and activation of existing process.
- [ ] Event-driven publication of provider results; no idle rendering timer.
- [ ] Background work does not block input or closing panels.

## Provider data and controls

- [ ] Cursor, Codex and Claude discovery, credentials, usage and stale fallback.
- [ ] Independent loading states and incremental publication per provider.
- [ ] All quota windows, remaining/used percentages, reset dates and countdowns.
- [ ] Token totals, included usage, on-demand costs, UTC daily costs.
- [ ] Localized status, errors and missing-data states.
- [ ] Explicit Cursor compatibility consent; login and login recheck.
- [ ] Manual and interval refresh, duplicate refresh protection.

## Windows and tray

- [ ] Transparent rounded bubble, opaque content, no initial clipping.
- [ ] Always on top, no taskbar entry, correct activation/focus policy.
- [ ] Click details; context menu settings; keyboard opening.
- [ ] Native dragging, click suppression after drag, persisted position.
- [ ] Edge snap, compact capsule, hover expansion and delayed collapse.
- [ ] Recover offscreen position; mixed DPI; screen changes.
- [ ] Panel anchoring, screen bounds, resize and blur dismissal.
- [ ] Closing a panel keeps the process alive; explicit quit exits.
- [ ] Tray menus, localized labels, status mode, cycling provider values.
- [ ] macOS template icon; Windows/Linux bitmap percentage icons.
- [ ] Hidden views stop rendering; visible background capsule updates.

## Settings and presentation

- [ ] All seven languages, system detection and persisted round trip.
- [ ] Display mode, percentage mode, refresh interval, snap, autostart.
- [ ] Provider labels, palette, visibility (at least one), ordering.
- [ ] Drag ordering, keyboard ordering, Escape cancel and failed-save recovery.
- [ ] Preserve unfinished edits during background updates.
- [ ] Screen-reader names, focus traversal, keyboard controls, reduced motion.
- [ ] Version beside title; update card outside scrolling usage content.
- [ ] Bundled fonts/icons; offline startup.

## Release acceptance

- [ ] Existing Rust contracts pass; equivalent native UI contracts pass.
- [ ] macOS idle update, Windows composited corner/startup/resize checks.
- [ ] Linux X11/XWayland and supported Wayland behavior documented/tested.
- [ ] Measure startup, idle CPU/GPU, memory and wakeups against baseline.
- [ ] JSON one-shot/watch protocols and Omarchy integration unchanged.
- [ ] Signed updater manifest and install/open-release behavior retained.
- [ ] Windows legacy silent upgrade, portable detection, NSIS hooks.
- [ ] macOS universal DMG and local install through `npm run install:local`.
- [ ] Linux Debian and standalone Omarchy packages.
- [ ] Seven README editions and development instructions updated.
- [ ] English then Chinese release notes, required quarantine command included.
- [ ] CI passes for the release commit; artifacts verified before publication.

## Development checkpoint

The experimental entry point lives in `desktop-gpui/` with a separate lockfile.
Run with an isolated `METRA_CONFIG_DIR` during testing. It currently provides
provider quota summaries, manual refresh and incremental provider notifications;
it is not a feature-complete desktop replacement.

GPUI Kit is pinned to 0.11.0. Rust 1.94 fails on GPUI's `std::hint::cold_path`;
the available Rust 1.99.1 toolchain is used for validation. No release tag or
release publication has been created by this migration.

Verified: two settings command contracts (seven-language round trips and final
visible-provider protection). The original checkout's collector unit tests
passed 57 tests with one ignored before isolation; repeat on this branch before
release. The independent GPUI entry point passes `cargo check` on Linux with Rust 1.99.1.
Native linking, visual acceptance and other platforms remain outstanding.

The native settings page now wires language, percentage mode, refresh interval,
provider visibility/color, and explicit Cursor compatibility consent to shared
validated settings commands. Saves run off the UI thread. Interval changes reset
the background refresh timer; no frame timer is used. This passes Linux `cargo
check`; native build and interaction verification remain pending. Provider label
editing, ordering, autostart, display mode and snap controls remain outstanding.

Usage cards now include all quota windows, remaining/used values, reset dates,
local captured timestamps, provider status/stale messages, token metrics and
included/on-demand/daily costs. Two native unit tests pass for missing-value and
zero-cost semantics. Locale-aware compact number/currency/date formatting still
needs parity work. The title/navigation are outside the scrolling body.

Linux native `cargo build --locked --manifest-path desktop-gpui/Cargo.toml`
passes. An isolated native process remained alive for eight seconds, with no
panic/error in its launch log, and was then terminated. This verifies startup
only, not rendered appearance or interaction correctness.

The native runtime now owns the dashboard independently of panel windows, an
instance lock, a filesystem activation watcher, and a tray event receiver.
Tray actions open details/settings, refresh, and quit. The icon uses the bundled
macOS template or color PNG; Linux uses tray-icon's KSNI backend without GTK.
Tray creation failure leaves the panel accessible. Status-mode cycling and menu
retranslation after settings changes remain outstanding.

Linux build and three native unit tests pass. A two-process isolated smoke test
confirmed the second process exits successfully, the first consumes the
activation request, and the first remains alive. No tray initialization error
was logged. Tray clicking, closing/reopening panels, and other OS behavior are
not yet accepted by this test.
