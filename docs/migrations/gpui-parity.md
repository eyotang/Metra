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

CI run 45556824806 built experimental macOS DMG and Windows NSIS/portable
artifacts successfully. Linux initially failed the pre-existing malformed
Claude-auth response assertion; the assertion hid the actual error message.
The same full core unit suite passed locally, and rerunning the failed Linux
job passed, including its installer. The root cause is not established; the
assertion now includes the synthetic failure message for future diagnosis.

Native numeric/currency/date formatting now uses pinned ICU4X locale data.
Reference tests generated with the original Intl options cover seven locales,
84 compact-number cases (including rounding/unit boundaries), 140 currency
cases (USD/EUR/JPY/KRW/CNY), and seven date/time cases. All pass, along with the
existing three native tests. French JPY/CNY symbols retain the original spelling.
Values remain exact decimal integers until formatting; missing values stay dashes.
The formatter uses local time for captured/reset timestamps, as before.

Saved label normalization is now reflected in the editor only if its current text
still equals the submitted text. A native lifecycle smoke verifies TEST becomes
TES after save, then a subsequent NEW draft survives completion of an OLD save.
The smoke passes on Linux. These changes still require the next platform CI run;
previous installer artifacts do not include the new locale dependency set.

Windows release builds now select the GUI subsystem; a native build script adds
Metra icon group 1 and version identity. GPUI's existing manifest remains the
single source of PerMonitorV2/asInvoker settings. Packaging now inspects the PE
resource tree and subsystem before accepting a Windows bundle. Actual linked
resource validation awaits Windows CI. macOS sets accessory activation before
launch; packaging verifies codesign, bundle identity/version/LSUIElement, CPU
architectures and DMG integrity. These platform-specific changes await CI.

A portable scripts/smoke-gpui.mjs runner launches the native smoke feature with
an isolated configuration and a bounded timeout. CI now runs it on all three
systems (Xvfb/software Vulkan/session D-Bus on Linux) before packaging. The
updated Linux host build/check and smoke pass. No remote runtime acceptance is
claimed until those jobs finish. The workflow preserves running evidence instead
of cancelling it when a new commit is pushed.

Native macOS work-area coordinates now come from the global AppKit visible
frame, matching the native positioning adapter; GPUI's display-local bounds were
incorrect for secondary monitors. Display-change events clamp bubble position
and re-anchor panels without a polling timer. Linux compiles this code; macOS
compilation and real mixed-DPI/display-removal acceptance remain outstanding.
GPUI/GPUI Kit Apache and ICU4X Unicode license texts are bundled in third-party
notices. The optional earlier Windows artifact download was stopped after it remained
pending; PE inspection will run directly on the newly linked executable in CI.
No local installed application has been replaced. The Linux native smoke also
passes a real offscreen move followed by the display-recovery clamp.

Run 45557609039 passed on all three systems (including locale dependencies and
experimental installers). The next run, manually dispatched as 45558411829,
verified Windows native startup/lifecycle, PE icon/version/DPI/asInvoker metadata
and installer packaging successfully. macOS exposed an invalid Application
initialization call: the prelaunch accessory policy now uses the supported
with_windowing API. Linux Xvfb correctly had no StatusNotifierWatcher; smoke
now verifies the persisted locale without requiring a nonexistent tray menu,
while retaining the real-menu assertion when a tray is present. Both corrections
await the next CI run.

Bubble rendering now preserves the original gradient border, compact label
badges, percentage sizes, stale opacity and remaining-quota warning thresholds
in either percentage mode. A 0..100 boundary contract passes for both modes and
missing values. Locale/mode changes invalidate the bubble presentation. It has
a localized accessible button name, usage tooltip, focus handling and a 300ms
post-drag click guard. Linux synthetic-data screenshot inspection confirmed no
label/value clipping. Toolkit-dispatched native mouse events verify left-click
open/close and right-click settings, in addition to previous lifecycle checks.
An OS-level XTest injection attempt did not establish event delivery under this
Hyprland session; system mouse/drag/hover acceptance remains open. The temporary
demo process was stopped, without modifying installed settings.

Run 45559089888 passed Linux and Windows native lifecycle/input tests and package
builds. macOS passed compilation and the native lifecycle/input test, then failed
packaging because requesting only DMG caused the bundler to delete its temporary
.app before metadata verification. Packaging now requests app,dmg and explicitly
uses the supplied Apple signing identity or an ad-hoc distribution signature;
local installation still requires the existing persistent local signing identity.
The macOS packaging correction awaits CI.

Reviewing the retained Windows upgrade acceptance found that status-bar-only
startup never migrated the bubble coordinate version, and a new non-docked bubble
never persisted its initial location. Both now persist version 3; status-bar
startup uses a hidden temporary native window to determine coordinates without
showing a bubble. The isolated --legacy-status smoke checks the original JSON
migration and preserves display mode, percentage mode, labels, order, visibility
and autostart. This and the full native lifecycle smoke pass locally. CI runs the
migration test on each system and invokes the retained v0.2.2 /S installer upgrade
acceptance against the GPUI Windows installer. Defaults and release workflow have
not yet been switched; this avoids publishing before upgrade acceptance passes.

Normal npm dev/build/package/check/test commands now target GPUI; the stable
package-native entry delegates to package-gpui. Explicit legacy commands retain
the reference shell. Seven README editions, Linux/development instructions and
sanitized native screenshots are synchronized. The local macOS installation
entry now defaults to the retained native .app while preserving its backup,
quarantine removal and persistent signing checks. Release CI uses the native
packager and adds native acceptance while retaining legacy regressions.

Local npm check, npm test, and both npm test:ui native scenarios pass. The new
npm package debug Debian archive was inspected for the exact staged executable,
permissions, native dependencies and checksum. No installation or publication
has been performed. The root Cargo desktop feature remains available for legacy
regression builds; shipped npm package artifacts use desktop-gpui.

Audit found GPUI's default last-window-close exit on Windows/Linux would terminate
status-bar-only startup or closing its final panel. The app now selects explicit
quit mode. Smoke closes the final status-bar panel for one phase and then reopens
it; migration smoke starts without opening a test panel. Both pass locally.
CI run 45559674202 passed Linux; Windows migration exited 0 without a result,
consistent with the now-fixed last-window behavior. macOS signing/metadata checks
passed but lipo interpreted the executable path as an architecture because the
arguments were ordered incorrectly. The verifier now puts the executable first.
These final corrections await the next platform CI run.
