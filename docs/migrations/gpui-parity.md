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

Shared status entries now drive an independent transparent popup bubble and the
tray tooltip. Bubble observations compare displayed entries before notifying.
Basic left-click details, right-click settings, and thresholded native dragging
are wired. Display mode creates/removes the bubble, with a bubble fallback when
tray creation fails. Tray menus rebuild on language/mode changes; status mode
cycles every three seconds and uses the existing numeric bitmap on Linux/Windows
and title on macOS. Returning to bubble mode restores the template/color icon.

Linux native tests (3), build, and six-second startup smoke pass without logged
errors. These do not establish pointer interaction, positioning, topmost/skip-
taskbar parity or cross-platform correctness. Snap/peek, position persistence,
keyboard bubble opening and panel anchoring remain outstanding.

Provider label editors now retain InputState across refreshes and save on blur or
Enter. Provider handles support native drag/drop and keyboard arrows (Escape
cancels an active drag). Autostart registration uses auto-launch and rolls back
registration on database save failure. Settings commands queue while a previous
save is running. Pointer-driven reorder/editor acceptance and draft normalization
feedback remain to verify.

A `native-smoke` feature runs isolated assertions for panel destruction/recreation,
bubble/status mode switching, Japanese tray menu synchronization, queued label
and interval persistence, and normal application quit. This passed on Linux with
exit 0. An initial test incorrectly expected four-character labels; the existing
normalization limit is three. Its assertion failure was followed by a SIGSEGV in
Wayland/Vulkan swapchain teardown (systemd core evidence). Corrected normal exit
passed; panic-unwind teardown is still a known diagnostic concern.

Positioned overlays now use raw native window adapters (Xcb, Win32, AppKit),
sharing the existing physical-coordinate geometry and version-2 migration rules.
Linux selects X11/XWayland when DISPLAY is available, matching the existing
positioned-overlay support contract. Bubble dimensions are 56x56 expanded and
32x56 docked. Hover expands, leave delays collapse by 500ms, native drag release
is observed before docking/persisting, and snap is exposed in settings. Panels
resize and clamp near the bubble or tray. Linux native smoke passes with actual
bubble position/work-area containment assertions. AppKit/Win32 adapters still
need CI compilation and hardware mixed-DPI validation. Pointer drag/hover,
monitor removal, focus policy, topmost and skip-taskbar acceptance remain open.

Cursor sign-in and URL launching are now shared desktop operations used by both
shells. GPUI provides login/recheck actions, background update checks after 20s
and daily thereafter, manual check, release notes, and signed installer/manual
release actions using the existing Updater. Core headless contracts and native
unit tests pass locally; installer execution and login UI are not accepted by
these tests. A branch-scoped three-platform CI workflow now checks native builds
and contracts; its first remote results are pending.

Default native startup now shows the bubble only; `--settings` opens settings.
The native executable also preserves --usage-json/--watch-json. Close, quit,
rescan, bubble keyboard opening and tray/bubble details toggling are wired.
Inter is bundled and the native theme is dark. Linux native lifecycle smoke
passes after these changes; pointer and keyboard input still need acceptance.

Experimental packaging lives in scripts/package-gpui.mjs. Linux creates a Debian
archive directly because Tauri's bundler unconditionally adds WebKit/GTK runtime
dependencies. The debug Debian artifact was inspected for exact GPUI binary,
root ownership, executable mode, desktop entry, native dependencies and SHA-256.
macOS/Windows packaging uses existing Tauri metadata/hooks with WebView bootstrap
skipped and the GPUI executable staged as metra. Those bundles remain unverified.
The npm lockfile mirror URLs were normalized to registry.npmjs.org with existing
integrities retained; npm ci passes (esbuild's script remains blocked by local
npm policy and is not needed for native packaging).

CI run 45555678600 is still running for the earlier 856365b commit. Subsequent
changes are kept local until it finishes to avoid cancelling first-platform
build evidence. Remote CI does not yet validate these latest changes.

First three-platform CI run 45555678600 passed on Linux, macOS and Windows at
856365b (core contracts, native check and unit tests). It does not establish
installer behavior or native pointer interaction on macOS/Windows. The next
workflow additionally builds experimental debug installers and retains artifacts.

A feature-gated METRA_GPUI_DEMO mode supplies synthetic provider data for visual
inspection and requires METRA_CONFIG_DIR. Linux screenshots exposed a tiled
panel; panels now use popup windows, preserve the 340/360 logical widths, and
resize on navigation. Shared bundled typography is compact. Settings and usage
screens were inspected with simulated data; scrolling and input acceptance are
still outstanding. Language labels now precede their controls, interval controls
wrap, and cards have explicit boundaries. Panels close on focus loss. Update
notes are collapsed by default and scroll within a bounded area when expanded.
Linux native lifecycle smoke and three native unit tests pass after these changes.
