export type BubbleDockSide = "left" | "right";

export interface BubblePoint {
  x: number;
  y: number;
}

export interface BubbleSize {
  width: number;
  height: number;
}

export interface BubbleMonitorGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
  scaleFactor: number;
}

export interface BubbleMotionSample extends BubblePoint {
  time: number;
}

export interface BubbleDockTarget {
  side: BubbleDockSide;
  position: BubblePoint;
  size: BubbleSize;
}

export const BUBBLE_FULL_LOGICAL_SIZE = 56;
export const BUBBLE_PEEK_LOGICAL_WIDTH = 32;
export const BUBBLE_SAFE_Y = 8;

export function bubblePhysicalSize(scaleFactor: number): BubbleSize {
  return {
    width: Math.round(BUBBLE_FULL_LOGICAL_SIZE * scaleFactor),
    height: Math.round(BUBBLE_FULL_LOGICAL_SIZE * scaleFactor),
  };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(value, maximum));
}

function overlapLength(startA: number, lengthA: number, startB: number, lengthB: number): number {
  return Math.max(0, Math.min(startA + lengthA, startB + lengthB) - Math.max(startA, startB));
}

function overlapArea(position: BubblePoint, size: BubbleSize, monitor: BubbleMonitorGeometry): number {
  return overlapLength(position.x, size.width, monitor.x, monitor.width)
    * overlapLength(position.y, size.height, monitor.y, monitor.height);
}

function distanceToMonitorSquared(position: BubblePoint, size: BubbleSize, monitor: BubbleMonitorGeometry): number {
  const centerX = position.x + size.width / 2;
  const centerY = position.y + size.height / 2;
  const nearestX = clamp(centerX, monitor.x, monitor.x + monitor.width);
  const nearestY = clamp(centerY, monitor.y, monitor.y + monitor.height);
  return (centerX - nearestX) ** 2 + (centerY - nearestY) ** 2;
}

export function selectBubbleMonitor(
  monitors: readonly BubbleMonitorGeometry[],
  position: BubblePoint,
  size: BubbleSize,
): BubbleMonitorGeometry | null {
  if (!monitors.length) return null;
  return monitors.reduce((selected, monitor) => {
    const selectedOverlap = overlapArea(position, size, selected);
    const monitorOverlap = overlapArea(position, size, monitor);
    if (monitorOverlap !== selectedOverlap) return monitorOverlap > selectedOverlap ? monitor : selected;
    return distanceToMonitorSquared(position, size, monitor) < distanceToMonitorSquared(position, size, selected)
      ? monitor
      : selected;
  });
}

export function projectedDistance(velocity: number, decelerationRate = 0.99): number {
  const boundedVelocity = clamp(velocity, -3_000, 3_000);
  return (boundedVelocity / 1_000) * decelerationRate / (1 - decelerationRate);
}

export function bubbleReleaseVelocity(samples: readonly BubbleMotionSample[]): BubblePoint {
  if (samples.length < 2) return { x: 0, y: 0 };
  const latest = samples[samples.length - 1];
  const earliest = [...samples].reverse().find((sample) => latest.time - sample.time >= 32) ?? samples[0];
  const elapsed = latest.time - earliest.time;
  if (elapsed <= 0) return { x: 0, y: 0 };
  return {
    x: ((latest.x - earliest.x) / elapsed) * 1_000,
    y: ((latest.y - earliest.y) / elapsed) * 1_000,
  };
}

export function calculateBubbleDockTarget(
  position: BubblePoint,
  velocity: BubblePoint,
  monitor: BubbleMonitorGeometry,
): BubbleDockTarget {
  const size = bubblePhysicalSize(monitor.scaleFactor);
  const projectedCenterX = position.x + size.width / 2 + projectedDistance(velocity.x);
  const side: BubbleDockSide = projectedCenterX < monitor.x + monitor.width / 2 ? "left" : "right";
  const safeY = Math.round(BUBBLE_SAFE_Y * monitor.scaleFactor);
  const minimumY = monitor.y + safeY;
  const maximumY = Math.max(minimumY, monitor.y + monitor.height - size.height - safeY);
  return {
    side,
    size,
    position: {
      x: side === "left" ? monitor.x : monitor.x + monitor.width - size.width,
      y: Math.round(clamp(position.y + projectedDistance(velocity.y), minimumY, maximumY)),
    },
  };
}

export function calculateBubbleFreeTarget(
  position: BubblePoint,
  monitor: BubbleMonitorGeometry,
): BubbleDockTarget {
  const size = bubblePhysicalSize(monitor.scaleFactor);
  const maximumX = Math.max(monitor.x, monitor.x + monitor.width - size.width);
  const maximumY = Math.max(monitor.y, monitor.y + monitor.height - size.height);
  const clampedPosition = {
    x: Math.round(clamp(position.x, monitor.x, maximumX)),
    y: Math.round(clamp(position.y, monitor.y, maximumY)),
  };
  const side: BubbleDockSide = clampedPosition.x + size.width / 2 < monitor.x + monitor.width / 2
    ? "left"
    : "right";
  return { side, position: clampedPosition, size };
}

export function calculateBubblePeekFrame(
  anchor: BubblePoint,
  fullSize: BubbleSize,
  side: BubbleDockSide,
  scaleFactor: number,
): { position: BubblePoint; size: BubbleSize } {
  const peekWidth = Math.round(BUBBLE_PEEK_LOGICAL_WIDTH * scaleFactor);
  return {
    position: {
      x: side === "left" ? anchor.x : anchor.x + fullSize.width - peekWidth,
      y: anchor.y,
    },
    size: { width: peekWidth, height: fullSize.height },
  };
}
