# GPUI desktop migration acceptance

Branch: `feat/gpui-desktop-migration`. Baseline: v0.5.18 (`8a55f80`).
The isolated worktree excludes unrelated, uncommitted changes in the original
checkout. Candidate version: **0.6.0; not published**.

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
- Run `45560439506` (`ce856e6`): Linux, macOS and Windows native tests and
  installers passed; Windows v0.2.2 `/S` upgrade restarted and preserved settings.
- Run `45562008197` (`8f8c09e`): Linux passed; macOS passed the 330-second inactive
  paint/template check and packaging. Windows failed in new acceptance code due
  to a raw-handle method collision, subsequently corrected.
- Run `45562512563` (`00ed66a`): Linux passed. Windows acceptance-code compilation
  found a glob-import name collision, subsequently corrected. macOS failed the
  Finder activation precondition before its idle wait; the test now waits for
  the actual inactive state, with a bounded seven-second deadline.
- The first full workflow run on the branch (`45561339569`, `8d4450b`) retains
  legacy browser/native regressions alongside GPUI tests. Its Windows failure
  was the corrected Win32 `BOOL` import. It is not final release evidence.
- The v0.6.0 Linux release package built locally through the default packaging
  entry point. Binary identity, package metadata, ownership, permissions,
  notices and checksum were verified. Its production executable passed the
  Omarchy/no-display JSON/watch/refresh/EOF contract. Public artifacts must come
  from the final tagged CI run.
- README screenshots are from the native v0.6.0 application with isolated,
  synthetic provider data, inspected after capture.

The full workflow also caught an unstamped Omarchy source manifest; its version
is now 0.6.0 and local source/archive manifest checks pass.

The v0.6.0 candidate `4fac267` additionally exposed a Windows client/outer-origin
mismatch in composited capture and a macOS activation-policy test precondition.
The native Windows adapter now uses client origins consistently with GPUI content
sizes, including non-client margins during moves. macOS acceptance now requests a
cooperative focus transfer to Finder, checks actual inactive state, and checks
that the later repaint does not reactivate Metra. These corrections require a new
CI run; the failed runs are not treated as passing evidence.

## Remaining release gates

- [ ] Final candidate passes native compilation and lifecycle checks on all platforms.
- [ ] Windows composited pixel test passes for bubble, capsule and panel: corners
      follow two synthetic backdrop colors while center content remains opaque.
- [ ] Final macOS inactive paint/template test passes, including its activation precondition.
- [ ] Final full workflow passes collector, native, legacy regression, Omarchy and signing-tool checks.
- [ ] Tagged release builds universal macOS DMG, Windows installer/portable and
      Linux/Omarchy assets; signatures, metadata, hashes and old-version upgrade pass.
- [ ] Publish English-first/Chinese-second release notes and compatible updater manifest.

No performance improvement or physical mixed-DPI hardware validation is claimed.
Geometry and native monitor APIs have automated coverage; real multi-monitor
pointer interaction remains a hardware coverage limit. macOS local installation
has not been performed on this Linux machine. If delivered separately, it must
use `npm run install:local` and pass that script's signature/xattr checks.

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
