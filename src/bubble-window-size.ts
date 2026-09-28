export interface WindowFrameSize { width: number; height: number }

// Native resize commands acknowledge queueing, not the compositor's final frame.
export async function waitForBubbleWindowSize(
  expected: WindowFrameSize,
  readSize: () => Promise<WindowFrameSize>,
  isCurrent: () => boolean,
  pause: () => Promise<void>,
): Promise<"ready" | "cancelled" | "timeout"> {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    if (!isCurrent()) return "cancelled";
    const actual = await readSize();
    if (!isCurrent()) return "cancelled";
    if (actual.width === expected.width && actual.height === expected.height) return "ready";
    if (attempt < 29) await pause();
  }
  return "timeout";
}
