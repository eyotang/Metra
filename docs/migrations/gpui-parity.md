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
