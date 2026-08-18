import type { ProviderName } from "./types";

const PROVIDER_ROW_CONTROL_SELECTOR = [
  ".drag-handle",
  ".color-trigger",
  "input",
  "textarea",
  "select",
  ".visibility-toggle",
  ".provider-nav",
  "[role=\"switch\"]",
  "[contenteditable=\"true\"]",
].join(", ");

interface ClosestTarget {
  closest(selector: string): unknown;
}

interface ProviderNavigationCard {
  classList: Pick<DOMTokenList, "add" | "remove">;
  focus(options?: FocusOptions): void;
  scrollIntoView(options?: ScrollIntoViewOptions): void;
}

interface ProviderNavigationRoot {
  querySelector(selector: string): ProviderNavigationCard | null;
  querySelectorAll(selector: string): Iterable<ProviderNavigationCard>;
}

interface ProviderCardNavigatorOptions {
  prefersReducedMotion(): boolean;
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
}

export function shouldNavigateFromProviderRow(target: ClosestTarget | null, sorting: boolean): boolean {
  return !sorting && Boolean(target) && !target!.closest(PROVIDER_ROW_CONTROL_SELECTOR);
}

export class ProviderCardNavigator {
  private readonly root: ProviderNavigationRoot;
  private readonly options: ProviderCardNavigatorOptions;
  private highlightTimer: unknown = null;

  constructor(root: ProviderNavigationRoot, options: ProviderCardNavigatorOptions) {
    this.root = root;
    this.options = options;
  }

  navigate(provider: ProviderName): boolean {
    const card = this.root.querySelector(`[data-provider-card="${provider}"]`);
    if (!card) return false;

    if (this.highlightTimer !== null) this.options.cancel(this.highlightTimer);
    for (const candidate of this.root.querySelectorAll("[data-provider-card]")) {
      candidate.classList.remove("is-navigation-target");
    }
    card.classList.add("is-navigation-target");
    card.focus({ preventScroll: true });
    card.scrollIntoView({
      behavior: this.options.prefersReducedMotion() ? "auto" : "smooth",
      block: "start",
      inline: "nearest",
    });
    this.highlightTimer = this.options.schedule(() => {
      card.classList.remove("is-navigation-target");
      this.highlightTimer = null;
    }, 1_400);
    return true;
  }
}
