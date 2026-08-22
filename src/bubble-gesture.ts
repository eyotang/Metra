export interface BubbleGestureCompletion {
  dragMoved: boolean;
  clickObserved: boolean;
  releaseConfirmed: boolean;
}

export interface BubbleGestureDecision {
  openPanel: boolean;
  suppressLateClick: boolean;
}

export type BubbleReleaseProbeResult =
  | { status: "released" }
  | { status: "pressed" }
  | { status: "unavailable"; reason: unknown };

export async function probeBubbleRelease(
  isPrimaryButtonPressed: () => Promise<boolean>,
): Promise<BubbleReleaseProbeResult> {
  try {
    return await isPrimaryButtonPressed()
      ? { status: "pressed" }
      : { status: "released" };
  } catch (reason) {
    return { status: "unavailable", reason };
  }
}

export function decideBubbleGestureCompletion(completion: BubbleGestureCompletion): BubbleGestureDecision {
  const openPanel = !completion.dragMoved && (completion.clickObserved || completion.releaseConfirmed);
  return {
    openPanel,
    suppressLateClick: openPanel && !completion.clickObserved,
  };
}
