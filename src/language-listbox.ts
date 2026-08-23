export type LanguageListboxNavigationKey = "ArrowDown" | "ArrowUp" | "Home" | "End";

export function nextLanguageOptionIndex(
  key: LanguageListboxNavigationKey,
  currentIndex: number,
  optionCount: number,
): number | null {
  if (!Number.isInteger(currentIndex) || optionCount <= 0) return null;
  const lastIndex = optionCount - 1;
  const current = Math.min(Math.max(currentIndex, 0), lastIndex);
  if (key === "ArrowDown") return Math.min(current + 1, lastIndex);
  if (key === "ArrowUp") return Math.max(current - 1, 0);
  if (key === "Home") return 0;
  if (key === "End") return lastIndex;
  return null;
}
