import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { availableMonitors, getCurrentWindow, type PhysicalPosition } from "@tauri-apps/api/window";
import {
  bubbleReleaseVelocity,
  calculateBubbleDockTarget,
  calculateBubbleFreeTarget,
  calculateBubblePeekFrame,
  selectBubbleMonitor,
  type BubbleDockSide,
  type BubbleMonitorGeometry,
  type BubbleMotionSample,
  type BubblePoint,
  type BubbleSize,
} from "./bubble-geometry";
import type { AppPayload, AppSettings, BubblePercentMode, ProviderName, ProviderSnapshot, ProviderStatus, QuotaKind, UiLanguage } from "./types";
import { ProviderCardNavigator, shouldNavigateFromProviderRow } from "./provider-navigation";
import {
  applyDocumentLocale,
  detectLocale,
  i18n,
  localizeProviderMessage,
  localizeQuotaLabel,
  t,
  type SupportedLocale,
  type TranslationKey,
} from "./i18n";
import metraWaterUrl from "./metra-water.svg";
import "./styles.css";

applyDocumentLocale(i18n.locale);

const app = document.querySelector<HTMLDivElement>("#app")!;
const providerCardNavigator = new ProviderCardNavigator(document, {
  prefersReducedMotion: () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  schedule: (callback, delay) => window.setTimeout(callback, delay),
  cancel: (handle) => window.clearTimeout(handle as number),
});
const currentWindow = getCurrentWindow();
const view = new URLSearchParams(location.search).get("view") ?? "bubble";
let payload: AppPayload | null = null;
let pendingSettings: AppSettings | null = null;
let activeLanguagePreference: UiLanguage | null = null;
let panelMode: "details" | "menu" = "details";
let panelDockSide: BubbleDockSide = "right";
let panelRequestSequence = 0;
const MENU_PANEL_HEIGHT = 480;
const PANEL_GAP = 3;
const ACTION_TIMEOUT_MS = 8_000;
const PANEL_SHOW_TIMEOUT_MS = 1_000;
const REFRESH_TIMEOUT_MS = 22_000;
const BUBBLE_IDLE_DELAY_MS = 3_000;
const BUBBLE_DRAG_FALLBACK_MS = 800;
const BUBBLE_DRAG_RELEASE_POLL_MS = 120;
const BUBBLE_DRAG_CLICK_SETTLE_MS = 80;
const BUBBLE_POINTER_DRAG_THRESHOLD_PX = 4;
const BUBBLE_SAMPLE_WINDOW_MS = 140;
const BUBBLE_PROGRAMMATIC_MOVE_TTL_MS = 180;
const detectedSystemLocale = detectLocale();

interface CursorLoginStart {
  method: "agent" | "editor";
  alreadyRunning: boolean;
}

function metraLogo(): string {
  return `<img class="logo-mark" src="${metraWaterUrl}" alt="" aria-hidden="true">`;
}

const STATUS_TEXT_KEYS: Record<ProviderStatus, TranslationKey> = {
  available: "status.available",
  desktop_installed: "status.desktopInstalled",
  not_installed: "status.notInstalled",
  not_logged_in: "status.notLoggedIn",
  unsupported: "status.unsupported",
  network_error: "status.networkError",
  protocol_error: "status.protocolError",
};
const PROVIDER_ORDER = ["cursor", "codex", "claude"] as const satisfies readonly ProviderName[];
const COLOR_PALETTE = [
  ["#ff6b6b", "#e99068", "#ffc21a", "#91b800", "#34b84a", "#4bd8c0", "#2da9dc", "#7698ee", "#d86bb3", "#9c83ff", "#949aa4"],
  ["#ffe0df", "#ffe3c7", "#fff0c2", "#e4f3ad", "#cceecd", "#c5f0eb", "#caebf7", "#dce6fb", "#f4d9e9", "#e8def9", "#e8eaed"],
  ["#ffb5b0", "#ffc98f", "#ffe08a", "#c9e45f", "#92da96", "#72d8cc", "#80d1ed", "#b4c8f6", "#efb2d7", "#d2bdf2", "#cbd0d6"],
  ["#ef4e48", "#eb6f17", "#dfa20a", "#749900", "#2fa43d", "#119b88", "#158eb9", "#487bea", "#c23f91", "#8752df", "#656c76"],
  ["#cf332d", "#a84d08", "#8f6508", "#496600", "#208c2b", "#087164", "#0b6787", "#2456d9", "#98246e", "#6d2bd1", "#3f454d"],
] as const satisfies readonly (readonly string[])[];
const COLOR_PALETTE_SET = new Set<string>(COLOR_PALETTE.flat());
const COLOR_HUE_KEYS = [
  "color.hue.red", "color.hue.orange", "color.hue.yellow", "color.hue.lime",
  "color.hue.green", "color.hue.cyan", "color.hue.sky", "color.hue.blue",
  "color.hue.pink", "color.hue.purple", "color.hue.gray",
] as const satisfies readonly TranslationKey[];
const COLOR_TONE_KEYS = [
  "color.tone.standard", "color.tone.light", "color.tone.soft", "color.tone.deep",
  "color.tone.dark",
] as const satisfies readonly TranslationKey[];
const UI_LANGUAGE_OPTIONS = [
  { value: "system", labelKey: "menu.languageSystem" },
  { value: "zh-CN", labelKey: "menu.languageZhCn" },
  { value: "en", labelKey: "menu.languageEnglish" },
  { value: "ja", labelKey: "menu.languageJapanese" },
  { value: "ko", labelKey: "menu.languageKorean" },
] as const;
const PROVIDER_META: Record<ProviderName, { name: string; fallbackLabel: string; fallbackColor: string }> = {
  cursor: { name: "Cursor", fallbackLabel: "C", fallbackColor: "#9c83ff" },
  codex: { name: "Codex", fallbackLabel: "X", fallbackColor: "#4bd8c0" },
  claude: { name: "Claude Code", fallbackLabel: "A", fallbackColor: "#e99068" },
};

function escapeHtml(value: unknown): string {
  return String(value ?? "").replace(/[&<>'"]/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[char]!);
}
function bubbleProviderOrder(settings?: AppSettings): ProviderName[] {
  const configured = settings?.bubbleProviderOrder ?? [];
  const order = configured.filter((provider, index) =>
    PROVIDER_ORDER.includes(provider) && configured.indexOf(provider) === index,
  );
  return [...order, ...PROVIDER_ORDER.filter((provider) => !order.includes(provider))];
}
function bubbleVisibleProviderOrder(settings?: AppSettings): ProviderName[] {
  const order = bubbleProviderOrder(settings);
  const configured = settings?.bubbleVisibleProviders;
  if (!configured) return order;
  const visible = order.filter((provider) => configured.includes(provider));
  return visible.length ? visible : order;
}
function bubbleProviderLabel(provider: ProviderName, settings?: AppSettings): string {
  const label = {
    cursor: settings?.cursorBubbleLabel,
    codex: settings?.codexBubbleLabel,
    claude: settings?.claudeBubbleLabel,
  }[provider];
  return label?.trim() || PROVIDER_META[provider].fallbackLabel;
}
function normalizeProviderColor(provider: ProviderName, value?: string): string {
  const color = value?.trim().toLowerCase();
  return color && COLOR_PALETTE_SET.has(color) ? color : PROVIDER_META[provider].fallbackColor;
}
function bubbleProviderColor(provider: ProviderName, settings?: AppSettings): string {
  const color = {
    cursor: settings?.cursorBubbleColor,
    codex: settings?.codexBubbleColor,
    claude: settings?.claudeBubbleColor,
  }[provider];
  return normalizeProviderColor(provider, color);
}
function providerColorStyle(provider: ProviderName, settings?: AppSettings): string {
  return `--provider-color:${bubbleProviderColor(provider, settings)}`;
}
function bubblePercent(provider: ProviderSnapshot, mode: BubblePercentMode): number | null {
  if (!provider.quotas.length) return null;
  return mode === "used"
    ? Math.max(...provider.quotas.map((quota) => quota.usedPercent))
    : Math.min(...provider.quotas.map((quota) => quota.remainingPercent));
}
function percent(value: number | null): string { return value === null ? "--" : `${Math.round(value)}%`; }
function severity(value: number | null, mode: BubblePercentMode): string {
  if (value === null) return "unknown";
  return mode === "used"
    ? value >= 95 ? "critical" : value >= 80 ? "warning" : "healthy"
    : value <= 5 ? "critical" : value <= 20 ? "warning" : "healthy";
}
function number(value?: number): string { return i18n.formatNumber(value); }
function planName(value?: string): string {
  if (!value) return t("plan.unknown");
  const known: Record<string, string> = { free: "Free", hobby: "Hobby", plus: "Plus", pro: "Pro", ultra: "Ultra", team: "Team", business: "Business", enterprise: "Enterprise" };
  return known[value.toLowerCase()] ?? value;
}
const CURSOR_PRO_INCLUDED_FALLBACK_CENTS = 2_000;
const CURSOR_ON_DEMAND_FALLBACK_CENTS = 50_000;
const CURSOR_ULTRA_QUOTA_KINDS = ["cursor_models", "other_models", "grok_bot"] as const satisfies readonly QuotaKind[];
const CURSOR_ULTRA_QUOTA_META: Record<QuotaKind, { label: string; hintKey: TranslationKey }> = {
  cursor_models: { label: "Cursor Models", hintKey: "quota.cursorModelsHint" },
  other_models: { label: "Other Models", hintKey: "quota.otherModelsHint" },
  grok_bot: { label: "Grok Bot", hintKey: "quota.grokBotHint" },
};
function money(cents?: number): string { return cents === undefined ? "—" : `$${(cents / 100).toFixed(2)}`; }
function cursorIncludedLimit(provider: ProviderSnapshot): number {
  if (provider.cost?.includedLimitCents && provider.cost.includedLimitCents > 0) return provider.cost.includedLimitCents;
  return CURSOR_PRO_INCLUDED_FALLBACK_CENTS;
}
function dateTime(value?: string): string { return i18n.formatDateTime(value); }
class ActionTimeoutError extends Error {}
type ToastTone = "loading" | "success" | "error" | "info";
let toastTimer: number | undefined;
let refreshInFlight: Promise<void> | null = null;
let cursorLoginPending = false;
let cursorLoginRecheckInFlight: Promise<void> | null = null;
type TokenProvider = ProviderName;
const TOKEN_GAIN_VISIBLE_MS = 4_800;
let tokenGains: Partial<Record<TokenProvider, number>> = {};
let tokenGainTimer: number | undefined;

function compactTokenDelta(tokens: number): string {
  return tokens >= 1_000_000 ? `${(tokens / 1_000_000).toFixed(2)}M` : `${(tokens / 1_000).toFixed(2)}K`;
}

function providerTokenCounter(provider: ProviderSnapshot): number | undefined {
  return provider.tokens?.lifetime ?? provider.tokens?.today;
}

function applyUsageUpdate(updated: AppPayload): void {
  if (view === "bubble" && payload) {
    const gains: Partial<Record<TokenProvider, number>> = {};
    for (const provider of PROVIDER_ORDER) {
      const previous = providerTokenCounter(payload.snapshot[provider]);
      const next = providerTokenCounter(updated.snapshot[provider]);
      if (!updated.snapshot[provider].stale && previous !== undefined && next !== undefined && next > previous) {
        gains[provider] = next - previous;
      }
    }
    if (Object.keys(gains).length) {
      tokenGains = gains;
      window.clearTimeout(tokenGainTimer);
      tokenGainTimer = window.setTimeout(() => { tokenGains = {}; render(); }, TOKEN_GAIN_VISIBLE_MS);
    }
  }
  if (updated.snapshot.cursor.status === "available") cursorLoginPending = false;
  const uiLanguage = activeLanguagePreference ?? updated.settings.uiLanguage;
  applyLanguagePreference(uiLanguage);
  payload = { ...updated, settings: { ...updated.settings, uiLanguage } };
  render();
}

function withTimeout<T>(operation: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new ActionTimeoutError(t("action.timeout", { action: label }))), timeoutMs);
    operation.then(
      (value) => { window.clearTimeout(timer); resolve(value); },
      (reason) => { window.clearTimeout(timer); reject(reason); },
    );
  });
}

function invokeWithTimeout<T>(command: string, args?: Record<string, unknown>, timeoutMs = ACTION_TIMEOUT_MS, label = t("common.operation")): Promise<T> {
  return withTimeout(invoke<T>(command, args), timeoutMs, label);
}

function effectiveLocale(language: UiLanguage): SupportedLocale {
  return language === "system" ? detectedSystemLocale : language;
}

function applyLanguagePreference(language: UiLanguage, forceNativeSync = false): SupportedLocale {
  const locale = effectiveLocale(language);
  const changed = activeLanguagePreference !== language || i18n.locale !== locale;
  activeLanguagePreference = language;
  i18n.setLocale(locale);
  applyDocumentLocale(locale);
  if (view === "bubble" && (changed || forceNativeSync)) {
    void invokeWithTimeout<void>("set_runtime_locale", { locale })
      .catch(() => undefined);
  }
  return locale;
}

function friendlyError(reason: unknown, fallback: string): string {
  if (reason instanceof ActionTimeoutError) return reason.message;
  const message = String(reason ?? "").trim();
  if (!message || message === "[object Object]") return fallback;
  return i18n.locale !== "zh-CN" && /[\p{Script=Han}]/u.test(message) ? fallback : message;
}

function showToast(message: string, tone: ToastTone = "info", durationMs = 2_800): void {
  window.clearTimeout(toastTimer);
  document.querySelector(".action-toast")?.remove();
  const toast = document.createElement("div");
  toast.className = `action-toast ${tone}`;
  toast.setAttribute("role", tone === "error" ? "alert" : "status");
  toast.innerHTML = `<i aria-hidden="true"></i><span>${escapeHtml(message)}</span>`;
  document.body.append(toast);
  requestAnimationFrame(() => toast.classList.add("visible"));
  if (durationMs > 0) {
    toastTimer = window.setTimeout(() => {
      toast.classList.remove("visible");
      window.setTimeout(() => toast.remove(), 180);
    }, durationMs);
  }
}

async function runUiAction<T>(pending: string, success: string, operation: () => Promise<T>): Promise<T | undefined> {
  showToast(pending, "loading", 0);
  try {
    const result = await operation();
    if (success) showToast(success, "success");
    return result;
  } catch (reason) {
    showToast(friendlyError(reason, t("action.failed", { action: pending })), "error", 4_500);
    return undefined;
  }
}

async function refreshWithFeedback(label = t("refresh.action"), includeCursor?: boolean): Promise<void> {
  if (refreshInFlight) {
    showToast(t("refresh.inProgress"), "loading", 0);
    return refreshInFlight;
  }
  const task = (async () => {
    const hasInlineProgress = view === "panel" && panelMode === "details";
    if (hasInlineProgress) {
      window.clearTimeout(toastTimer);
      document.querySelector(".action-toast")?.remove();
    } else {
      showToast(t("refresh.reading", { action: label }), "loading", 0);
    }
    let unlisten: (() => void) | undefined;
    try {
      let complete!: (value: AppPayload) => void;
      const completed = new Promise<AppPayload>((resolve) => { complete = resolve; });
      unlisten = await withTimeout(
        listen<AppPayload>("usage-updated", (event) => complete(event.payload)),
        ACTION_TIMEOUT_MS,
        t("refresh.listen"),
      );
      if (payload) {
        payload.snapshot.refreshing = true;
        render();
      }
      const cursorWillRefresh = includeCursor ?? payload?.settings.cursorCompatEnabled ?? false;
      await invokeWithTimeout<AppPayload>("refresh_now", { includeCursor }, ACTION_TIMEOUT_MS, t("refresh.start"));
      const updated = await withTimeout(completed, REFRESH_TIMEOUT_MS, label);
      applyUsageUpdate(updated);
      showToast(cursorWillRefresh
        ? t("refresh.success", { action: label })
        : t("refresh.successCursorSkipped", { action: label }), "success");
    } catch (reason) {
      if (payload) {
        payload.snapshot.refreshing = false;
        render();
      }
      showToast(friendlyError(reason, t("action.failed", { action: label })), "error", 4_500);
    } finally {
      unlisten?.();
    }
  })();
  refreshInFlight = task.finally(() => { refreshInFlight = null; });
  return refreshInFlight;
}
async function loadPayload(): Promise<void> {
  if (view === "bubble") renderBubble();
  else renderLoadingPanel();
  try {
    const loaded = await invokeWithTimeout<AppPayload>("get_app_payload", undefined, ACTION_TIMEOUT_MS, t("refresh.readUsage"));
    if (pendingSettings) loaded.settings = pendingSettings;
    payload = loaded;
    pendingSettings = null;
    applyLanguagePreference(payload.settings.uiLanguage, true);
    render();
  } catch (reason) {
    if (view === "panel") showToast(friendlyError(reason, t("refresh.readUsageFailed")), "error", 4_500);
  }
}

function renderLoadingPanel(): void {
  app.innerHTML = `<main class="panel details-panel loading-panel">
    <div class="panel-title"><div class="panel-brand">${metraLogo()}<div><strong>Metra</strong><small>${t("app.usageSubtitle")}</small></div></div></div>
    <div class="loading-state"><i></i><strong>${t("loading.ready")}</strong><small>${t("loading.usage")}</small></div>
  </main>`;
}


function bubbleRow(label: string, provider: ProviderSnapshot, mode: BubblePercentMode, tokenGain?: number): string {
  const value = bubblePercent(provider, mode);
  return `<div class="bubble-row ${provider.provider} ${severity(value, mode)} ${provider.stale ? "stale" : ""} ${tokenGain !== undefined ? "has-token-gain" : ""}" data-provider-accent="${provider.provider}" style="${providerColorStyle(provider.provider, payload?.settings)}">
    ${tokenGain !== undefined ? `<strong class="token-gain">${escapeHtml(label)}+${compactTokenDelta(tokenGain)}</strong>` : `<span class="provider-mark">${escapeHtml(label)}</span><strong>${percent(value)}</strong>`}
  </div>`;
}

function idleBubbleValue(provider: ProviderSnapshot, mode: BubblePercentMode): string {
  return `<span class="bubble-idle-value ${provider.provider} ${provider.stale ? "stale" : ""}" data-provider-accent="${provider.provider}" style="${providerColorStyle(provider.provider, payload?.settings)}">${percent(bubblePercent(provider, mode))}</span>`;
}

type BubbleWindowState = "visible" | "dragging" | "snapping" | "peek";
type BubbleDockFollowup = "schedule-idle" | "peek-now" | "none";
type PendingProgrammaticMove = BubblePoint & { expiresAt: number; token?: number };

class BubbleWindowController {
  private readonly shell: HTMLElement;
  private readonly center: HTMLElement;
  private readonly idleUsage: HTMLElement;
  private readonly reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  private readonly initialization: Promise<void>;
  private state: BubbleWindowState = "visible";
  private side: BubbleDockSide = "right";
  private anchor: BubblePoint | null = null;
  private fullSize: BubbleSize | null = null;
  private monitor: BubbleMonitorGeometry | null = null;
  private busy = true;
  private panelVisible = false;
  private hovering = false;
  private focused = false;
  private initialized = false;
  private snapEnabled: boolean;
  private docked = false;
  private dockAfterPanelClose = false;
  private dragged = false;
  private dragMoved = false;
  private primaryPointerDown = false;
  private nativeDragging = false;
  private nativeReleaseConfirmed = false;
  private pendingClick = false;
  private dragStartedFromPeek = false;
  private pointerStartScreen: BubblePoint | null = null;
  private dragStartPosition: BubblePoint | null = null;
  private lastObservedPosition: BubblePoint | null = null;
  private movementSamples: BubbleMotionSample[] = [];
  private idleTimer: number | undefined;
  private dragSettleTimer: number | undefined;
  private dragGeneration = 0;
  private finishInProgress = false;
  private snapToken = 0;
  private nativeSession = 0;
  private nativeSessionReady: Promise<void>;
  private snapCompletion: Promise<void> | null = null;
  private frameTransition: Promise<void> | null = null;
  private dragStartCompletion: Promise<void> = Promise.resolve();
  private nativeQueue: Promise<unknown> = Promise.resolve();
  private positionSaveQueue: Promise<unknown> = Promise.resolve();
  private pendingProgrammaticMoves: PendingProgrammaticMove[] = [];

  constructor(snapEnabled: boolean) {
    this.shell = document.querySelector<HTMLElement>(".bubble-shell")!;
    this.center = document.querySelector<HTMLElement>(".bubble-center")!;
    this.idleUsage = document.querySelector<HTMLElement>(".bubble-idle-usage")!;
    this.snapEnabled = snapEnabled;
    this.nativeSessionReady = this.beginNativeWindowSession();
    this.bindEvents();
    this.initialization = this.initialize();
    void this.initialization.catch((reason) => this.reportError(reason, t("bubble.error.initializePosition")));
  }

  update(activeRows: string, idleValues: string, order: ProviderName[], mode: BubblePercentMode, busy: boolean, snapEnabled: boolean): void {
    this.center.className = `bubble-center provider-count-${order.length}`;
    this.center.innerHTML = activeRows;
    this.idleUsage.className = `bubble-idle-usage provider-count-${order.length}`;
    this.idleUsage.innerHTML = idleValues;
    const usage = order.map((provider) => {
      const snapshot = payload?.snapshot[provider];
      return `${PROVIDER_META[provider].name} ${snapshot ? percent(bubblePercent(snapshot, mode)) : t("bubble.loading")}`;
    }).join(t("bubble.usageSeparator"));
    const dragHint = snapEnabled ? t("bubble.snapHint") : t("bubble.freeHint");
    this.shell.setAttribute("aria-label", t("bubble.ariaLabel", { usage, dragHint }));
    this.setSnapEnabled(snapEnabled);
    const wasBusy = this.busy;
    this.busy = busy;
    this.applyVisualState();
    if (busy) {
      this.clearIdleTimer();
      if (this.state === "peek") void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.wake")));
    } else if (wasBusy || this.state === "visible") {
      this.scheduleIdle();
    }
  }

  async prepareForPanel(): Promise<void> {
    await this.initialization;
    if (this.snapCompletion) await this.snapCompletion;
    await this.reveal();
    this.clearIdleTimer();
  }

  setPanelVisible(visible: boolean): void {
    this.panelVisible = visible;
    this.shell.setAttribute("aria-expanded", String(visible));
    if (visible) {
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.wake")));
    } else if (!this.maybeDockAfterPanelClose()) {
      this.scheduleIdle();
    }
  }

  private setSnapEnabled(enabled: boolean): void {
    if (this.snapEnabled === enabled) return;
    this.snapEnabled = enabled;
    this.clearIdleTimer();
    this.cancelSnap();
    if (!enabled) {
      this.docked = false;
      this.dockAfterPanelClose = false;
      if (this.state === "snapping") {
        this.state = "visible";
        this.applyVisualState();
      }
      if (this.state === "peek") {
        void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.disableSnap")));
      } else {
        void this.initialization.then(() => {
          if (!this.snapEnabled && !this.primaryPointerDown && !this.nativeDragging) {
            this.startDock({ x: 0, y: 0 }, false);
          }
        }).catch((reason) => this.reportError(reason, t("bubble.error.disableSnap")));
      }
      return;
    }
    this.dockAfterPanelClose = true;
    void this.initialization.then(() => {
      if (!this.snapEnabled || this.docked) {
        this.dockAfterPanelClose = false;
        this.scheduleIdle();
        return;
      }
      this.maybeDockAfterPanelClose();
    }).catch((reason) => this.reportError(reason, t("bubble.error.enableSnap")));
  }

  private maybeDockAfterPanelClose(): boolean {
    if (!this.snapEnabled || !this.dockAfterPanelClose) return false;
    if (this.panelVisible || this.primaryPointerDown || this.nativeDragging) return true;
    this.startDock({ x: 0, y: 0 }, true);
    return true;
  }

  private bindEvents(): void {
    this.shell.addEventListener("selectstart", (event) => event.preventDefault());
    this.shell.addEventListener("pointerenter", () => {
      this.hovering = true;
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.wake")));
    });
    this.shell.addEventListener("pointerleave", () => {
      this.hovering = false;
      this.scheduleIdle();
    });
    this.shell.addEventListener("focusin", () => {
      this.focused = true;
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.wake")));
    });
    this.shell.addEventListener("focusout", () => {
      this.focused = false;
      this.scheduleIdle();
    });
    window.addEventListener("focus", () => {
      if (document.activeElement !== this.shell) return;
      this.focused = true;
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, t("bubble.error.wake")));
    });
    window.addEventListener("blur", () => {
      this.focused = false;
      this.scheduleIdle();
    });
    this.shell.addEventListener("pointerdown", (event) => this.onPointerDown(event));
    window.addEventListener("pointerup", (event) => this.onPointerUp(event));
    window.addEventListener("pointercancel", () => this.onPointerCancel());
    this.shell.addEventListener("click", () => {
      if (this.dragged) return;
      if (this.nativeDragging) {
        this.pendingClick = true;
        return;
      }
      void showPanel("details", true);
    });
    this.shell.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      void showPanel("menu");
    });
    this.shell.addEventListener("keydown", (event) => {
      if (event.repeat || (event.key !== "Enter" && event.key !== " ")) return;
      event.preventDefault();
      void showPanel("details", true);
    });
  }

  private async initialize(): Promise<void> {
    await this.nativeSessionReady;
    void currentWindow.onMoved((event) => this.onWindowMoved(event.payload))
      .catch((reason) => this.reportError(reason, t("bubble.error.watchMovement")));
    await this.dockCurrentPosition({ x: 0, y: 0 }, false);
    this.initialized = true;
    this.applyVisualState();
    if (!this.busy) this.scheduleIdle();
  }

  private onPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    window.getSelection()?.removeAllRanges();
    this.clearIdleTimer();
    const stateBeforeDrag = this.state;
    const interruptedFrame = stateBeforeDrag === "peek" || this.frameTransition !== null;
    this.cancelSnap();
    this.dragStartedFromPeek = stateBeforeDrag === "peek";
    this.dragStartPosition = this.lastObservedPosition
      ? { ...this.lastObservedPosition }
      : this.state === "peek" && this.anchor && this.fullSize && this.monitor
      ? calculateBubblePeekFrame(this.anchor, this.fullSize, this.side, this.monitor.scaleFactor).position
      : this.anchor ? { ...this.anchor } : null;
    this.primaryPointerDown = true;
    this.nativeDragging = true;
    this.nativeReleaseConfirmed = false;
    this.dragged = false;
    this.dragMoved = false;
    this.pendingClick = false;
    this.pointerStartScreen = { x: event.screenX, y: event.screenY };
    this.dragGeneration += 1;
    this.movementSamples = this.dragStartPosition
      ? [{ ...this.dragStartPosition, time: performance.now() }]
      : [];
    const restorePosition = interruptedFrame && this.anchor && this.fullSize
      ? { ...this.anchor }
      : stateBeforeDrag === "snapping" && this.lastObservedPosition
      ? { ...this.lastObservedPosition }
      : undefined;
    const restoreSize = interruptedFrame && restorePosition ? this.fullSize ?? undefined : undefined;
    if (restorePosition) {
      this.lastObservedPosition = { ...restorePosition };
      this.dragStartPosition = { ...restorePosition };
      this.movementSamples = [{ ...restorePosition, time: performance.now() }];
      this.rememberProgrammaticMove(restorePosition, this.snapToken, true);
    }
    this.state = "dragging";
    this.applyVisualState();
    this.scheduleDragFinish(BUBBLE_DRAG_FALLBACK_MS);
    this.dragStartCompletion = this.beginNativeDrag(
      this.dragGeneration,
      this.snapToken,
      restorePosition,
      restoreSize,
    );
  }

  private async beginNativeDrag(
    generation: number,
    token: number,
    position?: BubblePoint,
    size?: BubbleSize,
  ): Promise<void> {
    try {
      await this.nativeSessionReady;
      if (!this.isActiveDrag(generation) || token !== this.snapToken) return;
      const started = await invoke<boolean>(
        "start_bubble_drag",
        {
          session: this.nativeSession,
          token,
          x: position?.x,
          y: position?.y,
          width: size?.width,
          height: size?.height,
        },
      );
      if (!started && this.isActiveDrag(generation)) {
        throw t("bubble.error.startDrag");
      }
    } catch (reason) {
      if (!this.isActiveDrag(generation)) return;
      const restorePeek = this.dragStartedFromPeek;
      this.primaryPointerDown = false;
      this.nativeDragging = false;
      this.pendingClick = false;
      this.state = "visible";
      this.applyVisualState();
      this.reportError(reason, t("bubble.error.startDrag"));
      if (restorePeek && this.snapEnabled && this.docked) {
        void this.transitionToPeek(this.snapToken)
          .catch((error) => this.reportError(error, t("bubble.error.hide")));
      } else {
        this.scheduleIdle();
      }
    }
  }

  private onPointerUp(event: PointerEvent): void {
    if (event.button !== 0) return;
    if (this.pointerStartScreen
      && Math.hypot(event.screenX - this.pointerStartScreen.x, event.screenY - this.pointerStartScreen.y)
        >= BUBBLE_POINTER_DRAG_THRESHOLD_PX) {
      this.dragMoved = true;
      this.dragged = true;
    }
    this.pointerStartScreen = null;
    this.primaryPointerDown = false;
    this.nativeReleaseConfirmed = true;
    if (this.nativeDragging) this.scheduleDragFinish(BUBBLE_DRAG_CLICK_SETTLE_MS);
    else this.scheduleIdle();
    window.setTimeout(() => { this.dragged = false; }, 0);
  }

  private onPointerCancel(): void {
    this.pendingClick = false;
    this.pointerStartScreen = null;
    this.nativeReleaseConfirmed = false;
    if (this.nativeDragging) this.scheduleDragFinish(24);
    else {
      this.primaryPointerDown = false;
      this.scheduleIdle();
    }
  }

  private onWindowMoved(position: PhysicalPosition): void {
    const programmaticMove = this.consumeProgrammaticMove(position);
    if (programmaticMove) {
      if (programmaticMove.token !== undefined && programmaticMove.token !== this.snapToken) return;
      this.lastObservedPosition = { x: position.x, y: position.y };
      this.rebaseUnmovedGesture(position);
      return;
    }
    this.lastObservedPosition = { x: position.x, y: position.y };
    if (!this.nativeDragging) return;
    const now = performance.now();
    if (!this.dragStartPosition) this.dragStartPosition = { x: position.x, y: position.y };
    if (Math.hypot(position.x - this.dragStartPosition.x, position.y - this.dragStartPosition.y) >= 1) {
      this.dragMoved = true;
      this.dragged = true;
    }
    this.movementSamples.push({ x: position.x, y: position.y, time: now });
    this.movementSamples = this.movementSamples.filter((sample) => now - sample.time <= BUBBLE_SAMPLE_WINDOW_MS);
    this.scheduleDragFinish(this.nativeReleaseConfirmed ? 24 : BUBBLE_DRAG_FALLBACK_MS);
  }

  private scheduleDragFinish(delay: number): void {
    window.clearTimeout(this.dragSettleTimer);
    this.dragSettleTimer = window.setTimeout(() => {
      void this.finishNativeDrag();
    }, delay);
  }

  private async finishNativeDrag(): Promise<void> {
    if (!this.nativeDragging) return;
    if (this.finishInProgress) {
      this.scheduleDragFinish(BUBBLE_DRAG_RELEASE_POLL_MS);
      return;
    }
    this.finishInProgress = true;
    const generation = this.dragGeneration;
    const dragStartCompletion = this.dragStartCompletion;
    try {
      if (!this.nativeReleaseConfirmed) {
        try {
          const pressed = await invoke<boolean>("is_primary_mouse_button_pressed");
          if (!this.isActiveDrag(generation)) return;
          if (pressed) {
            this.scheduleDragFinish(BUBBLE_DRAG_RELEASE_POLL_MS);
            return;
          }
        } catch (reason) {
          if (!this.isActiveDrag(generation)) return;
          this.reportError(reason, t("bubble.error.confirmDragEnd"));
        }
        this.primaryPointerDown = false;
        this.nativeReleaseConfirmed = true;
      }
      await dragStartCompletion;
      if (!this.isActiveDrag(generation)) return;
      const position = await this.afterNativeQueue(() => currentWindow.outerPosition());
      if (!this.isActiveDrag(generation)) return;
      this.lastObservedPosition = { x: position.x, y: position.y };
      if (!this.dragStartPosition) {
        this.dragStartPosition = { x: position.x, y: position.y };
      } else if (Math.hypot(position.x - this.dragStartPosition.x, position.y - this.dragStartPosition.y) >= 1) {
        this.dragMoved = true;
        this.dragged = true;
      }
      this.movementSamples.push({ x: position.x, y: position.y, time: performance.now() });
      if (!this.dragMoved) {
        this.finishNativeGestureWithoutMovement();
        return;
      }
      const latest = this.movementSamples[this.movementSamples.length - 1] ?? this.dragStartPosition;
      const releasePosition = latest ? { x: latest.x, y: latest.y } : undefined;
      const velocity = bubbleReleaseVelocity(this.movementSamples);
      this.nativeDragging = false;
      this.dragMoved = false;
      this.pendingClick = false;
      this.dragStartedFromPeek = false;
      this.dragStartPosition = null;
      this.startDock(velocity, true, "peek-now", releasePosition);
    } finally {
      this.finishInProgress = false;
    }
  }

  private isActiveDrag(generation: number): boolean {
    return this.nativeDragging && this.dragGeneration === generation;
  }

  private finishNativeGestureWithoutMovement(): void {
    const shouldOpenPanel = this.pendingClick;
    const restorePeek = this.dragStartedFromPeek && !shouldOpenPanel;
    this.pendingClick = false;
    this.nativeDragging = false;
    this.nativeReleaseConfirmed = true;
    this.dragMoved = false;
    this.pointerStartScreen = null;
    this.dragStartPosition = null;
    this.movementSamples = [];
    this.state = "visible";
    this.dragStartedFromPeek = false;
    this.applyVisualState();
    if (restorePeek && this.snapEnabled && this.docked) {
      void this.transitionToPeek(this.snapToken)
        .catch((reason) => this.reportError(reason, t("bubble.error.hide")));
    } else if (!this.maybeDockAfterPanelClose()) {
      this.scheduleIdle();
    }
    if (shouldOpenPanel) void showPanel("details", true);
  }

  private rebaseUnmovedGesture(position: BubblePoint): void {
    if (!this.nativeDragging || this.dragMoved) return;
    this.dragStartPosition = { x: position.x, y: position.y };
    this.movementSamples = [{ x: position.x, y: position.y, time: performance.now() }];
  }

  private startDock(
    velocity: BubblePoint,
    animate: boolean,
    followup: BubbleDockFollowup = "schedule-idle",
    releasePosition?: BubblePoint,
  ): void {
    let tracked: Promise<void>;
    tracked = this.dockCurrentPosition(velocity, animate, followup, releasePosition).catch((reason) => {
      this.state = "visible";
      this.applyVisualState();
      this.reportError(reason, t("bubble.error.snap"));
      this.scheduleIdle();
    }).finally(() => {
      if (this.snapCompletion === tracked) this.snapCompletion = null;
    });
    this.snapCompletion = tracked;
  }

  private async dockCurrentPosition(
    velocity: BubblePoint,
    animate: boolean,
    followup: BubbleDockFollowup = "schedule-idle",
    releasePosition?: BubblePoint,
  ): Promise<void> {
    const token = ++this.snapToken;
    const snapEnabled = this.snapEnabled;
    const [windowPosition, size, monitors] = await Promise.all([
      this.afterNativeQueue(() => currentWindow.outerPosition()),
      this.afterNativeQueue(() => currentWindow.outerSize()),
      availableMonitors(),
    ]);
    if (token !== this.snapToken || snapEnabled !== this.snapEnabled) return;
    const position = releasePosition ?? windowPosition;
    this.lastObservedPosition = { x: position.x, y: position.y };
    const geometries = monitors.map((monitor) => ({
      x: monitor.workArea.position.x,
      y: monitor.workArea.position.y,
      width: monitor.workArea.size.width,
      height: monitor.workArea.size.height,
      scaleFactor: monitor.scaleFactor,
    }));
    const monitor = selectBubbleMonitor(geometries, position, releasePosition ? this.fullSize ?? size : size);
    if (!monitor) return;
    const target = snapEnabled
      ? calculateBubbleDockTarget(position, velocity, monitor)
      : calculateBubbleFreeTarget(position, monitor);
    const shouldAnimate = animate && snapEnabled;
    this.state = shouldAnimate ? "snapping" : "visible";
    this.applyVisualState();
    await this.resizeWindow(target.size, token);
    if (token !== this.snapToken || snapEnabled !== this.snapEnabled) return;
    const completed = shouldAnimate
      ? await this.animateTo(target.position, velocity, token, releasePosition)
      : await this.moveWindow(target.position, token).then(() => token === this.snapToken);
    if (!completed) return;
    if (token !== this.snapToken || snapEnabled !== this.snapEnabled) return;
    this.side = target.side;
    this.monitor = monitor;
    this.fullSize = target.size;
    this.anchor = target.position;
    this.docked = snapEnabled;
    if (snapEnabled) this.dockAfterPanelClose = false;
    this.state = "visible";
    this.applyVisualState();
    const anchorSave = this.saveAnchor(target.position);
    if (snapEnabled && followup === "peek-now") await this.enterPeekAfterDock(token);
    await anchorSave;
    if (token === this.snapToken && snapEnabled === this.snapEnabled && followup === "schedule-idle") {
      this.scheduleIdle();
    }
  }

  private async animateTo(
    target: BubblePoint,
    releaseVelocity: BubblePoint,
    token: number,
    releasePosition?: BubblePoint,
  ): Promise<boolean> {
    const start = releasePosition ?? await this.afterNativeQueue(() => currentWindow.outerPosition());
    if (token !== this.snapToken) return false;
    if (this.reducedMotion) {
      await this.moveWindow(target, token);
      return token === this.snapToken;
    }
    let x = start.x;
    let y = start.y;
    let velocityX = releaseVelocity.x;
    let velocityY = releaseVelocity.y;
    let previous = performance.now();
    const started = previous;
    const response = 0.38;
    const omega = (2 * Math.PI) / response;
    const stiffness = omega * omega;
    const damping = 2 * omega;
    while (token === this.snapToken) {
      const now = await new Promise<number>((resolve) => requestAnimationFrame(resolve));
      if (token !== this.snapToken) return false;
      const elapsed = now - started;
      const delta = Math.min((now - previous) / 1_000, 0.032);
      previous = now;
      velocityX += (-stiffness * (x - target.x) - damping * velocityX) * delta;
      velocityY += (-stiffness * (y - target.y) - damping * velocityY) * delta;
      x += velocityX * delta;
      y += velocityY * delta;
      await this.moveWindow({ x: Math.round(x), y: Math.round(y) }, token);
      if (token !== this.snapToken) return false;
      const settled = Math.hypot(x - target.x, y - target.y) < 0.75
        && Math.hypot(velocityX, velocityY) < 5;
      if (settled || elapsed > 720) break;
    }
    if (token !== this.snapToken) return false;
    await this.moveWindow(target, token);
    return token === this.snapToken;
  }

  private cancelSnap(): void {
    this.snapToken += 1;
  }

  private clearIdleTimer(): void {
    window.clearTimeout(this.idleTimer);
    this.idleTimer = undefined;
  }

  private scheduleIdle(): void {
    this.clearIdleTimer();
    if (!this.snapEnabled || !this.docked) return;
    if (!this.initialized || this.busy || this.panelVisible || this.hovering || this.focused || this.primaryPointerDown || this.nativeDragging || this.state !== "visible") return;
    this.idleTimer = window.setTimeout(() => {
      void this.enterPeek().catch((reason) => this.reportError(reason, t("bubble.error.hide")));
    }, BUBBLE_IDLE_DELAY_MS);
  }

  private async enterPeek(): Promise<void> {
    if (!this.snapEnabled || !this.docked) return;
    if (this.busy || this.panelVisible || this.hovering || this.focused || this.primaryPointerDown || this.nativeDragging || this.state !== "visible") return;
    await this.initialization;
    if (this.frameTransition) await this.frameTransition;
    await this.dockCurrentPosition({ x: 0, y: 0 }, false, "none");
    if (!this.snapEnabled || !this.docked || this.busy || this.panelVisible || this.hovering || this.focused || this.primaryPointerDown || this.nativeDragging || this.state !== "visible") return;
    await this.transitionToPeek(this.snapToken);
  }

  private async enterPeekAfterDock(token: number): Promise<void> {
    if (token !== this.snapToken || !this.snapEnabled || !this.docked) return;
    if (this.panelVisible || this.primaryPointerDown || this.nativeDragging || this.state !== "visible") return;
    await this.transitionToPeek(token);
  }

  private async transitionToPeek(token: number): Promise<void> {
    if (token !== this.snapToken) return;
    if (!this.anchor || !this.fullSize || !this.monitor) return;
    this.clearIdleTimer();
    this.state = "peek";
    this.applyVisualState();
    const frame = calculateBubblePeekFrame(this.anchor, this.fullSize, this.side, this.monitor.scaleFactor);
    await this.runFrameTransition(frame.position, frame.size, token);
    if (token !== this.snapToken && this.state === "peek") {
      this.state = "visible";
      this.applyVisualState();
    }
  }

  private async reveal(): Promise<void> {
    this.clearIdleTimer();
    await this.initialization;
    if (this.state !== "peek") {
      if (this.frameTransition) await this.frameTransition;
      if (this.state !== "dragging" && this.state !== "snapping") this.state = "visible";
      this.applyVisualState();
      return;
    }
    if (!this.anchor || !this.fullSize) return;
    const token = this.snapToken;
    this.state = "visible";
    this.applyVisualState();
    await this.runFrameTransition(this.anchor, this.fullSize, token);
    if (token !== this.snapToken) return;
    await this.dockCurrentPosition({ x: 0, y: 0 }, false, "none");
  }

  private applyVisualState(): void {
    this.shell.dataset.state = this.state === "peek" ? "idle" : this.state;
    this.shell.dataset.side = this.side;
  }

  private resizeWindow(size: BubbleSize, token?: number): Promise<void> {
    const operationToken = token ?? this.snapToken;
    return this.queueNative(async () => {
      await this.nativeSessionReady;
      if (operationToken !== this.snapToken) return;
      const accepted = await invokeWithTimeout<boolean>(
        "set_bubble_window_frame",
        { session: this.nativeSession, token: operationToken, width: size.width, height: size.height },
        ACTION_TIMEOUT_MS,
        t("bubble.action.updateFrame"),
      );
      this.requireAcceptedNativeOperation(accepted, operationToken);
    });
  }

  private moveWindow(position: BubblePoint, token?: number): Promise<void> {
    const operationToken = token ?? this.snapToken;
    const rounded = { x: Math.round(position.x), y: Math.round(position.y) };
    return this.queueNative(async () => {
      await this.nativeSessionReady;
      if (operationToken !== this.snapToken) return;
      this.rememberProgrammaticMove(rounded, operationToken);
      const accepted = await invokeWithTimeout<boolean>(
        "set_bubble_window_frame",
        { session: this.nativeSession, token: operationToken, x: rounded.x, y: rounded.y },
        ACTION_TIMEOUT_MS,
        t("bubble.action.updateFrame"),
      );
      this.requireAcceptedNativeOperation(accepted, operationToken);
    });
  }

  private setWindowFrame(position: BubblePoint, size: BubbleSize, token?: number): Promise<void> {
    const operationToken = token ?? this.snapToken;
    const rounded = { x: Math.round(position.x), y: Math.round(position.y) };
    return this.queueNative(async () => {
      await this.nativeSessionReady;
      if (operationToken !== this.snapToken) return;
      this.rememberProgrammaticMove(rounded, operationToken);
      const accepted = await invokeWithTimeout<boolean>(
        "set_bubble_window_frame",
        {
          session: this.nativeSession,
          token: operationToken,
          x: rounded.x,
          y: rounded.y,
          width: size.width,
          height: size.height,
        },
        ACTION_TIMEOUT_MS,
        t("bubble.action.updateFrame"),
      );
      this.requireAcceptedNativeOperation(accepted, operationToken);
    });
  }

  private async beginNativeWindowSession(): Promise<void> {
    this.nativeSession = await invokeWithTimeout<number>(
      "begin_bubble_window_session",
      undefined,
      ACTION_TIMEOUT_MS,
      t("bubble.action.initializeWindow"),
    );
  }

  private requireAcceptedNativeOperation(accepted: boolean, operationToken: number): void {
    if (!accepted && operationToken === this.snapToken) throw t("bubble.error.updateFrame");
  }

  private rememberProgrammaticMove(position: BubblePoint, token?: number, force = false): void {
    const now = performance.now();
    this.pendingProgrammaticMoves = this.pendingProgrammaticMoves
      .filter((candidate) => this.nativeDragging || candidate.expiresAt > now)
      .slice(-63);
    if (!force && this.lastObservedPosition
      && this.lastObservedPosition.x === position.x
      && this.lastObservedPosition.y === position.y) return;
    this.pendingProgrammaticMoves.push({ ...position, token, expiresAt: now + BUBBLE_PROGRAMMATIC_MOVE_TTL_MS });
  }

  private consumeProgrammaticMove(position: BubblePoint): PendingProgrammaticMove | null {
    const now = performance.now();
    this.pendingProgrammaticMoves = this.pendingProgrammaticMoves
      .filter((candidate) => this.nativeDragging || candidate.expiresAt > now);
    const index = this.pendingProgrammaticMoves.findIndex(
      (candidate) => candidate.x === position.x && candidate.y === position.y,
    );
    if (index < 0) return null;
    return this.pendingProgrammaticMoves.splice(index, 1)[0] ?? null;
  }

  private async runFrameTransition(position: BubblePoint, size: BubbleSize, token?: number): Promise<void> {
    const transition = this.setWindowFrame(position, size, token);
    this.frameTransition = transition;
    try {
      await transition;
    } finally {
      if (this.frameTransition === transition) this.frameTransition = null;
    }
  }

  private queueNative(operation: () => Promise<unknown>): Promise<void> {
    const task = this.nativeQueue.then(operation, operation).then(() => undefined);
    this.nativeQueue = task.catch(() => undefined);
    return task;
  }

  private async afterNativeQueue<T>(operation: () => Promise<T>): Promise<T> {
    await this.nativeQueue;
    return operation();
  }

  private async saveAnchor(position: BubblePoint): Promise<void> {
    const task = this.positionSaveQueue.then(() => invokeWithTimeout<AppSettings>(
      "save_window_position",
      { x: Math.round(position.x), y: Math.round(position.y) },
      ACTION_TIMEOUT_MS,
      t("bubble.action.savePosition"),
    ));
    this.positionSaveQueue = task.catch(() => undefined);
    try {
      await task;
    } catch (reason) {
      this.reportError(reason, t("bubble.error.savePosition"));
    }
  }

  private reportError(reason: unknown, fallback: string): void {
    showToast(friendlyError(reason, fallback), "error", 4_500);
  }
}

let bubbleController: BubbleWindowController | null = null;

function renderBubble(): void {
  const snapshot = payload?.snapshot;
  const mode = payload?.settings.bubblePercentMode ?? "remaining";
  const snapEnabled = payload?.settings.bubbleSnapEnabled ?? false;
  const order = bubbleVisibleProviderOrder(payload?.settings);
  const rows = snapshot
    ? order.map((provider) => bubbleRow(bubbleProviderLabel(provider, payload?.settings), snapshot[provider], mode, tokenGains[provider])).join("")
    : order.map((provider) => `<div class="bubble-row ${provider} loading-row" data-provider-accent="${provider}" style="${providerColorStyle(provider, payload?.settings)}"><span class="provider-mark">${escapeHtml(bubbleProviderLabel(provider, payload?.settings))}</span><strong>··</strong></div>`).join("");
  const idleValues = snapshot
    ? order.map((provider) => idleBubbleValue(snapshot[provider], mode)).join("")
    : order.map((provider) => `<span class="bubble-idle-value ${provider}" data-provider-accent="${provider}" style="${providerColorStyle(provider, payload?.settings)}">··</span>`).join("");
  if (!bubbleController) {
    app.innerHTML = `<div class="bubble-shell" id="open-details" data-state="visible" data-side="right" role="button" tabindex="0" aria-haspopup="dialog" aria-expanded="false">
        <div class="bubble-glow" aria-hidden="true"></div>
        <div class="bubble-center provider-count-${order.length}" aria-hidden="true"></div>
        <span class="bubble-idle-usage provider-count-${order.length}" aria-hidden="true"></span>
      </div>`;
    bubbleController = new BubbleWindowController(snapEnabled);
  }
  const busy = !snapshot || snapshot.refreshing || Object.keys(tokenGains).length > 0;
  bubbleController.update(rows, idleValues, order, mode, busy, snapEnabled);
}

function quotaRows(provider: ProviderSnapshot): string {
  const message = localizeProviderMessage(provider.message, provider.provider, provider.status);
  if (!provider.quotas.length) return `<p class="empty">${escapeHtml(message ?? t("quota.noData"))}</p>`;
  return provider.quotas.map((quota) => `<div class="quota">
    <div class="quota-head"><span>${escapeHtml(localizeQuotaLabel(quota.label))}</span><strong>${t("quota.remaining", { value: Math.round(quota.remainingPercent) })}</strong></div>
    <div class="track"><i style="width:${Math.max(0, Math.min(100, quota.remainingPercent))}%"></i></div>
    <div class="quota-meta"><span>${t("quota.used", { value: Math.round(quota.usedPercent) })}</span><span>${quota.resetsAt ? t("quota.resetsAt", { date: dateTime(quota.resetsAt) }) : ""}</span></div>
  </div>`).join("");
}

function isCursorUltraUsage(provider: ProviderSnapshot): boolean {
  if (provider.provider !== "cursor") return false;
  const hasUltraPool = provider.quotas.some((quota) => quota.kind !== undefined && CURSOR_ULTRA_QUOTA_KINDS.includes(quota.kind));
  return hasUltraPool || provider.plan?.toLowerCase() === "ultra";
}

function cursorUltraQuotaBlock(provider: ProviderSnapshot, kind: QuotaKind): string {
  const quota = provider.quotas.find((candidate) => candidate.kind === kind);
  const meta = CURSOR_ULTRA_QUOTA_META[kind];
  const hint = t(meta.hintKey);
  if (!quota) {
    return `<div class="cursor-ultra-quota unavailable" data-quota-kind="${kind}">
      <div class="quota-head"><span>${meta.label}</span><strong>${t("common.waitingForData")}</strong></div>
      <div class="track"><i style="width:0%"></i></div>
      <div class="quota-meta"><span>${hint}</span><span>—</span></div>
    </div>`;
  }
  return `<div class="cursor-ultra-quota" data-quota-kind="${kind}">
    <div class="quota-head"><span>${escapeHtml(localizeQuotaLabel(quota.label || meta.label))}</span><strong>${t("quota.remaining", { value: Math.round(quota.remainingPercent) })}</strong></div>
    <div class="track"><i style="width:${Math.max(0, Math.min(100, quota.remainingPercent))}%"></i></div>
    <div class="quota-meta"><span>${t("quota.used", { value: Math.round(quota.usedPercent) })}</span><span>${quota.resetsAt ? t("quota.resetsAt", { date: dateTime(quota.resetsAt) }) : hint}</span></div>
  </div>`;
}

function cursorCostBlock(label: string, used: number | undefined, limit: number, extraClass = ""): string {
  const remaining = used === undefined ? undefined : Math.max(0, limit - used);
  const remainingPercent = remaining === undefined ? 0 : Math.max(0, Math.min(100, remaining * 100 / limit));
  return `<div class="cursor-cost-block ${extraClass}">
    <div class="quota-head"><span>${label}</span><strong>${remaining === undefined ? t("common.waitingForData") : t("quota.moneyRemaining", { value: money(remaining) })}</strong></div>
    <div class="track"><i style="width:${remainingPercent}%"></i></div>
    <div class="quota-meta"><span>${t("quota.moneyUsed", { value: money(used) })}</span><span>${t("quota.limit", { value: money(limit) })}</span></div>
  </div>`;
}

function cursorUltraOnDemandBlock(provider: ProviderSnapshot): string {
  const cost = provider.cost;
  if (cost?.onDemandEnabled === false) {
    return `<div class="cursor-ultra-on-demand disabled">
      <div class="quota-head"><span>${t("quota.onDemand")}</span><strong>${t("common.disabled")}</strong></div>
      <div class="quota-meta"><span>${t("quota.onDemandDisabledHint")}</span><span>—</span></div>
    </div>`;
  }
  if (cost?.onDemandEnabled === true && cost.onDemandLimitCents !== undefined && cost.onDemandLimitCents > 0) {
    return cursorCostBlock("On-Demand", cost.onDemandUsedCents, cost.onDemandLimitCents, "cursor-ultra-on-demand enabled");
  }
  if (cost?.onDemandEnabled === true) {
    return `<div class="cursor-ultra-on-demand enabled">
      <div class="quota-head"><span>${t("quota.onDemand")}</span><strong>${t("common.enabled")}</strong></div>
      <div class="quota-meta"><span>${t("quota.moneyUsed", { value: money(cost.onDemandUsedCents) })}</span><span>${t("quota.limitWaiting")}</span></div>
    </div>`;
  }
  return `<div class="cursor-ultra-on-demand unknown">
    <div class="quota-head"><span>${t("quota.onDemand")}</span><strong>${t("common.waitingForData")}</strong></div>
    <div class="quota-meta"><span>${t("quota.onDemandUnavailableHint")}</span><span>—</span></div>
  </div>`;
}

function cursorUltraBlocks(provider: ProviderSnapshot): string {
  return `<div class="cursor-ultra-blocks">
    ${CURSOR_ULTRA_QUOTA_KINDS.map((kind) => cursorUltraQuotaBlock(provider, kind)).join("")}
    ${cursorUltraOnDemandBlock(provider)}
    ${provider.cost?.periodEnd ? `<div class="cursor-reset"><span>${t("quota.reset")}</span><strong>${dateTime(provider.cost.periodEnd)}</strong></div>` : ""}
  </div>`;
}

function cursorCostBlocks(provider: ProviderSnapshot): string {
  const cost = provider.cost;
  const includedLimit = cursorIncludedLimit(provider);
  const onDemandLimit = cost?.onDemandLimitCents ?? CURSOR_ON_DEMAND_FALLBACK_CENTS;
  return `<div class="cursor-costs">
    ${cursorCostBlock(t("quota.subscription", { value: money(includedLimit) }), cost?.includedUsedCents, includedLimit)}
    ${cursorCostBlock(`On-Demand · ${money(onDemandLimit)}`, cost?.onDemandUsedCents, onDemandLimit)}
    ${cost?.periodEnd ? `<div class="cursor-reset"><span>${t("quota.reset")}</span><strong>${dateTime(cost.periodEnd)}</strong></div>` : ""}
  </div>`;
}
function providerLoading(name: string): string {
  return `<div class="provider-loading" role="status">
    <i aria-hidden="true"></i>
    <div><strong>${t("provider.connectingTitle", { provider: escapeHtml(name) })}</strong><small>${t("provider.connectingDescription")}</small></div>
  </div>`;
}
function cursorLoginPrompt(cursorCompat: boolean): string {
  return `<div class="cursor-login-prompt">
    <i aria-hidden="true">↗</i>
    <div><strong>${cursorCompat ? t("cursor.loginExpired") : t("cursor.loginToContinue")}</strong><small>${cursorCompat ? t("cursor.loginExpiredHint") : t("cursor.loginAuthorizationHint")}</small></div>
  </div>`;
}
function providerCard(name: string, provider: ProviderSnapshot): string {
  const token = provider.provider === "codex" || provider.provider === "claude" ? provider.tokens : undefined;
  const cost = provider.cost;
  const claudeApiUsage = provider.provider === "claude" && cost?.todayUsedCents !== undefined;
  const todayTokenLabel = claudeApiUsage ? t("provider.apiTodayTokensUtc") : t("provider.localTodayTokens");
  const lifetimeTokenLabel = provider.provider === "claude" ? t("provider.localLifetimeTokens") : t("provider.officialLifetimeTokens");
  const cursorCompat = provider.provider === "cursor" && Boolean(payload?.settings.cursorCompatEnabled);
  const cursorNeedsLogin = provider.provider === "cursor" && provider.status === "not_logged_in";
  const cursorUsageAvailable = cursorCompat && provider.status === "available";
  const cursorCanEnableUsage = provider.provider === "cursor" && (provider.status === "available" || provider.status === "desktop_installed");
  const cursorUltra = cursorUsageAvailable && isCursorUltraUsage(provider);
  const loading = Boolean(payload?.snapshot.refreshing && !provider.quotas.length && !provider.cost && !provider.tokens);
  const usage = loading ? providerLoading(name) : cursorNeedsLogin ? cursorLoginPrompt(cursorCompat) : cursorUltra ? cursorUltraBlocks(provider) : cursorUsageAvailable ? cursorCostBlocks(provider) : quotaRows(provider);
  const localizedMessage = localizeProviderMessage(provider.message, provider.provider, provider.status);
  const displayedStatus = loading ? t("status.connecting") : provider.stale ? t("status.stale") : t(STATUS_TEXT_KEYS[provider.status]);
  const statusTone = loading ? "pending" : provider.stale ? "stale" : provider.status === "available" ? "available" : provider.status === "desktop_installed" ? "pending" : "unavailable";
  return `<section id="provider-card-${provider.provider}" class="provider-card ${provider.stale ? "is-stale" : ""}" data-provider-card="${provider.provider}" data-provider-accent="${provider.provider}" style="${providerColorStyle(provider.provider, payload?.settings)}" tabindex="-1" aria-labelledby="provider-title-${provider.provider}">
    <header><div><span class="provider-dot ${provider.provider}" aria-hidden="true"></span><strong id="provider-title-${provider.provider}">${name}</strong></div><span class="status ${statusTone}">${displayedStatus}</span></header>
    <div class="plan">${escapeHtml(planName(provider.plan))} · ${dateTime(provider.capturedAt)}</div>
    ${usage}
    ${cursorNeedsLogin && !loading ? `<button class="compat-cta cursor-login-cta" id="login-cursor">${cursorCompat ? t("cursor.loginInCursor") : t("cursor.login")}</button>` : cursorCanEnableUsage && !cursorCompat && !loading ? `<button class="compat-cta" id="enable-cursor-usage">${t("cursor.readExactUsage")}</button>` : ""}
    ${token ? `<div class="metrics"><div><span>${todayTokenLabel}</span><strong>${number(token.today)}</strong></div><div><span>${lifetimeTokenLabel}</span><strong>${number(token.lifetime)}</strong></div></div>` : ""}
    ${claudeApiUsage ? `<div class="metrics single-metric"><div><span>${t("provider.apiEstimatedCostUtc")}</span><strong>${cost.currency === "USD" ? money(cost.todayUsedCents) : `${escapeHtml(cost.currency)} ${((cost.todayUsedCents ?? 0) / 100).toFixed(2)}`}</strong></div></div>` : cost && provider.provider !== "cursor" ? `<div class="metrics"><div><span>Included</span><strong>${money(cost.includedUsedCents)} / ${money(cost.includedLimitCents)}</strong></div><div><span>On-Demand</span><strong>${money(cost.onDemandUsedCents)} / ${money(cost.onDemandLimitCents)}</strong></div></div>` : ""}
    ${localizedMessage && (provider.quotas.length || cursorCompat) ? `<p class="notice">${escapeHtml(localizedMessage)}</p>` : ""}
  </section>`;
}

function bubbleConfigItem(provider: ProviderName, settings: AppSettings): string {
  const name = PROVIDER_META[provider].name;
  const color = bubbleProviderColor(provider, settings);
  const visible = bubbleVisibleProviderOrder(settings).includes(provider);
  return `<div class="bubble-config-item ${visible ? "" : "is-hidden"}" data-bubble-provider="${provider}" data-bubble-color="${color}" data-bubble-visible="${visible}">
    <button type="button" class="drag-handle" title="${t("config.dragOrder")}" aria-label="${t("config.dragProviderOrder", { provider: name })}">⠿</button>
    <button type="button" class="bubble-config-dot color-trigger ${provider}" data-color-provider="${provider}" data-provider-accent="${provider}" style="--provider-color:${color}" aria-haspopup="dialog" aria-expanded="false" aria-controls="provider-color-palette" aria-label="${t("config.chooseCurrentColor", { provider: name, color })}" title="${t("config.chooseColor", { provider: name })}"></button>
    <a class="bubble-config-name provider-nav" href="#provider-card-${provider}" aria-label="${t("config.viewStats", { provider: name })}">${name}<span aria-hidden="true">›</span></a>
    <input type="text" maxlength="3" value="${escapeHtml(bubbleProviderLabel(provider, settings))}" aria-label="${t("config.bubbleLabel", { provider: name })}" spellcheck="false">
    <button type="button" class="visibility-toggle" role="switch" aria-checked="${visible}" aria-disabled="false" aria-label="${t("config.providerVisible", { provider: name })}" title="${visible ? t("config.hideProvider", { provider: name }) : t("config.showProvider", { provider: name })}"><span aria-hidden="true"></span></button>
  </div>`;
}

function renderBubbleConfig(settings: AppSettings): string {
  return `<section class="bubble-config" aria-labelledby="bubble-config-title">
    <div class="bubble-config-head"><strong id="bubble-config-title">${t("config.title")}</strong><small>${t("config.subtitle")}</small></div>
    <div class="bubble-config-list">${bubbleProviderOrder(settings).map((provider) => bubbleConfigItem(provider, settings)).join("")}</div>
  </section>`;
}

let activeColorPopover: HTMLElement | null = null;
let activeColorTrigger: HTMLButtonElement | null = null;

function dismissColorPaletteOnPointerDown(event: PointerEvent): void {
  if (activeColorPopover?.contains(event.target as Node) || activeColorTrigger?.contains(event.target as Node)) return;
  closeColorPalette(false);
}

function closeColorPalette(restoreFocus = false): boolean {
  if (!activeColorPopover && !activeColorTrigger) return false;
  const trigger = activeColorTrigger;
  document.removeEventListener("pointerdown", dismissColorPaletteOnPointerDown, true);
  activeColorPopover?.remove();
  trigger?.setAttribute("aria-expanded", "false");
  activeColorPopover = null;
  activeColorTrigger = null;
  if (restoreFocus && trigger?.isConnected) requestAnimationFrame(() => trigger.focus());
  return true;
}

function positionColorPalette(trigger: HTMLElement, popover: HTMLElement): void {
  const margin = 10;
  const gap = 7;
  const triggerRect = trigger.getBoundingClientRect();
  const popoverRect = popover.getBoundingClientRect();
  const maxLeft = Math.max(margin, window.innerWidth - popoverRect.width - margin);
  const left = Math.min(Math.max(margin, triggerRect.left - 18), maxLeft);
  let top = triggerRect.bottom + gap;
  if (top + popoverRect.height > window.innerHeight - margin) top = triggerRect.top - popoverRect.height - gap;
  top = Math.max(margin, Math.min(top, window.innerHeight - popoverRect.height - margin));
  popover.style.left = `${Math.round(left)}px`;
  popover.style.top = `${Math.round(top)}px`;
}

function applyProviderColor(provider: ProviderName, color: string): void {
  document.querySelectorAll<HTMLElement>(`[data-provider-accent="${provider}"]`)
    .forEach((element) => element.style.setProperty("--provider-color", color));
}

function openColorPalette(provider: ProviderName, item: HTMLElement, trigger: HTMLButtonElement, editor: HTMLElement): void {
  if (activeColorTrigger === trigger) {
    closeColorPalette(true);
    return;
  }
  closeColorPalette(false);
  const selectedColor = normalizeProviderColor(provider, item.dataset.bubbleColor);
  const popover = document.createElement("div");
  popover.id = "provider-color-palette";
  popover.className = "color-palette-popover";
  popover.dataset.colorPalette = provider;
  popover.setAttribute("role", "dialog");
  popover.setAttribute("aria-modal", "false");
  popover.setAttribute("aria-label", t("config.chooseColor", { provider: PROVIDER_META[provider].name }));
  popover.innerHTML = COLOR_PALETTE.map((row, rowIndex) => `<div class="color-palette-row" role="group" aria-label="${t("color.group", { tone: t(COLOR_TONE_KEYS[rowIndex]) })}">
    ${row.map((color, columnIndex) => `<button type="button" class="color-swatch ${color === selectedColor ? "selected" : ""}" data-color-value="${color}" style="--swatch-color:${color}" aria-label="${t("color.swatch", { tone: t(COLOR_TONE_KEYS[rowIndex]), hue: t(COLOR_HUE_KEYS[columnIndex]), color })}" aria-pressed="${color === selectedColor}" tabindex="${color === selectedColor ? "0" : "-1"}"></button>`).join("")}
  </div>`).join("");
  document.body.append(popover);
  activeColorPopover = popover;
  activeColorTrigger = trigger;
  trigger.setAttribute("aria-expanded", "true");
  positionColorPalette(trigger, popover);
  document.addEventListener("pointerdown", dismissColorPaletteOnPointerDown, true);

  const swatches = [...popover.querySelectorAll<HTMLButtonElement>("[data-color-value]")];
  swatches.forEach((swatch) => swatch.addEventListener("click", () => {
    const color = normalizeProviderColor(provider, swatch.dataset.colorValue);
    item.dataset.bubbleColor = color;
    trigger.style.setProperty("--provider-color", color);
    trigger.setAttribute("aria-label", t("config.chooseCurrentColor", { provider: PROVIDER_META[provider].name, color }));
    applyProviderColor(provider, color);
    closeColorPalette(true);
    saveBubbleDisplayConfig(editor);
  }));
  popover.addEventListener("keydown", (event) => {
    const target = event.target as HTMLButtonElement;
    const currentIndex = swatches.indexOf(target);
    if (currentIndex < 0) return;
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = currentIndex > 0 ? currentIndex - 1 : currentIndex;
    if (event.key === "ArrowRight") nextIndex = currentIndex < swatches.length - 1 ? currentIndex + 1 : currentIndex;
    if (event.key === "ArrowUp") nextIndex = currentIndex >= COLOR_HUE_KEYS.length ? currentIndex - COLOR_HUE_KEYS.length : currentIndex;
    if (event.key === "ArrowDown") nextIndex = currentIndex + COLOR_HUE_KEYS.length < swatches.length ? currentIndex + COLOR_HUE_KEYS.length : currentIndex;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = swatches.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    target.tabIndex = -1;
    swatches[nextIndex].tabIndex = 0;
    swatches[nextIndex].focus();
  });
  requestAnimationFrame(() => popover.querySelector<HTMLButtonElement>(".color-swatch.selected")?.focus());
}

let bubbleConfigSaveQueue: Promise<void> = Promise.resolve();
function saveBubbleDisplayConfig(editor: HTMLElement): void {
  if (!payload) return;
  const items = [...editor.querySelectorAll<HTMLElement>("[data-bubble-provider]")];
  const order = items.map((item) => item.dataset.bubbleProvider as ProviderName);
  const labelFor = (provider: ProviderName) => items
    .find((item) => item.dataset.bubbleProvider === provider)
    ?.querySelector<HTMLInputElement>("input")?.value.trim() ?? "";
  const cursorLabel = labelFor("cursor") || "C";
  const codexLabel = labelFor("codex") || "X";
  const claudeLabel = labelFor("claude") || "A";
  const colorFor = (provider: ProviderName) => normalizeProviderColor(
    provider,
    items.find((item) => item.dataset.bubbleProvider === provider)?.dataset.bubbleColor,
  );
  const cursorColor = colorFor("cursor");
  const codexColor = colorFor("codex");
  const claudeColor = colorFor("claude");
  const visibleProviders = items
    .filter((item) => item.dataset.bubbleVisible !== "false")
    .map((item) => item.dataset.bubbleProvider as ProviderName);
  payload.settings = {
    ...payload.settings,
    bubbleProviderOrder: order,
    bubbleVisibleProviders: visibleProviders,
    cursorBubbleLabel: cursorLabel,
    codexBubbleLabel: codexLabel,
    claudeBubbleLabel: claudeLabel,
    cursorBubbleColor: cursorColor,
    codexBubbleColor: codexColor,
    claudeBubbleColor: claudeColor,
  };
  bubbleConfigSaveQueue = bubbleConfigSaveQueue.then(async () => {
    try {
      const settings = await invokeWithTimeout<AppSettings>(
        "set_bubble_display_config",
        { order, visibleProviders, cursorLabel, codexLabel, claudeLabel, cursorColor, codexColor, claudeColor },
        ACTION_TIMEOUT_MS,
        t("config.save"),
      );
      if (payload) payload.settings = settings;
    } catch (reason) {
      showToast(friendlyError(reason, t("config.saveFailed")), "error", 4_500);
      void loadPayload();
    }
  });
}

function updateVisibilityControl(item: HTMLElement, toggle: HTMLButtonElement, visible: boolean): void {
  const provider = item.dataset.bubbleProvider as ProviderName;
  const name = PROVIDER_META[provider].name;
  item.dataset.bubbleVisible = String(visible);
  item.classList.toggle("is-hidden", !visible);
  toggle.setAttribute("aria-checked", String(visible));
  toggle.setAttribute("aria-label", t("config.providerVisible", { provider: name }));
  toggle.title = visible ? t("config.hideProvider", { provider: name }) : t("config.showProvider", { provider: name });
}

function syncVisibilityToggleAvailability(editor: HTMLElement): void {
  const items = [...editor.querySelectorAll<HTMLElement>(".bubble-config-item")];
  const visibleItems = items.filter((item) => item.dataset.bubbleVisible !== "false");
  items.forEach((item) => {
    const toggle = item.querySelector<HTMLButtonElement>(".visibility-toggle")!;
    const provider = item.dataset.bubbleProvider as ProviderName;
    const name = PROVIDER_META[provider].name;
    const visible = item.dataset.bubbleVisible !== "false";
    const locked = visibleItems.length === 1 && item === visibleItems[0];
    toggle.setAttribute("aria-disabled", String(locked));
    toggle.title = locked
      ? t("config.keepOne")
      : visible ? t("config.hideProvider", { provider: name }) : t("config.showProvider", { provider: name });
  });
}

function bindBubbleConfigEditor(): void {
  const editor = document.querySelector<HTMLElement>(".bubble-config-list");
  if (!editor) return;
  editor.querySelectorAll<HTMLElement>(".bubble-config-item").forEach((item) => {
    const handle = item.querySelector<HTMLButtonElement>(".drag-handle")!;
    const provider = item.dataset.bubbleProvider as ProviderName;
    const colorTrigger = item.querySelector<HTMLButtonElement>(".color-trigger")!;
    const visibilityToggle = item.querySelector<HTMLButtonElement>(".visibility-toggle")!;
    const providerNav = item.querySelector<HTMLAnchorElement>(".provider-nav")!;
    const navigateToProvider = () => providerCardNavigator.navigate(provider);
    providerNav.addEventListener("click", (event) => {
      event.preventDefault();
      if (editor.classList.contains("is-sorting")) return;
      navigateToProvider();
    });
    item.addEventListener("click", (event) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!shouldNavigateFromProviderRow(target, editor.classList.contains("is-sorting"))) return;
      navigateToProvider();
    });
    colorTrigger.addEventListener("click", () => openColorPalette(provider, item, colorTrigger, editor));
    visibilityToggle.addEventListener("click", () => {
      const visible = item.dataset.bubbleVisible !== "false";
      const visibleCount = editor.querySelectorAll('[data-bubble-visible="true"]').length;
      if (visible && visibleCount === 1) {
        showToast(t("config.keepOne"), "info", 2_200);
        return;
      }
      updateVisibilityControl(item, visibilityToggle, !visible);
      syncVisibilityToggleAvailability(editor);
      saveBubbleDisplayConfig(editor);
      showToast(visible
        ? t("config.providerHidden", { provider: PROVIDER_META[provider].name })
        : t("config.providerShown", { provider: PROVIDER_META[provider].name }), "info", 1_600);
    });
    let activePointer: number | null = null;
    let startY = 0;
    let lastY = 0;
    let dragTop = 0;
    let initialOrder = "";
    let placeholder: HTMLElement | null = null;
    const orderKey = () => [...editor.querySelectorAll<HTMLElement>(".bubble-config-item")]
      .map((candidate) => candidate.dataset.bubbleProvider)
      .join(",");
    const placePlaceholderAt = (clientY: number) => {
      if (!placeholder) return;
      const nextItem = [...editor.querySelectorAll<HTMLElement>(".bubble-config-item")]
        .filter((candidate) => candidate !== item)
        .find((candidate) => clientY < candidate.getBoundingClientRect().top + candidate.offsetHeight / 2);
      editor.insertBefore(placeholder, nextItem ?? null);
    };
    const updateDragPosition = (clientY: number) => {
      lastY = clientY;
      if (Math.abs(lastY - startY) < 3) return;
      item.style.transform = `translateY(${lastY - startY}px) scale(1.025)`;
      placePlaceholderAt(lastY);
    };
    const finishPointerSort = (event: PointerEvent) => {
      if (activePointer !== event.pointerId) return;
      lastY = event.clientY || lastY;
      updateDragPosition(lastY);
      window.removeEventListener("pointermove", trackPointerSort, true);
      window.removeEventListener("pointerup", finishPointerSort, true);
      window.removeEventListener("pointercancel", finishPointerSort, true);
      activePointer = null;
      const targetTop = placeholder?.getBoundingClientRect().top ?? dragTop;
      item.classList.add("dropping");
      item.style.transform = `translateY(${targetTop - dragTop}px) scale(1)`;
      window.setTimeout(() => {
        if (placeholder?.isConnected) editor.insertBefore(item, placeholder);
        placeholder?.remove();
        placeholder = null;
        item.classList.remove("dragging", "dropping");
        item.removeAttribute("aria-grabbed");
        item.removeAttribute("style");
        editor.classList.remove("is-sorting");
        if (editor.isConnected && orderKey() !== initialOrder) saveBubbleDisplayConfig(editor);
      }, 130);
    };
    const trackPointerSort = (event: PointerEvent) => {
      if (activePointer !== event.pointerId) return;
      lastY = event.clientY;
      if (Math.abs(lastY - startY) < 3) return;
      event.preventDefault();
      updateDragPosition(lastY);
    };
    handle.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      const rect = item.getBoundingClientRect();
      activePointer = event.pointerId;
      startY = event.clientY;
      lastY = event.clientY;
      dragTop = rect.top;
      initialOrder = orderKey();
      placeholder = document.createElement("div");
      placeholder.className = "bubble-config-placeholder";
      placeholder.setAttribute("aria-hidden", "true");
      editor.insertBefore(placeholder, item);
      item.style.position = "fixed";
      item.style.zIndex = "30";
      item.style.top = `${rect.top}px`;
      item.style.left = `${rect.left}px`;
      item.style.width = `${rect.width}px`;
      item.style.height = `${rect.height}px`;
      item.style.transform = "translateY(0) scale(1.025)";
      item.setAttribute("aria-grabbed", "true");
      item.classList.add("dragging");
      editor.classList.add("is-sorting");
      window.addEventListener("pointermove", trackPointerSort, true);
      window.addEventListener("pointerup", finishPointerSort, true);
      window.addEventListener("pointercancel", finishPointerSort, true);
      event.preventDefault();
    });
    handle.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
      event.preventDefault();
      const items = [...editor.querySelectorAll<HTMLElement>(".bubble-config-item")];
      const index = items.indexOf(item);
      const targetIndex = event.key === "ArrowUp" ? index - 1 : index + 1;
      const target = items[targetIndex];
      if (!target) return;
      if (event.key === "ArrowUp") editor.insertBefore(item, target);
      else editor.insertBefore(target, item);
      handle.focus();
      saveBubbleDisplayConfig(editor);
    });
    const input = item.querySelector<HTMLInputElement>("input")!;
    input.addEventListener("keydown", (event) => { if (event.key === "Enter") input.blur(); });
    input.addEventListener("change", () => saveBubbleDisplayConfig(editor));
  });
  syncVisibilityToggleAvailability(editor);
}

function renderDetails(): void {
  if (!payload) return;
  closeColorPalette(false);
  app.innerHTML = `<main class="panel details-panel ${payload.snapshot.refreshing ? "is-refreshing" : ""}">
    <div class="panel-title">
      <div class="panel-brand">${metraLogo()}<div><strong>Metra</strong><small>${t("app.usageSubtitle")}</small></div></div>
      ${payload.snapshot.refreshing ? `<div class="refresh-status" role="status"><i></i><span>${t("refresh.updating")}</span></div>` : ""}
      <button id="refresh" class="icon-btn" title="${payload.snapshot.refreshing ? t("refresh.refreshing") : t("refresh.now")}" aria-label="${payload.snapshot.refreshing ? t("refresh.refreshingUsage") : t("refresh.nowUsage")}"><span aria-hidden="true">↻</span></button>
    </div>
    ${renderBubbleConfig(payload.settings)}
    <div class="provider-list">${providerCard("Cursor", payload.snapshot.cursor)}${providerCard("Codex", payload.snapshot.codex)}${providerCard("Claude Code", payload.snapshot.claude)}</div>
    <footer>${payload.snapshot.refreshing
      ? t("refresh.everyMinutesActive", { minutes: payload.settings.refreshMinutes })
      : t("refresh.everyMinutes", { minutes: payload.settings.refreshMinutes })}</footer>
  </main>`;
  document.querySelector("#refresh")?.addEventListener("click", () => { void refreshWithFeedback(); });
  document.querySelector("#login-cursor")?.addEventListener("click", () => { void loginCursor(); });
  document.querySelector("#enable-cursor-usage")?.addEventListener("click", () => enableCursorUsage());
  bindBubbleConfigEditor();
}
async function loginCursor(): Promise<void> {
  if (cursorLoginPending) {
    await recheckCursorLogin();
    return;
  }
  const button = document.querySelector<HTMLButtonElement>("#login-cursor");
  cursorLoginPending = true;
  if (button) {
    button.disabled = true;
    button.setAttribute("aria-busy", "true");
    button.textContent = t("cursor.opening");
  }
  try {
    const started = await invokeWithTimeout<CursorLoginStart>("start_cursor_login", undefined, ACTION_TIMEOUT_MS, t("cursor.openLogin"));
    const message = started.method === "agent"
      ? started.alreadyRunning ? t("cursor.loginAlreadyRunning") : t("cursor.loginOpened")
      : t("cursor.settingsOpened");
    showToast(message, "success", 4_500);
  } catch (reason) {
    cursorLoginPending = false;
    if (button) {
      button.disabled = false;
      button.removeAttribute("aria-busy");
      button.textContent = t("cursor.retryLogin");
    }
    showToast(friendlyError(reason, t("cursor.openLoginFailed")), "error", 4_500);
  }
}

async function recheckCursorLogin(): Promise<void> {
  if (!cursorLoginPending || cursorLoginRecheckInFlight) return cursorLoginRecheckInFlight ?? undefined;
  const task = (async () => {
    try {
      showToast(t("cursor.checkingLogin"), "loading", 0);
      const updated = await invokeWithTimeout<AppPayload>("recheck_cursor_login", undefined, REFRESH_TIMEOUT_MS, t("cursor.checkLogin"));
      applyUsageUpdate(updated);
      const loggedIn = updated.snapshot.cursor.status === "available";
      cursorLoginPending = false;
      showToast(loggedIn ? t("cursor.loginSuccess") : t("cursor.loginNotDetected"), loggedIn ? "success" : "info", 4_500);
      if (!loggedIn) requestAnimationFrame(() => document.querySelector<HTMLButtonElement>("#login-cursor")?.focus());
    } catch (reason) {
      cursorLoginPending = false;
      showToast(friendlyError(reason, t("cursor.checkLoginFailed")), "error", 4_500);
    }
  })();
  cursorLoginRecheckInFlight = task.finally(() => { cursorLoginRecheckInFlight = null; });
  return cursorLoginRecheckInFlight;
}
function enableCursorUsage(): void {
  if (!payload || document.querySelector(".cursor-consent")) return;
  const consent = document.createElement("div");
  consent.className = "cursor-consent";
  consent.innerHTML = `<section role="dialog" aria-modal="true" aria-labelledby="consent-title">
    <span class="provider-dot cursor" style="${providerColorStyle("cursor", payload.settings)}" aria-hidden="true"></span>
    <h2 id="consent-title">${t("consent.cursorTitle")}</h2>
    <p>${t("consent.cursorReadOnly")}</p>
    <p>${t("consent.cursorMemoryOnly")}</p>
    <div class="consent-error" aria-live="polite"></div>
    <div class="consent-actions"><button id="consent-cancel">${t("common.cancel")}</button><button id="consent-confirm" class="primary">${t("consent.enable")}</button></div>
  </section>`;
  app.append(consent);
  const cancelButton = consent.querySelector<HTMLButtonElement>("#consent-cancel")!;
  const confirmButton = consent.querySelector<HTMLButtonElement>("#consent-confirm")!;
  const error = consent.querySelector<HTMLElement>(".consent-error")!;
  cancelButton.onclick = () => consent.remove();
  confirmButton.onclick = async () => {
    confirmButton.disabled = true;
    cancelButton.disabled = true;
    confirmButton.textContent = t("consent.enabling");
    error.textContent = t("consent.reading");
    try {
      payload!.settings = await invokeWithTimeout<AppSettings>("set_cursor_compat", { enabled: true }, ACTION_TIMEOUT_MS, t("consent.enableAction"));
      payload!.snapshot.refreshing = true;
      panelMode = "details";
      renderDetails();
      showToast(t("consent.enabledReading"), "success");
    } catch (reason) {
      error.textContent = t("consent.enableFailed", {
        reason: friendlyError(reason, t("consent.enableFailureReason")),
      });
      confirmButton.disabled = false;
      cancelButton.disabled = false;
      confirmButton.textContent = t("common.retry");
    }
  };
}

function renderMenu(): void {
  if (!payload) return;
  closeColorPalette(false);
  const s = payload.settings;
  app.innerHTML = `<main class="panel menu-panel">
    <div class="menu-brand">${metraLogo()}<div><strong>Metra</strong><small>${t("app.desktopBubbleSubtitle")}</small></div></div>
    <div class="menu-language-row">
      <span>${t("menu.language")}</span>
      <label class="language-select-control">
        <select data-ui-language aria-label="${t("menu.language")}">
          ${UI_LANGUAGE_OPTIONS.map(({ value, labelKey }) => `<option value="${value}" ${s.uiLanguage === value ? "selected" : ""}>${t(labelKey)}</option>`).join("")}
        </select>
        <i aria-hidden="true"></i>
      </label>
    </div>
    <div class="menu-label">${t("menu.refreshInterval")}</div>
    <div class="intervals">${[1, 5, 15, 30, 60].map((n) => `<button data-interval="${n}" class="${s.refreshMinutes === n ? "selected" : ""}">${n < 60 ? `${n}m` : "1h"}</button>`).join("")}</div>
    <div class="menu-label">${t("menu.bubblePercentage")}</div>
    <div class="percent-modes"><button data-percent-mode="used" class="${s.bubblePercentMode === "used" ? "selected" : ""}">${t("menu.used")}</button><button data-percent-mode="remaining" class="${s.bubblePercentMode === "remaining" ? "selected" : ""}">${t("menu.remaining")}</button></div>
    <button data-action="snap" role="switch" aria-checked="${s.bubbleSnapEnabled}"><span><b>${t("menu.autoSnap")}</b><small>${t("menu.autoSnapHint")}</small></span><i class="switch ${s.bubbleSnapEnabled ? "on" : ""}" aria-hidden="true"></i></button>
    <button data-action="autostart"><span>${t("menu.autostart")}</span><i class="switch ${s.autostart ? "on" : ""}"></i></button>
    <button data-action="compat"><span><b>${t("menu.cursorCompat")}</b><small>${t("menu.cursorCompatHint")}</small></span><i class="switch ${s.cursorCompatEnabled ? "on" : ""}"></i></button>
    <button data-action="rescan"><span>${t("menu.rescanCli")}</span><kbd>↻</kbd></button>
    <button data-action="quit" class="danger"><span>${t("menu.quit")}</span></button>
  </main>`;
  const languageSelect = app.querySelector<HTMLSelectElement>("[data-ui-language]");
  if (languageSelect) languageSelect.onchange = () => {
    void (async () => {
      const language = languageSelect.value as UiLanguage;
      if (language === payload!.settings.uiLanguage) return;
      languageSelect.disabled = true;
      showToast(t("menu.switchingLanguage"), "loading", 0);
      try {
        const settings = await invokeWithTimeout<AppSettings>(
          "set_ui_language",
          { language, locale: effectiveLocale(language) },
          ACTION_TIMEOUT_MS,
          t("menu.updateLanguageAction"),
        );
        payload!.settings = settings;
        applyLanguagePreference(settings.uiLanguage);
        renderMenu();
        showToast(t("menu.languageUpdated"), "success");
      } catch (reason) {
        languageSelect.disabled = false;
        languageSelect.value = payload!.settings.uiLanguage;
        showToast(friendlyError(reason, t("action.failed", { action: t("menu.updateLanguageAction") })), "error", 4_500);
      }
    })();
  };
  app.querySelectorAll<HTMLButtonElement>("[data-percent-mode]").forEach((button) => button.onclick = () => {
    void (async () => {
      const mode = button.dataset.percentMode as BubblePercentMode;
      if (mode === payload!.settings.bubblePercentMode) return;
      button.disabled = true;
      const label = mode === "used" ? t("menu.used") : t("menu.remaining");
      const settings = await runUiAction<AppSettings>(
        t("menu.switchingPercent", { mode: label }),
        t("menu.percentSwitched", { mode: label }),
        () => invokeWithTimeout<AppSettings>("set_bubble_percent_mode", { mode }, ACTION_TIMEOUT_MS, t("menu.switchPercentAction")),
      );
      if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
    })();
  });
  app.querySelectorAll<HTMLButtonElement>("[data-interval]").forEach((button) => button.onclick = () => {
    void (async () => {
      button.disabled = true;
      const settings = await runUiAction<AppSettings>(
        t("menu.updatingInterval"),
        t("menu.intervalUpdated"),
        () => invokeWithTimeout<AppSettings>("set_refresh_interval", { minutes: Number(button.dataset.interval) }, ACTION_TIMEOUT_MS, t("menu.updateIntervalAction")),
      );
      if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
    })();
  });
  app.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => button.onclick = () => {
    void (async () => {
      const action = button.dataset.action;
      if (action === "rescan") {
        await refreshWithFeedback(t("menu.rescan"), true);
        return;
      }
      button.disabled = true;
      if (action === "snap") {
        const enabled = !payload!.settings.bubbleSnapEnabled;
        const settings = await runUiAction<AppSettings>(
          enabled ? t("menu.enablingSnap") : t("menu.disablingSnap"),
          enabled ? t("menu.snapEnabled") : t("menu.snapDisabled"),
          () => invokeWithTimeout<AppSettings>("set_bubble_snap_enabled", { enabled }, ACTION_TIMEOUT_MS, t("menu.updateSnapAction")),
        );
        if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
        return;
      }
      if (action === "autostart") {
        const settings = await runUiAction<AppSettings>(
          t("menu.updatingAutostart"),
          t("menu.autostartUpdated"),
          () => invokeWithTimeout<AppSettings>("set_autostart", { enabled: !payload!.settings.autostart }, ACTION_TIMEOUT_MS, t("menu.updateAutostartAction")),
        );
        if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
        return;
      }
      if (action === "compat") {
        const enable = !payload!.settings.cursorCompatEnabled;
        if (enable && payload!.snapshot.cursor.status === "not_logged_in") {
          button.disabled = false;
          await loginCursor();
          return;
        }
        if (enable && payload!.snapshot.cursor.status !== "available" && payload!.snapshot.cursor.status !== "desktop_installed") {
          button.disabled = false;
          const cursor = payload!.snapshot.cursor;
          showToast(localizeProviderMessage(cursor.message, cursor.provider, cursor.status) ?? t("cursor.accountUnavailable"), "info", 4_500);
          return;
        }
        if (enable) { button.disabled = false; enableCursorUsage(); return; }
        const settings = await runUiAction<AppSettings>(
          t("menu.disablingCompat"),
          t("menu.compatDisabled"),
          () => invokeWithTimeout<AppSettings>("set_cursor_compat", { enabled: false }, ACTION_TIMEOUT_MS, t("menu.disableCompatAction")),
        );
        if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
        return;
      }
      if (action === "quit") {
        await runUiAction<void>(t("menu.quitting"), "", () => invokeWithTimeout<void>("quit_app", undefined, ACTION_TIMEOUT_MS, t("menu.quit")));
      }
    })();
  });
}

function renderPanel(): void {
  panelMode === "details" ? renderDetails() : renderMenu();
  app.dataset.panelDockSide = panelDockSide;
}
function render(): void { view === "bubble" ? renderBubble() : renderPanel(); }

async function showPanel(mode: "details" | "menu", toggle = false): Promise<void> {
  const requestId = ++panelRequestSequence;
  try {
    if (view === "bubble") await bubbleController?.prepareForPanel();
    if (requestId !== panelRequestSequence) return;
    await invokeWithTimeout<number>(
      "show_panel",
      { mode, toggle, requestId },
      PANEL_SHOW_TIMEOUT_MS,
      t("panel.show"),
    );
  } catch (reason) {
    if (requestId === panelRequestSequence) {
      showToast(friendlyError(reason, t("panel.openFailed")), "error", 4_500);
    }
  }
}

async function hidePanelWindow(): Promise<void> {
  await withTimeout(currentWindow.hide(), ACTION_TIMEOUT_MS, t("panel.hide"));
  await withTimeout(emit("panel-visibility-changed", { visible: false }), ACTION_TIMEOUT_MS, t("panel.syncVisibility"));
}

if (view !== "bubble") {
  void listen<{ mode: "details" | "menu"; dockSide?: BubbleDockSide }>("panel-mode", (event) => {
    panelMode = event.payload.mode;
    panelDockSide = event.payload.dockSide ?? panelDockSide;
    renderPanel();
  });
  void currentWindow.onFocusChanged(({ payload: focused }) => {
    if (focused) {
      if (cursorLoginPending) void recheckCursorLogin();
      return;
    }
    window.setTimeout(() => {
      void (async () => {
        try {
          const stillFocused = await withTimeout(currentWindow.isFocused(), ACTION_TIMEOUT_MS, t("panel.checkFocus"));
          if (!stillFocused) await hidePanelWindow();
        } catch (reason) {
          showToast(friendlyError(reason, t("panel.hideFailed")), "error", 4_500);
        }
      })();
    }, 120);
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      if (closeColorPalette(true)) {
        event.preventDefault();
        return;
      }
      void hidePanelWindow()
        .catch((reason) => showToast(friendlyError(reason, t("panel.hideFailed")), "error", 4_500));
    }
  });
}
void listen<{ visible: boolean }>("panel-visibility-changed", (event) => {
  if (view === "bubble") bubbleController?.setPanelVisible(event.payload.visible);
});
void listen<{ side?: BubbleDockSide }>("bubble-reveal-requested", () => {
  if (view === "bubble") void bubbleController?.prepareForPanel();
});
void listen<AppSettings>("settings-updated", (event) => {
  pendingSettings = event.payload;
  applyLanguagePreference(event.payload.uiLanguage);
  if (!payload) return;
  payload.settings = event.payload;
  pendingSettings = null;
  render();
});
void listen<AppPayload>("usage-updated", (event) => applyUsageUpdate(event.payload));
void listen<{ success: boolean; message: string }>("cursor-login-finished", (event) => {
  if (event.payload.success) return;
  cursorLoginPending = false;
  showToast(localizeProviderMessage(event.payload.message, "cursor", "not_logged_in") ?? t("cursor.loginNotDetected"), "error", 4_500);
});
void loadPayload();
