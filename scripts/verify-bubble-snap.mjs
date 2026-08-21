import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  bubblePhysicalSize,
  bubbleReleaseVelocity,
  calculateBubbleDockTarget,
  calculateBubbleFreeTarget,
  calculateBubblePeekFrame,
  projectedDistance,
  selectBubbleMonitor,
} from "../src/bubble-geometry.ts";

const primary = { x: 0, y: 24, width: 1200, height: 776, scaleFactor: 1 };
const leftDisplay = { x: -1600, y: -120, width: 1600, height: 900, scaleFactor: 1.5 };

assert.deepEqual(bubblePhysicalSize(1), { width: 56, height: 56 });
assert.deepEqual(bubblePhysicalSize(2), { width: 112, height: 112 });

assert.equal(
  selectBubbleMonitor([primary, leftDisplay], { x: -100, y: 100 }, { width: 84, height: 84 }),
  leftDisplay,
  "the display with the greatest window overlap must win",
);
assert.equal(
  selectBubbleMonitor([primary, leftDisplay], { x: 2_000, y: 100 }, { width: 56, height: 56 }),
  primary,
  "an off-screen restored position must fall back to the nearest display",
);

const slowLeft = calculateBubbleDockTarget({ x: 500, y: 300 }, { x: 0, y: 0 }, primary);
assert.equal(slowLeft.side, "left");
assert.deepEqual(slowLeft.position, { x: 0, y: 300 });

const fractionalProjection = calculateBubbleDockTarget({ x: 500, y: 300 }, { x: 0, y: 397 }, primary);
assert.equal(Number.isInteger(fractionalProjection.position.y), true, "native window coordinates must be integers");

const flickRight = calculateBubbleDockTarget({ x: 500, y: 780 }, { x: 2_000, y: 1_500 }, primary);
assert.equal(flickRight.side, "right", "release velocity should be able to carry the bubble across the midpoint");
assert.deepEqual(flickRight.position, { x: 1144, y: 736 }, "the target must respect the work-area bottom inset");

assert.deepEqual(
  calculateBubbleFreeTarget({ x: 500, y: 300 }, primary),
  { side: "left", position: { x: 500, y: 300 }, size: { width: 56, height: 56 } },
  "snap-disabled placement must preserve a visible release position",
);
assert.deepEqual(
  calculateBubbleFreeTarget({ x: 2_000, y: -100 }, primary),
  { side: "right", position: { x: 1144, y: 24 }, size: { width: 56, height: 56 } },
  "snap-disabled placement must keep the full bubble inside the selected work area",
);
assert.deepEqual(
  calculateBubbleFreeTarget({ x: 100, y: 1_000 }, leftDisplay),
  { side: "right", position: { x: -84, y: 696 }, size: { width: 84, height: 84 } },
  "free placement must clamp negative-coordinate, scaled displays in physical pixels",
);
assert.deepEqual(
  calculateBubbleFreeTarget({ x: 500, y: 500 }, { x: -20, y: 10, width: 40, height: 30, scaleFactor: 1 }),
  { side: "right", position: { x: -20, y: 10 }, size: { width: 56, height: 56 } },
  "a work area smaller than the bubble must fall back to its origin",
);

assert.deepEqual(
  calculateBubblePeekFrame({ x: 1144, y: 300 }, { width: 56, height: 56 }, "right", 1),
  { position: { x: 1168, y: 300 }, size: { width: 32, height: 56 } },
  "right peek must preserve the screen-edge anchor",
);
assert.deepEqual(
  calculateBubblePeekFrame({ x: -1600, y: 100 }, { width: 84, height: 84 }, "left", 1.5),
  { position: { x: -1600, y: 100 }, size: { width: 48, height: 84 } },
  "left peek must work with negative coordinates and scaled displays",
);

assert.equal(Math.round(projectedDistance(1_000)), 99);
assert.deepEqual(
  bubbleReleaseVelocity([
    { x: 100, y: 100, time: 0 },
    { x: 130, y: 70, time: 50 },
  ]),
  { x: 600, y: -600 },
);

const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const config = readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8");
const appRust = readFileSync(new URL("../src-tauri/src/app.rs", import.meta.url), "utf8");
const settingsRust = readFileSync(new URL("../src-tauri/src/settings.rs", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/types.ts", import.meta.url), "utf8");

assert.match(main, /const BUBBLE_IDLE_DELAY_MS = 3_000/, "ordinary idle peek must still wait three seconds");
assert.match(main, /function idleBubbleValue\(/, "idle mode needs a percentage-only renderer");
assert.match(main, /availableMonitors\(\)/, "dock selection must consider every display");
assert.match(main, /calculateBubblePeekFrame\(/, "idle mode must resize and reposition the native window");
const beginNativeDragMethod = main.match(/private async beginNativeDrag\([\s\S]*?\n  }\n/)?.[0] ?? "";
const pointerDownMethod = main.match(/private onPointerDown\([\s\S]*?\n  }\n/)?.[0] ?? "";
assert.match(beginNativeDragMethod, /"start_bubble_drag"/, "native dragging must use the serialized native frame gate");
assert.doesNotMatch(beginNativeDragMethod, /invokeWithTimeout/, "a long native drag must not be failed by the ordinary action timeout");
assert.doesNotMatch(main, /data-tauri-drag-region=/, "automatic drag regions must not race a pending native frame transition");
assert.match(main, /this\.beginNativeDrag\([\s\S]{0,180}this\.snapToken,[\s\S]{0,80}restorePosition,[\s\S]{0,80}restoreSize/, "pointerdown must immediately pass the cancellation token and full docked frame to native dragging");
assert.match(pointerDownMethod, /stateBeforeDrag === "snapping" && this\.lastObservedPosition/, "an interrupted snap must normalize the native drag origin before classifying a fast click");
assert.match(pointerDownMethod, /rememberProgrammaticMove\(restorePosition, this\.snapToken, true\)/, "the normalized drag origin must be excluded from real movement samples");
assert.match(appRust, /async fn start_bubble_drag\(/, "native dragging must run off the main thread so macOS dragging cannot block the IPC response");
assert.match(appRust, /fn start_bubble_drag\([\s\S]*accept_bubble_operation[\s\S]*apply_bubble_window_frame[\s\S]*start_bubble_window_drag\(&window\)/, "native dragging must atomically reject stale frames, restore the full frame, and then start dragging");
assert.match(appRust, /fn start_bubble_window_drag\([\s\S]*run_on_main_thread[\s\S]*DispatchQueue::main\(\)\.exec_async[\s\S]*\.start_dragging\(\)/, "macOS dragging must enter the main GCD queue after Tao's asynchronous frame changes");
assert.match(appRust, /fn begin_bubble_window_session\([\s\S]*next_bubble_operation_session/, "every WebView lifetime must receive a fresh native frame session");
assert.match(appRust, /fn set_bubble_window_frame\([\s\S]*sequence[\s\S]*accept_bubble_operation/, "all native frame updates must share the drag operation token gate");
assert.match(appRust, /generate_handler!\[[\s\S]*set_bubble_window_frame,[\s\S]*start_bubble_drag,/, "the serialized frame and drag commands must be registered");
assert.match(main, /this\.nativeSession = await invokeWithTimeout<number>\([\s\S]{0,80}"begin_bubble_window_session"/, "the renderer must claim a new native frame session after reload");
assert.match(main, /requireAcceptedNativeOperation\(accepted, operationToken\)/, "the renderer must not advance visual state after a rejected native frame");
const onWindowMovedMethod = main.match(/private onWindowMoved\([\s\S]*?\n  }\n/)?.[0] ?? "";
assert.match(onWindowMovedMethod, /this\.dragMoved = true;[\s\S]*this\.dragged = true;/, "only actual native window movement may classify the gesture as a drag");
assert.match(onWindowMovedMethod, /consumeProgrammaticMove\(position\)[\s\S]*!this\.nativeDragging/, "programmatic positions must be ignored before classifying native drag movement");
assert.match(main, /programmaticMove\.token !== undefined && programmaticMove\.token !== this\.snapToken\) return/, "a stale frame event must not replace the active drag position");
assert.match(main, /private onPointerCancel[\s\S]{0,180}nativeReleaseConfirmed = false/, "pointer cancellation must fall back to the real native mouse-button state");
assert.match(main, /private onPointerUp[\s\S]{0,640}scheduleDragFinish\(BUBBLE_DRAG_CLICK_SETTLE_MS\)/, "pointer release must leave time for delayed native move events");
assert.match(main, /event\.screenX[\s\S]{0,220}this\.dragMoved = true;/, "pointer travel must suppress a drag gesture's synthetic click");
assert.match(main, /if \(this\.nativeDragging\) \{\s*this\.pendingClick = true;/, "clicks must wait until the native gesture is classified");
assert.match(main, /currentWindow\.outerPosition\(\)[\s\S]{0,800}this\.dragMoved = true;/, "gesture classification must reconcile the final native window position");
assert.match(main, /const position = await this\.afterNativeQueue\(\(\) => currentWindow\.outerPosition\(\)\)[\s\S]{0,620}this\.movementSamples\.push\(\{ x: position\.x, y: position\.y/, "every release must sample the final native position even when pointer travel was detected first");
assert.match(main, /if \(this\.finishInProgress\)[\s\S]{0,180}this\.finishInProgress = true;/, "native drag completion must be serialized");
assert.match(main, /is_primary_mouse_button_pressed/, "native drag fallback must confirm the real pointer release");
assert.match(main, /this\.startDock\(velocity, true, "peek-now", releasePosition\)/, "a completed drag must request immediate half-hide after snapping");
assert.match(main, /followup === "peek-now"[\s\S]{0,100}enterPeekAfterDock\(token\)/, "post-drag snapping must enter peek without scheduling the idle timer");
const immediatePeekMethod = main.match(/private async enterPeekAfterDock\([\s\S]*?\n  }\n/)?.[0] ?? "";
assert.match(immediatePeekMethod, /transitionToPeek\(token\)/, "immediate post-drag peek must use the tokenized native half-width transition");
assert.doesNotMatch(immediatePeekMethod, /busy|hovering|focused/, "activity or pointer presence must not make the first snapped half-hide wait for idle");
assert.match(main, /private async transitionToPeek\(token: number\)[\s\S]{0,520}runFrameTransition\(frame\.position, frame\.size, token\)/, "a newer drag must cancel a queued post-drag peek transition");
assert.match(main, /private setWindowFrame\([\s\S]{0,300}operationToken !== this\.snapToken/, "a canceled peek must be rejected at the frontend frame queue boundary");
assert.match(main, /private async reveal\([\s\S]{0,620}runFrameTransition\(this\.anchor, this\.fullSize, token\)[\s\S]{0,120}token !== this\.snapToken\) return/, "a new drag must also cancel an interrupted reveal before it can re-dock the window");
assert.match(main, /const releasePosition = latest[\s\S]{0,420}this\.startDock\(velocity, true, "peek-now", releasePosition\)/, "drag completion must preserve the latest real release position");
assert.match(main, /const \[windowPosition, size, monitors\][\s\S]{0,360}const position = releasePosition \?\? windowPosition/, "docking must wait for the native queue and then prefer the real release position over a stale frame");
assert.match(main, /animateTo\(target\.position, velocity, token, releasePosition\)/, "snap animation must start from the reconciled release position");
assert.match(main, /rememberProgrammaticMove\(rounded, operationToken\)/, "queued native moves must retain their snap token for stale-event filtering");
assert.match(main, /filter\(\(candidate\) => this\.nativeDragging \|\| candidate\.expiresAt > now\)/, "programmatic move markers must survive a long native drag until their events are delivered");
assert.doesNotMatch(main, /currentWindow\.set(?:Position|Size)\(/, "bubble frame updates must not bypass the native token gate");
assert.match(main, /calculateBubbleFreeTarget\(/, "snap-disabled dragging must preserve a free position");
assert.match(main, /if \(!this\.snapEnabled/, "idle half-hide must be disabled with edge snapping");
assert.match(main, /window\.addEventListener\("blur"[\s\S]{0,120}this\.focused = false;[\s\S]{0,80}this\.scheduleIdle\(\)/, "losing native window focus must allow idle peek even when the button retains DOM focus");
assert.match(main, /data-action="snap"[\s\S]{0,220}bubbleSnapEnabled/, "the context menu must expose the edge-snap switch");
assert.match(main, /data-action="snap" role="switch" aria-checked="\$\{s\.bubbleSnapEnabled\}"/, "the edge-snap switch must expose its state to assistive technology");
assert.match(main, /set_bubble_snap_enabled/, "the edge-snap switch must persist through the native settings service");
assert.match(types, /bubbleSnapEnabled:\s*boolean/, "frontend settings must expose the edge-snap preference");
assert.match(settingsRust, /bubble_snap_enabled:\s*false/, "edge snapping must default to disabled");
assert.match(appRust, /fn set_bubble_snap_enabled\(/, "the native edge-snap settings command is missing");
const snapSettingsCommand = appRust.match(/fn set_bubble_snap_enabled\([\s\S]*?\n}\n/)?.[0] ?? "";
assert.match(snapSettingsCommand, /bubble_snap_enabled = enabled/, "the native edge-snap command must persist the requested value");
assert.match(snapSettingsCommand, /app\.emit\("settings-updated"/, "the native edge-snap command must notify the bubble immediately");
assert.match(appRust, /generate_handler!\[[\s\S]*?set_bubble_snap_enabled/, "the native edge-snap command must be registered");
assert.match(main, /if \(requestId !== panelRequestSequence\) return;/, "older panel requests must not overtake newer interactions");
assert.doesNotMatch(main, /function clampAndSave\(/, "the previous full-window clamp must not fight snap or peek moves");
assert.match(css, /data-state="idle"/, "the idle visual state is missing");
assert.match(css, /\.bubble-idle-value[^}]*var\(--provider-color\)/, "idle percentages must retain provider colors");
assert.match(css, /\.bubble-idle-usage[^}]*width:\s*32px[^}]*align-items:\s*flex-end/, "right-docked percentages must reach the right screen edge");
assert.match(css, /\.bubble-shell\[data-side="left"\] \.bubble-idle-usage[^}]*align-items:\s*flex-start/, "left-docked percentages must reach the left screen edge");
assert.match(config, /"minWidth":\s*32/, "the native bubble must be allowed to shrink to the peek width");
assert.match(config, /"maxWidth":\s*56/, "the native bubble must remain bounded to the full width");
assert.match(appRust, /panel_bubble_x\(/, "panel positioning must account for a half-width bubble");
assert.match(appRust, /panel-visibility-changed/, "the bubble must know when panel interaction blocks idle mode");
assert.match(appRust, /fn next_panel_request\(/, "tray and bubble panel requests must share one native sequence");
assert.match(appRust, /fn is_primary_mouse_button_pressed\(/, "native drag release confirmation command is missing");

console.log("Bubble snap and idle geometry: PASS");
