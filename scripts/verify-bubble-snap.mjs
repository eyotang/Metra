import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  bubblePhysicalSize,
  bubbleReleaseVelocity,
  calculateBubbleDockTarget,
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

assert.match(main, /const BUBBLE_IDLE_DELAY_MS = 3_000/, "idle peek must wait three seconds");
assert.match(main, /function idleBubbleValue\(/, "idle mode needs a percentage-only renderer");
assert.match(main, /availableMonitors\(\)/, "dock selection must consider every display");
assert.match(main, /calculateBubblePeekFrame\(/, "idle mode must resize and reposition the native window");
assert.match(main, /data-tauri-drag-region="deep"/, "native dragging must begin on mousedown through Tauri's drag region");
assert.match(main, /private onWindowMoved[\s\S]{0,520}this\.dragMoved = true;[\s\S]{0,80}this\.dragged = true;/, "only actual native window movement may classify the gesture as a drag");
assert.match(main, /private onWindowMoved[\s\S]{0,180}consumeProgrammaticMove\(position\)[\s\S]{0,120}!this\.nativeDragging/, "programmatic positions must be ignored before classifying native drag movement");
assert.match(main, /private onPointerCancel[\s\S]{0,180}nativeReleaseConfirmed = false/, "pointer cancellation must fall back to the real native mouse-button state");
assert.match(main, /private onPointerUp[\s\S]{0,640}scheduleDragFinish\(BUBBLE_DRAG_CLICK_SETTLE_MS\)/, "pointer release must leave time for delayed native move events");
assert.match(main, /event\.screenX[\s\S]{0,220}this\.dragMoved = true;/, "pointer travel must suppress a drag gesture's synthetic click");
assert.match(main, /if \(this\.nativeDragging\) \{\s*this\.pendingClick = true;/, "clicks must wait until the native gesture is classified");
assert.match(main, /currentWindow\.outerPosition\(\)[\s\S]{0,520}this\.dragMoved = true;/, "gesture classification must reconcile the final native window position");
assert.match(main, /if \(this\.finishInProgress\)[\s\S]{0,180}this\.finishInProgress = true;/, "native drag completion must be serialized");
assert.match(main, /is_primary_mouse_button_pressed/, "native drag fallback must confirm the real pointer release");
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
