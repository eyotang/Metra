import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  decideBubbleGestureCompletion,
  probeBubbleRelease,
} from "../src/bubble-gesture.ts";

const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
const pointerDownMethod = main.match(/private onPointerDown\([\s\S]*?\n  }\n/)?.[0] ?? "";
const finishNativeDragMethod = main.match(/private async finishNativeDrag\([\s\S]*?\n  }\n\n  private isActiveDrag/)?.[0] ?? "";
const cancelFailedProbeMethod = main.match(/private cancelNativeGestureAfterReleaseProbeFailure\([\s\S]*?\n  }\n/)?.[0] ?? "";
const finishUnmovedMethod = main.match(/private finishNativeGestureWithoutMovement\([\s\S]*?\n  }\n/)?.[0] ?? "";

assert.match(
  pointerDownMethod,
  /scheduleDragFinish\(BUBBLE_DRAG_RELEASE_POLL_MS\)/,
  "a Windows click that loses DOM release events must promptly poll the native button state",
);
assert.match(
  finishNativeDragMethod,
  /probeBubbleRelease[\s\S]*releaseProbe\.status === "unavailable"[\s\S]{0,500}BUBBLE_DRAG_RELEASE_PROBE_MAX_FAILURES[\s\S]{0,180}cancelNativeGestureAfterReleaseProbeFailure\(\)[\s\S]{0,60}return;[\s\S]{0,100}nativeReleaseConfirmed = true/,
  "only a successful released-state probe may confirm a missing DOM release",
);
assert.match(
  main,
  /const BUBBLE_DRAG_RELEASE_PROBE_MAX_FAILURES = [1-9]\d*;/,
  "a broken native release probe must fail closed after bounded retries",
);
assert.match(
  cancelFailedProbeMethod,
  /suppressLateClick = true;[\s\S]*awaitingGestureRecovery = true;[\s\S]*nativeDragging = false;/,
  "a failed release probe must consume delayed clicks and block automatic frame changes",
);
assert.doesNotMatch(
  cancelFailedProbeMethod,
  /finishNativeGestureWithoutMovement|transitionToPeek|maybeDockAfterPanelClose|startDock|scheduleIdle/,
  "probe failure cancellation must not open, restore, hide, or dock a window whose release was never confirmed",
);
assert.match(
  main,
  /private scheduleIdle\([\s\S]{0,500}awaitingGestureRecovery/,
  "automatic idle peeking must wait until a canceled native gesture has recovered",
);
assert.match(
  finishUnmovedMethod,
  /decideBubbleGestureCompletion[\s\S]*suppressLateClick/,
  "the native gesture completion path must use the cross-platform click classifier",
);
assert.match(
  main,
  /if \(this\.suppressLateClick\) \{\s*this\.suppressLateClick = false;[\s\S]{0,160}return;/,
  "a delayed WebView click from the completed native gesture must not toggle the panel closed",
);

assert.deepEqual(
  await probeBubbleRelease(async () => false),
  { status: "released" },
  "a successful native probe must confirm a released primary button",
);

assert.deepEqual(
  await probeBubbleRelease(async () => true),
  { status: "pressed" },
  "a held primary button must keep the native gesture active",
);

const probeFailure = new Error("native input state unavailable");
const unavailableProbe = await probeBubbleRelease(async () => { throw probeFailure; });
assert.equal(unavailableProbe.status, "unavailable");
assert.equal(
  unavailableProbe.status === "unavailable" ? unavailableProbe.reason : undefined,
  probeFailure,
  "a failed native probe must not be promoted to a confirmed release",
);

assert.equal(
  decideBubbleGestureCompletion({ dragMoved: false, clickObserved: true, releaseConfirmed: true }).openPanel,
  true,
  "an ordinary completed click must open the details panel",
);

assert.equal(
  decideBubbleGestureCompletion({ dragMoved: false, clickObserved: false, releaseConfirmed: true }).openPanel,
  true,
  "Windows native dragging suppresses the DOM click after ReleaseCapture, so an unmoved confirmed release must still open details",
);

assert.equal(
  decideBubbleGestureCompletion({ dragMoved: true, clickObserved: false, releaseConfirmed: true }).openPanel,
  false,
  "a real drag must not open the details panel",
);

assert.equal(
  decideBubbleGestureCompletion({ dragMoved: false, clickObserved: false, releaseConfirmed: false }).openPanel,
  false,
  "an unconfirmed cancellation must not be treated as a click",
);

assert.deepEqual(
  decideBubbleGestureCompletion({ dragMoved: false, clickObserved: false, releaseConfirmed: true }),
  { openPanel: true, suppressLateClick: true },
  "a native release that substitutes for the missing DOM click must consume any delayed click",
);

assert.deepEqual(
  decideBubbleGestureCompletion({ dragMoved: false, clickObserved: true, releaseConfirmed: true }),
  { openPanel: true, suppressLateClick: false },
  "a normal DOM click must not suppress the user's next gesture",
);

console.log("Bubble click gesture contract: PASS");
