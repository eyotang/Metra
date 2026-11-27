# GPUI desktop migration acceptance

Branch: `feat/gpui-desktop-migration`. Baseline: v0.5.18 (`8a55f80`).
The isolated worktree excludes unrelated, uncommitted changes in the original
checkout. Released version: **[0.6.0](https://github.com/eyotang/Metra/releases/tag/v0.6.0)**,
published on 2026-11-25 from tag commit `d176a66`.
The branch's subsequent documentation commit records the completed release audit.

## Feature inventory

| Existing behavior | Native implementation and evidence |
| --- | --- |
| Cursor, Codex and Claude discovery, credentials, quotas and stale fallback | Shared `src/providers/` and `src/service.rs`; existing collector contracts retained. |
| Quota windows, used/remaining percentages, reset times, tokens and costs | `usage.rs`; missing/zero-value tests and seven-language formatting contracts. |
| Independent loading states and incremental provider updates | Background refresh channel publishes each provider result to the persistent dashboard entity. |
| Cursor consent, login and login recheck | Shared `desktop_actions.rs`; native consent and login actions use the same service. |
| Manual/interval refresh and duplicate protection | Refresh task prevents overlap; manual refresh resets the clock; interval changes retain elapsed time; unrelated settings do not reset the clock. |
| Floating bubble and status bar | Persistent runtime, native tray menus, five-second provider cycling, unavailable-tray rejection and startup fallback. |
| Labels, colors, visibility and ordering | Shared validation; cards and bubble use configured badges/colors; drag and keyboard reorder; final visible provider cannot be hidden. |
| Seven languages and settings persistence | Shared SQLite settings and language detection; seven-language round trips and ICU number/currency/date tests. |
| Unsaved label drafts | Saved normalization only replaces the submitted draft; newer edits survive background save/refresh. Native lifecycle test covers both cases. |
| Startup options | Shared platform registration, asynchronous save and rollback on persistence failure. |
| Bubble docking, hover, drag and saved position | Native move/resize, 500ms hover collapse, drag click suppression, FIFO position writer; shared geometry tests cover secondary displays and legacy coordinates. |
| Offscreen/display recovery | Native monitor work areas/DPI; saved monitor selection, nearest work area for a live window, display-change observer and panel reanchoring. |
| Details/settings/consent panels | Rounded opaque content over transparent native windows; bounds clamping, resize, blur close, Escape close and drag cancellation. |
| Background lifetime and reopening | Explicit quit mode, state owned independently of windows, single-instance activation and macOS reopen callback. |
| Keyboard and accessible controls | Focused panel/bubble roots, named icon buttons and swatches, keyboard ordering and Enter/Space activation. |
| Update checks, notes and installation | Shared updater/signature verification; startup/daily/manual checks, bounded notes, installed-Windows update and manual download elsewhere. |
| JSON and Omarchy integration | Same headless entry point and collector; existing protocol/widget verification retained in CI. |
| Offline resources | Bundled fonts/icons and ICU locale data; no WebView runtime. |

## Verification evidence

- Local `npm test`: 106 tests passed, 6 existing opt-in tests ignored. These
  include collector contracts, settings, updater checks, monitor geometry and
  native formatting. Ignored tests requiring external environments or release
  assets are not counted as passed.
- Local native lifecycle checks: panel recreation, display modes, locale,
  settings persistence, label drafts, offscreen recovery, bubble input, Escape
  close and legacy status-bar configuration migration passed.
- Local native idle check: the compact bubble's render callback received 37% ->
  61% usage after 330 seconds, without opening a panel or expanding.
- Run `45567406576` (`6ab722e`): all three full regression jobs
  passed, including native lifecycle, legacy rendering/desktop, collector,
  Omarchy and signing-tool checks, plus macOS native inactive repaint.
- Windows actual composited pixels passed for bubble, capsule and panel over
  two backdrop colors; six evidence images were downloaded and representative
  surfaces visually inspected. The root theme background and Windows native
  non-client frame were both removed from the transparent overlay surface.
- Run `45567266857` (`6ab722e`): all three native jobs passed, including
  installers, the macOS 330-second inactive/visible repaint and tray template
  assertions, and Windows v0.2.2 silent upgrade with settings preservation.
- The v0.6.0 Linux release package built locally through the default packaging
  entry point. Binary identity, package metadata, ownership, permissions,
  notices and checksum were verified. Its production executable passed the
  Omarchy/no-display JSON/watch/refresh/EOF contract. Public artifacts must come
  from the final tagged CI run.
- README screenshots are from the native v0.6.0 application with isolated,
  synthetic provider data, inspected after capture.
- Tagged [run 45569629174](https://github.com/eyotang/Metra/actions/runs/45569629174)
  passed all three verification jobs, all three release jobs and publication.
  macOS packaging verified both architectures, strict app signing, identity and
  DMG integrity. The production Windows installer passed the v0.2.2 silent
  upgrade and signature check against the existing public key.
- All ten public release files were downloaded and checked against the published
  sizes/digests, package checksum files and exact updater manifest. Release notes
  match `docs/releases/v0.6.0.md`, including English-first/Chinese-second order and
  both macOS quarantine commands. The release is public, stable and latest.
- Downloaded Windows PE metadata and the public installer signature passed local
  verification. The downloaded Linux package's executable passed the no-display
  JSON/watch/refresh/EOF contract; package version, architecture, permissions,
  dependencies and Omarchy manifest version passed. Public Windows/Linux files
  match the independently checked CI artifacts byte for byte.

Consent navigation resizes consistently; failed compatibility enablement keeps
the consent page open. Native lifecycle checks cover consent-size transitions.
The feature-gated macOS harness explicitly deactivates the visible app, asserts
actual inactive/non-hidden state, and checks render delivery after 330 seconds.
Production activation behavior is unchanged by that test setup.

## Completed release gates

- [x] Final candidate passes native compilation and lifecycle checks on all platforms.
- [x] Windows composited pixel test passes for bubble, capsule and panel: corners
      follow two synthetic backdrop colors while center content remains opaque.
- [x] Final macOS inactive paint/template test passes, including its activation precondition.
- [x] Final full workflow passes collector, native, legacy regression, Omarchy and signing-tool checks.
- [x] Tagged release builds universal macOS DMG, Windows installer/portable and
      Linux/Omarchy assets; signatures, metadata, hashes and old-version upgrade pass.
- [x] Publish English-first/Chinese-second release notes and compatible updater manifest.

No performance improvement or physical mixed-DPI hardware validation is claimed.
Geometry and native monitor APIs have automated coverage; real multi-monitor
pointer interaction remains a hardware coverage limit. macOS local installation
has not been performed on this Linux machine. If delivered separately, it must
use `npm run install:local` and pass that script's signature/xattr checks.

## Parity follow-up after v0.6.0

- Provider drag targets use the upper and lower halves of the entire card. A
  moving badge and insertion line preview the drop. Escape cancels without
  saving, and the moved provider keeps keyboard focus. The original preset
  palette opens from each provider's current color.
- Each provider shows a spinner and an indeterminate progress bar until its own
  result arrives. A login recheck marks Cursor only. Settings and navigation
  remain available during usage collection. Percentages round half-up and reset
  countdowns round up at the same boundaries as the old panel.
- Free bubble placement is clamped to the selected display's work area before
  it is saved. Hover expansion rebases a held press when the window origin
  moves. Tray panels use the tray icon's display when its bounds are available.
- The expanded native smoke runs with synthetic usage. It exercises both drop
  positions, Escape cancellation, keyboard sorting, palette dismissal and
  persistence, and independent refresh state. The signed Windows in-app upgrade
  and real mixed-DPI multi-display interactions remain platform acceptance items.

## v0.6.1 candidate acceptance

Use the published `v0.5.18` and `v0.6.0` artifacts as compatibility fixtures. The
v0.5.18 reference UI and commands live in `frontend/main.ts`, `src/app.rs` and
`src/tray.rs`. The v0.6.0 counterparts are in `desktop-gpui/src/`. Shared tests
exercise the Cursor authorization gate and stale usage, Codex CLI protocol,
Claude CLI and API fallback, all seven languages, SQLite settings and geometry.
Real accounts and startup at OS login require platform acceptance with a test
profile. The fixtures do not use personal provider accounts.

| Surface | Candidate verification |
| --- | --- |
| Linux UI and Debian | Run the native lifecycle and repaint checks in Xvfb. Extract the published Debian packages and run both published executables against isolated settings, then read the same SQLite settings with the packaged candidate. |
| Omarchy and JSON | Run the published Omarchy collectors and the candidate with an empty profile and no display. Verify the JSON snapshot, watch, refresh and EOF protocol, plus the source widget model. |
| Windows | Install the published v0.2.2, v0.5.18 and v0.6.0 packages on a clean runner. Verify process startup, silent upgrade, restart and persisted settings. Exercise the old `/S`, `/S /R /UPDATE` and new `/S /NS` installer invocations. Also run the candidate portable executable against settings migrated by both recent published builds. |
| macOS | Mount the published universal DMGs read-only. Check their digests, bundle identity, architecture and code signature. Run the bundled executables and the candidate against isolated settings. Native CI separately exercises the candidate panel, tray and inactive redraw. |
| Updates | Check the live stable GitHub release and verify retained Windows signatures. The tagged job signs and verifies the new installer with the existing Minisign key. An application-to-application upgrade to the newly signed public artifact requires that artifact to exist. |

Local Linux evidence includes graphical startup from the actual Debian packages
for all three versions, the two published SQLite migrations into the candidate,
and the candidate's lifecycle, inactive repaint and headless protocol checks.
The standalone Omarchy QML smoke ran against the local Quickshell host with
fixtures. The published artifacts match their GitHub digests and both retained
Windows installers pass the candidate's Minisign verifier.

The patch release is `v0.6.1`. Feature-branch pushes and manual CI dispatches
verify the candidate. A push of the matching `v0.6.1` tag starts the release
workflow. After verification it builds the macOS universal DMG, the Windows x64
installer and portable executable, the Linux x64 Debian package and the Omarchy
x64 archive. CI verifies macOS signing and emits the installer signature,
checksums and legacy `latest.json` before making the GitHub release public.

## Reproduction

```sh
npm ci
npm run check
npm test
npm run test:ui
node scripts/smoke-gpui.mjs --idle
# macOS long inactive acceptance:
METRA_IDLE_SMOKE_SECONDS=330 node scripts/smoke-gpui.mjs --idle
# Windows composited pixel evidence:
node scripts/smoke-gpui.mjs --pixels
npm run package
```

The smoke executable is built with `native-smoke`; isolated configuration and
synthetic data are mandatory for idle/pixel scenarios. On headless Linux, wrap
smoke commands with `dbus-run-session -- xvfb-run -a`. Windows pixel captures are
uploaded from `test-results/native-gpui-windows/`. Production builds do not include
these scenarios. The former Tauri shell remains a regression reference, not the
normal npm build output.
