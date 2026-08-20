import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { availableMonitors, getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";
import {
  bubbleReleaseVelocity,
  calculateBubbleDockTarget,
  calculateBubblePeekFrame,
  selectBubbleMonitor,
  type BubbleDockSide,
  type BubbleMonitorGeometry,
  type BubbleMotionSample,
  type BubblePoint,
  type BubbleSize,
} from "./bubble-geometry";
import type { AppPayload, AppSettings, BubblePercentMode, ProviderName, ProviderSnapshot, ProviderStatus, QuotaKind } from "./types";
import { ProviderCardNavigator, shouldNavigateFromProviderRow } from "./provider-navigation";
import metraWaterUrl from "./metra-water.svg";
import "./styles.css";

const app = document.querySelector<HTMLDivElement>("#app")!;
const providerCardNavigator = new ProviderCardNavigator(document, {
  prefersReducedMotion: () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  schedule: (callback, delay) => window.setTimeout(callback, delay),
  cancel: (handle) => window.clearTimeout(handle as number),
});
const currentWindow = getCurrentWindow();
const view = new URLSearchParams(location.search).get("view") ?? "bubble";
let payload: AppPayload | null = null;
let panelMode: "details" | "menu" = "details";
let panelDockSide: BubbleDockSide = "right";
let panelRequestSequence = 0;
const MENU_PANEL_HEIGHT = 390;
const PANEL_GAP = 3;
const ACTION_TIMEOUT_MS = 8_000;
const PANEL_SHOW_TIMEOUT_MS = 1_000;
const REFRESH_TIMEOUT_MS = 22_000;
const BUBBLE_IDLE_DELAY_MS = 3_000;
const BUBBLE_DRAG_THRESHOLD = 4;
const BUBBLE_DRAG_SETTLE_MS = 180;
const BUBBLE_SAMPLE_WINDOW_MS = 140;

interface CursorLoginStart {
  method: "agent" | "editor";
  alreadyRunning: boolean;
}

function metraLogo(): string {
  return `<img class="logo-mark" src="${metraWaterUrl}" alt="" aria-hidden="true">`;
}

const statusText: Record<ProviderStatus, string> = {
  available: "可用", desktop_installed: "桌面版已安装", not_installed: "未安装", not_logged_in: "未登录",
  unsupported: "暂不可用", network_error: "网络错误", protocol_error: "协议不兼容",
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
const COLOR_HUE_NAMES = ["红", "橙", "黄", "青柠", "绿", "青", "天蓝", "蓝", "粉", "紫", "灰"] as const;
const COLOR_TONE_NAMES = ["标准", "浅色", "柔和", "深色", "暗色"] as const;
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
function number(value?: number): string { return value === undefined ? "—" : new Intl.NumberFormat("zh-CN").format(value); }
function planName(value?: string): string {
  if (!value) return "套餐未知";
  const known: Record<string, string> = { free: "Free", hobby: "Hobby", plus: "Plus", pro: "Pro", ultra: "Ultra", team: "Team", business: "Business", enterprise: "Enterprise" };
  return known[value.toLowerCase()] ?? value;
}
const CURSOR_PRO_INCLUDED_FALLBACK_CENTS = 2_000;
const CURSOR_ON_DEMAND_FALLBACK_CENTS = 50_000;
const CURSOR_ULTRA_QUOTA_KINDS = ["cursor_models", "other_models", "grok_bot"] as const satisfies readonly QuotaKind[];
const CURSOR_ULTRA_QUOTA_META: Record<QuotaKind, { label: string; hint: string }> = {
  cursor_models: { label: "Cursor Models", hint: "包含 Cursor Grok 和 Composer" },
  other_models: { label: "Other Models", hint: "其他模型额度" },
  grok_bot: { label: "Grok Bot", hint: "每周额度" },
};
function money(cents?: number): string { return cents === undefined ? "—" : `$${(cents / 100).toFixed(2)}`; }
function cursorIncludedLimit(provider: ProviderSnapshot): number {
  if (provider.cost?.includedLimitCents && provider.cost.includedLimitCents > 0) return provider.cost.includedLimitCents;
  return CURSOR_PRO_INCLUDED_FALLBACK_CENTS;
}
function dateTime(value?: string): string { return value ? new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)) : "—"; }
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
  payload = updated;
  render();
}

function withTimeout<T>(operation: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new ActionTimeoutError(`${label}超时，请稍后重试`)), timeoutMs);
    operation.then(
      (value) => { window.clearTimeout(timer); resolve(value); },
      (reason) => { window.clearTimeout(timer); reject(reason); },
    );
  });
}

function invokeWithTimeout<T>(command: string, args?: Record<string, unknown>, timeoutMs = ACTION_TIMEOUT_MS, label = "操作"): Promise<T> {
  return withTimeout(invoke<T>(command, args), timeoutMs, label);
}

function friendlyError(reason: unknown, fallback: string): string {
  if (reason instanceof ActionTimeoutError) return reason.message;
  const message = String(reason ?? "").trim();
  return message && message !== "[object Object]" ? message : fallback;
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
    showToast(friendlyError(reason, `${pending}失败，请稍后重试`), "error", 4_500);
    return undefined;
  }
}

async function refreshWithFeedback(label = "刷新", includeCursor?: boolean): Promise<void> {
  if (refreshInFlight) {
    showToast("正在刷新，请稍候", "loading", 0);
    return refreshInFlight;
  }
  const task = (async () => {
    const hasInlineProgress = view === "panel" && panelMode === "details";
    if (hasInlineProgress) {
      window.clearTimeout(toastTimer);
      document.querySelector(".action-toast")?.remove();
    } else {
      showToast(`${label}中，正在读取最新用量…`, "loading", 0);
    }
    let unlisten: (() => void) | undefined;
    try {
      let complete!: (value: AppPayload) => void;
      const completed = new Promise<AppPayload>((resolve) => { complete = resolve; });
      unlisten = await withTimeout(
        listen<AppPayload>("usage-updated", (event) => complete(event.payload)),
        ACTION_TIMEOUT_MS,
        "建立刷新监听",
      );
      if (payload) {
        payload.snapshot.refreshing = true;
        render();
      }
      const cursorWillRefresh = includeCursor ?? payload?.settings.cursorCompatEnabled ?? false;
      await invokeWithTimeout<AppPayload>("refresh_now", { includeCursor }, ACTION_TIMEOUT_MS, "启动刷新");
      const updated = await withTimeout(completed, REFRESH_TIMEOUT_MS, label);
      applyUsageUpdate(updated);
      showToast(cursorWillRefresh ? `${label}完成，数据已更新` : `${label}完成，Cursor 兼容模式未开启，已跳过`, "success");
    } catch (reason) {
      if (payload) {
        payload.snapshot.refreshing = false;
        render();
      }
      showToast(friendlyError(reason, `${label}失败，请稍后重试`), "error", 4_500);
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
    payload = await invokeWithTimeout<AppPayload>("get_app_payload", undefined, ACTION_TIMEOUT_MS, "读取用量");
    render();
  } catch (reason) {
    if (view === "panel") showToast(friendlyError(reason, "读取用量失败，请重新打开"), "error", 4_500);
  }
}

function renderLoadingPanel(): void {
  app.innerHTML = `<main class="panel details-panel loading-panel">
    <div class="panel-title"><div class="panel-brand">${metraLogo()}<div><strong>Metra</strong><small>额度与用量</small></div></div></div>
    <div class="loading-state"><i></i><strong>界面已就绪</strong><small>正在读取 Cursor、Codex 和 Claude Code 用量…</small></div>
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
  private dragged = false;
  private nativeDragging = false;
  private dragOrigin: { x: number; y: number; pointerId: number } | null = null;
  private movementSamples: BubbleMotionSample[] = [];
  private idleTimer: number | undefined;
  private dragSettleTimer: number | undefined;
  private snapToken = 0;
  private snapCompletion: Promise<void> | null = null;
  private nativeQueue: Promise<unknown> = Promise.resolve();

  constructor() {
    this.shell = document.querySelector<HTMLElement>(".bubble-shell")!;
    this.center = document.querySelector<HTMLElement>(".bubble-center")!;
    this.idleUsage = document.querySelector<HTMLElement>(".bubble-idle-usage")!;
    this.bindEvents();
    this.initialization = this.initialize();
    void this.initialization.catch((reason) => this.reportError(reason, "初始化悬浮球位置失败"));
  }

  update(activeRows: string, idleValues: string, order: ProviderName[], mode: BubblePercentMode, busy: boolean): void {
    this.center.className = `bubble-center provider-count-${order.length}`;
    this.center.innerHTML = activeRows;
    this.idleUsage.className = `bubble-idle-usage provider-count-${order.length}`;
    this.idleUsage.innerHTML = idleValues;
    const usage = order.map((provider) => {
      const snapshot = payload?.snapshot[provider];
      return `${PROVIDER_META[provider].name} ${snapshot ? percent(bubblePercent(snapshot, mode)) : "加载中"}`;
    }).join("，");
    this.shell.setAttribute("aria-label", `Metra 用量悬浮球，${usage}；拖动可吸附到屏幕边缘，按回车打开详情`);
    const wasBusy = this.busy;
    this.busy = busy;
    this.applyVisualState();
    if (busy) {
      this.clearIdleTimer();
      if (this.state === "peek") void this.reveal().catch((reason) => this.reportError(reason, "唤醒悬浮球失败"));
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
      void this.reveal().catch((reason) => this.reportError(reason, "唤醒悬浮球失败"));
    } else {
      this.scheduleIdle();
    }
  }

  private bindEvents(): void {
    this.shell.addEventListener("selectstart", (event) => event.preventDefault());
    this.shell.addEventListener("pointerenter", () => {
      this.hovering = true;
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, "唤醒悬浮球失败"));
    });
    this.shell.addEventListener("pointerleave", () => {
      this.hovering = false;
      this.scheduleIdle();
    });
    this.shell.addEventListener("focusin", () => {
      this.focused = true;
      this.clearIdleTimer();
      void this.reveal().catch((reason) => this.reportError(reason, "唤醒悬浮球失败"));
    });
    this.shell.addEventListener("focusout", () => {
      this.focused = false;
      this.scheduleIdle();
    });
    this.shell.addEventListener("pointerdown", (event) => this.onPointerDown(event));
    this.shell.addEventListener("pointermove", (event) => this.onPointerMove(event));
    this.shell.addEventListener("pointerup", (event) => this.onPointerUp(event));
    this.shell.addEventListener("pointercancel", () => this.onPointerCancel());
    this.shell.addEventListener("click", () => {
      if (this.dragged) return;
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
    void currentWindow.onMoved((event) => this.onWindowMoved(event.payload))
      .catch((reason) => this.reportError(reason, "监听悬浮球移动失败"));
    await this.dockCurrentPosition({ x: 0, y: 0 }, false);
    this.initialized = true;
    this.applyVisualState();
    if (!this.busy) this.scheduleIdle();
  }

  private onPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    window.getSelection()?.removeAllRanges();
    this.clearIdleTimer();
    this.cancelSnap();
    this.dragged = false;
    this.dragOrigin = { x: event.clientX, y: event.clientY, pointerId: event.pointerId };
    this.shell.setPointerCapture(event.pointerId);
    if (this.state === "peek") {
      void this.reveal().catch((reason) => this.reportError(reason, "唤醒悬浮球失败"));
    } else {
      this.state = "visible";
      this.applyVisualState();
    }
  }

  private onPointerMove(event: PointerEvent): void {
    if (!this.dragOrigin || !(event.buttons & 1)) return;
    if (Math.hypot(event.clientX - this.dragOrigin.x, event.clientY - this.dragOrigin.y) < BUBBLE_DRAG_THRESHOLD) return;
    const pointerId = this.dragOrigin.pointerId;
    this.dragged = true;
    this.dragOrigin = null;
    if (this.shell.hasPointerCapture(pointerId)) this.shell.releasePointerCapture(pointerId);
    void this.beginNativeDrag();
  }

  private onPointerUp(event: PointerEvent): void {
    this.dragOrigin = null;
    if (this.shell.hasPointerCapture(event.pointerId)) this.shell.releasePointerCapture(event.pointerId);
    if (this.nativeDragging) this.scheduleDragFinish(24);
    window.setTimeout(() => { this.dragged = false; }, 0);
  }

  private onPointerCancel(): void {
    this.dragOrigin = null;
    if (this.nativeDragging) this.scheduleDragFinish(BUBBLE_DRAG_SETTLE_MS);
    else this.scheduleIdle();
  }

  private async beginNativeDrag(): Promise<void> {
    await this.initialization;
    if (this.state === "peek") await this.reveal();
    this.cancelSnap();
    const position = await this.afterNativeQueue(() => currentWindow.outerPosition());
    this.nativeDragging = true;
    this.state = "dragging";
    this.movementSamples = [{ x: position.x, y: position.y, time: performance.now() }];
    this.applyVisualState();
    try {
      await withTimeout(currentWindow.startDragging(), ACTION_TIMEOUT_MS, "拖动气泡");
      this.scheduleDragFinish(BUBBLE_DRAG_SETTLE_MS);
    } catch (reason) {
      this.nativeDragging = false;
      this.state = "visible";
      this.applyVisualState();
      this.reportError(reason, "无法拖动气泡");
      this.scheduleIdle();
    }
  }

  private onWindowMoved(position: PhysicalPosition): void {
    if (!this.nativeDragging) return;
    const now = performance.now();
    this.movementSamples.push({ x: position.x, y: position.y, time: now });
    this.movementSamples = this.movementSamples.filter((sample) => now - sample.time <= BUBBLE_SAMPLE_WINDOW_MS);
    this.scheduleDragFinish(BUBBLE_DRAG_SETTLE_MS);
  }

  private scheduleDragFinish(delay: number): void {
    window.clearTimeout(this.dragSettleTimer);
    this.dragSettleTimer = window.setTimeout(() => {
      if (!this.nativeDragging) return;
      this.nativeDragging = false;
      const velocity = bubbleReleaseVelocity(this.movementSamples);
      this.snapCompletion = this.dockCurrentPosition(velocity, true).finally(() => { this.snapCompletion = null; });
    }, delay);
  }

  private async dockCurrentPosition(velocity: BubblePoint, animate: boolean): Promise<void> {
    const [position, size, monitors] = await Promise.all([
      this.afterNativeQueue(() => currentWindow.outerPosition()),
      this.afterNativeQueue(() => currentWindow.outerSize()),
      availableMonitors(),
    ]);
    const geometries = monitors.map((monitor) => ({
      x: monitor.workArea.position.x,
      y: monitor.workArea.position.y,
      width: monitor.workArea.size.width,
      height: monitor.workArea.size.height,
      scaleFactor: monitor.scaleFactor,
    }));
    const monitor = selectBubbleMonitor(geometries, position, size);
    if (!monitor) return;
    const target = calculateBubbleDockTarget(position, velocity, monitor);
    this.side = target.side;
    this.monitor = monitor;
    this.fullSize = target.size;
    this.state = animate ? "snapping" : "visible";
    this.applyVisualState();
    await this.resizeWindow(target.size);
    const completed = animate
      ? await this.animateTo(target.position, velocity)
      : await this.moveWindow(target.position).then(() => true);
    if (!completed) return;
    this.anchor = target.position;
    this.state = "visible";
    this.applyVisualState();
    await this.saveAnchor();
    this.scheduleIdle();
  }

  private async animateTo(target: BubblePoint, releaseVelocity: BubblePoint): Promise<boolean> {
    const token = ++this.snapToken;
    const start = await this.afterNativeQueue(() => currentWindow.outerPosition());
    if (this.reducedMotion) {
      await this.moveWindow(target);
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
      const elapsed = now - started;
      const delta = Math.min((now - previous) / 1_000, 0.032);
      previous = now;
      velocityX += (-stiffness * (x - target.x) - damping * velocityX) * delta;
      velocityY += (-stiffness * (y - target.y) - damping * velocityY) * delta;
      x += velocityX * delta;
      y += velocityY * delta;
      await this.moveWindow({ x: Math.round(x), y: Math.round(y) });
      const settled = Math.hypot(x - target.x, y - target.y) < 0.75
        && Math.hypot(velocityX, velocityY) < 5;
      if (settled || elapsed > 720) break;
    }
    if (token !== this.snapToken) return false;
    await this.moveWindow(target);
    return true;
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
    if (!this.initialized || this.busy || this.panelVisible || this.hovering || this.focused || this.nativeDragging || this.state !== "visible") return;
    this.idleTimer = window.setTimeout(() => {
      void this.enterPeek().catch((reason) => this.reportError(reason, "隐藏悬浮球失败"));
    }, BUBBLE_IDLE_DELAY_MS);
  }

  private async enterPeek(): Promise<void> {
    if (this.busy || this.panelVisible || this.hovering || this.focused || this.nativeDragging || this.state !== "visible") return;
    await this.initialization;
    if (!this.anchor || !this.fullSize || !this.monitor) return;
    this.state = "peek";
    this.applyVisualState();
    const frame = calculateBubblePeekFrame(this.anchor, this.fullSize, this.side, this.monitor.scaleFactor);
    await this.setWindowFrame(frame.position, frame.size);
  }

  private async reveal(): Promise<void> {
    this.clearIdleTimer();
    await this.initialization;
    if (this.state !== "peek") {
      if (this.state !== "dragging" && this.state !== "snapping") this.state = "visible";
      this.applyVisualState();
      return;
    }
    if (!this.anchor || !this.fullSize) return;
    this.state = "visible";
    this.applyVisualState();
    await this.setWindowFrame(this.anchor, this.fullSize);
  }

  private applyVisualState(): void {
    this.shell.dataset.state = this.state === "peek" ? "idle" : this.state;
    this.shell.dataset.side = this.side;
  }

  private resizeWindow(size: BubbleSize): Promise<void> {
    return this.queueNative(() => currentWindow.setSize(new PhysicalSize(size.width, size.height)));
  }

  private moveWindow(position: BubblePoint): Promise<void> {
    return this.queueNative(() => currentWindow.setPosition(new PhysicalPosition(Math.round(position.x), Math.round(position.y))));
  }

  private setWindowFrame(position: BubblePoint, size: BubbleSize): Promise<void> {
    return this.queueNative(async () => {
      await Promise.all([
        currentWindow.setSize(new PhysicalSize(size.width, size.height)),
        currentWindow.setPosition(new PhysicalPosition(Math.round(position.x), Math.round(position.y))),
      ]);
    });
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

  private async saveAnchor(): Promise<void> {
    if (!this.anchor) return;
    try {
      await invokeWithTimeout<AppSettings>(
        "save_window_position",
        { x: Math.round(this.anchor.x), y: Math.round(this.anchor.y) },
        ACTION_TIMEOUT_MS,
        "保存气泡位置",
      );
    } catch (reason) {
      this.reportError(reason, "保存气泡位置失败");
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
    bubbleController = new BubbleWindowController();
  }
  const busy = !snapshot || snapshot.refreshing || Object.keys(tokenGains).length > 0;
  bubbleController.update(rows, idleValues, order, mode, busy);
}

function quotaRows(provider: ProviderSnapshot): string {
  if (!provider.quotas.length) return `<p class="empty">${escapeHtml(provider.message ?? "当前数据源未提供额度")}</p>`;
  return provider.quotas.map((quota) => `<div class="quota">
    <div class="quota-head"><span>${escapeHtml(quota.label)}</span><strong>剩余 ${Math.round(quota.remainingPercent)}%</strong></div>
    <div class="track"><i style="width:${Math.max(0, Math.min(100, quota.remainingPercent))}%"></i></div>
    <div class="quota-meta"><span>已用 ${Math.round(quota.usedPercent)}%</span><span>${quota.resetsAt ? `${dateTime(quota.resetsAt)} 重置` : ""}</span></div>
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
  if (!quota) {
    return `<div class="cursor-ultra-quota unavailable" data-quota-kind="${kind}">
      <div class="quota-head"><span>${meta.label}</span><strong>等待数据</strong></div>
      <div class="track"><i style="width:0%"></i></div>
      <div class="quota-meta"><span>${meta.hint}</span><span>—</span></div>
    </div>`;
  }
  return `<div class="cursor-ultra-quota" data-quota-kind="${kind}">
    <div class="quota-head"><span>${escapeHtml(quota.label || meta.label)}</span><strong>剩余 ${Math.round(quota.remainingPercent)}%</strong></div>
    <div class="track"><i style="width:${Math.max(0, Math.min(100, quota.remainingPercent))}%"></i></div>
    <div class="quota-meta"><span>已用 ${Math.round(quota.usedPercent)}%</span><span>${quota.resetsAt ? `${dateTime(quota.resetsAt)} 重置` : meta.hint}</span></div>
  </div>`;
}

function cursorCostBlock(label: string, used: number | undefined, limit: number, extraClass = ""): string {
  const remaining = used === undefined ? undefined : Math.max(0, limit - used);
  const remainingPercent = remaining === undefined ? 0 : Math.max(0, Math.min(100, remaining * 100 / limit));
  return `<div class="cursor-cost-block ${extraClass}">
    <div class="quota-head"><span>${label}</span><strong>${remaining === undefined ? "等待数据" : `剩余 ${money(remaining)}`}</strong></div>
    <div class="track"><i style="width:${remainingPercent}%"></i></div>
    <div class="quota-meta"><span>已用 ${money(used)}</span><span>限额 ${money(limit)}</span></div>
  </div>`;
}

function cursorUltraOnDemandBlock(provider: ProviderSnapshot): string {
  const cost = provider.cost;
  if (cost?.onDemandEnabled === false) {
    return `<div class="cursor-ultra-on-demand disabled">
      <div class="quota-head"><span>On-Demand</span><strong>未启用</strong></div>
      <div class="quota-meta"><span>按量付费当前未启用</span><span>—</span></div>
    </div>`;
  }
  if (cost?.onDemandEnabled === true && cost.onDemandLimitCents !== undefined && cost.onDemandLimitCents > 0) {
    return cursorCostBlock("On-Demand", cost.onDemandUsedCents, cost.onDemandLimitCents, "cursor-ultra-on-demand enabled");
  }
  if (cost?.onDemandEnabled === true) {
    return `<div class="cursor-ultra-on-demand enabled">
      <div class="quota-head"><span>On-Demand</span><strong>已启用</strong></div>
      <div class="quota-meta"><span>已用 ${money(cost.onDemandUsedCents)}</span><span>限额等待数据</span></div>
    </div>`;
  }
  return `<div class="cursor-ultra-on-demand unknown">
    <div class="quota-head"><span>On-Demand</span><strong>等待数据</strong></div>
    <div class="quota-meta"><span>按量付费状态暂不可用</span><span>—</span></div>
  </div>`;
}

function cursorUltraBlocks(provider: ProviderSnapshot): string {
  return `<div class="cursor-ultra-blocks">
    ${CURSOR_ULTRA_QUOTA_KINDS.map((kind) => cursorUltraQuotaBlock(provider, kind)).join("")}
    ${cursorUltraOnDemandBlock(provider)}
    ${provider.cost?.periodEnd ? `<div class="cursor-reset"><span>额度重置</span><strong>${dateTime(provider.cost.periodEnd)}</strong></div>` : ""}
  </div>`;
}

function cursorCostBlocks(provider: ProviderSnapshot): string {
  const cost = provider.cost;
  const includedLimit = cursorIncludedLimit(provider);
  const onDemandLimit = cost?.onDemandLimitCents ?? CURSOR_ON_DEMAND_FALLBACK_CENTS;
  return `<div class="cursor-costs">
    ${cursorCostBlock(`订阅额度 · ${money(includedLimit)}`, cost?.includedUsedCents, includedLimit)}
    ${cursorCostBlock(`On-Demand · ${money(onDemandLimit)}`, cost?.onDemandUsedCents, onDemandLimit)}
    ${cost?.periodEnd ? `<div class="cursor-reset"><span>额度重置</span><strong>${dateTime(cost.periodEnd)}</strong></div>` : ""}
  </div>`;
}
function providerLoading(name: string): string {
  return `<div class="provider-loading" role="status">
    <i aria-hidden="true"></i>
    <div><strong>正在连接 ${escapeHtml(name)} CLI</strong><small>正在检查安装、登录与用量信息…</small></div>
  </div>`;
}
function cursorLoginPrompt(cursorCompat: boolean): string {
  return `<div class="cursor-login-prompt">
    <i aria-hidden="true">↗</i>
    <div><strong>${cursorCompat ? "Cursor 登录已失效" : "登录后继续读取额度"}</strong><small>${cursorCompat ? "若 Cursor 仍显示已登录，请先退出后重新登录" : "浏览器授权完成后返回 Metra，将自动检测"}</small></div>
  </div>`;
}
function providerCard(name: string, provider: ProviderSnapshot): string {
  const token = provider.provider === "codex" || provider.provider === "claude" ? provider.tokens : undefined;
  const cost = provider.cost;
  const todayTokenLabel = "本机今日 token";
  const lifetimeTokenLabel = provider.provider === "claude" ? "本机累计 token" : "官方累计 token";
  const cursorCompat = provider.provider === "cursor" && Boolean(payload?.settings.cursorCompatEnabled);
  const cursorNeedsLogin = provider.provider === "cursor" && provider.status === "not_logged_in";
  const cursorUsageAvailable = cursorCompat && provider.status === "available";
  const cursorCanEnableUsage = provider.provider === "cursor" && (provider.status === "available" || provider.status === "desktop_installed");
  const cursorUltra = cursorUsageAvailable && isCursorUltraUsage(provider);
  const loading = Boolean(payload?.snapshot.refreshing && !provider.quotas.length && !provider.cost && !provider.tokens);
  const usage = loading ? providerLoading(name) : cursorNeedsLogin ? cursorLoginPrompt(cursorCompat) : cursorUltra ? cursorUltraBlocks(provider) : cursorUsageAvailable ? cursorCostBlocks(provider) : quotaRows(provider);
  const displayedStatus = loading ? "连接中" : provider.stale ? "数据已过期" : statusText[provider.status];
  const statusTone = loading ? "pending" : provider.stale ? "stale" : provider.status === "available" ? "available" : provider.status === "desktop_installed" ? "pending" : "unavailable";
  return `<section id="provider-card-${provider.provider}" class="provider-card ${provider.stale ? "is-stale" : ""}" data-provider-card="${provider.provider}" data-provider-accent="${provider.provider}" style="${providerColorStyle(provider.provider, payload?.settings)}" tabindex="-1" aria-labelledby="provider-title-${provider.provider}">
    <header><div><span class="provider-dot ${provider.provider}" aria-hidden="true"></span><strong id="provider-title-${provider.provider}">${name}</strong></div><span class="status ${statusTone}">${displayedStatus}</span></header>
    <div class="plan">${escapeHtml(planName(provider.plan))} · ${dateTime(provider.capturedAt)}</div>
    ${usage}
    ${cursorNeedsLogin && !loading ? `<button class="compat-cta cursor-login-cta" id="login-cursor">${cursorCompat ? "在 Cursor 中重新登录" : "登录 Cursor"}</button>` : cursorCanEnableUsage && !cursorCompat && !loading ? `<button class="compat-cta" id="enable-cursor-usage">读取 Cursor 精确用量</button>` : ""}
    ${token ? `<div class="metrics"><div><span>${todayTokenLabel}</span><strong>${number(token.today)}</strong></div><div><span>${lifetimeTokenLabel}</span><strong>${number(token.lifetime)}</strong></div></div>` : ""}
    ${cost && provider.provider !== "cursor" ? `<div class="metrics"><div><span>Included</span><strong>${money(cost.includedUsedCents)} / ${money(cost.includedLimitCents)}</strong></div><div><span>On-Demand</span><strong>${money(cost.onDemandUsedCents)} / ${money(cost.onDemandLimitCents)}</strong></div></div>` : ""}
    ${provider.message && (provider.quotas.length || cursorCompat) ? `<p class="notice">${escapeHtml(provider.message)}</p>` : ""}
  </section>`;
}

function bubbleConfigItem(provider: ProviderName, settings: AppSettings): string {
  const name = PROVIDER_META[provider].name;
  const color = bubbleProviderColor(provider, settings);
  const visible = bubbleVisibleProviderOrder(settings).includes(provider);
  return `<div class="bubble-config-item ${visible ? "" : "is-hidden"}" data-bubble-provider="${provider}" data-bubble-color="${color}" data-bubble-visible="${visible}">
    <button type="button" class="drag-handle" title="拖动调整顺序" aria-label="拖动调整 ${name} 的显示顺序">⠿</button>
    <button type="button" class="bubble-config-dot color-trigger ${provider}" data-color-provider="${provider}" data-provider-accent="${provider}" style="--provider-color:${color}" aria-haspopup="dialog" aria-expanded="false" aria-controls="provider-color-palette" aria-label="选择 ${name} 标记颜色，当前 ${color}" title="选择 ${name} 标记颜色"></button>
    <a class="bubble-config-name provider-nav" href="#provider-card-${provider}" aria-label="查看 ${name} 统计">${name}<span aria-hidden="true">›</span></a>
    <input type="text" maxlength="3" value="${escapeHtml(bubbleProviderLabel(provider, settings))}" aria-label="${name} 在悬浮球上的显示字符" spellcheck="false">
    <button type="button" class="visibility-toggle" role="switch" aria-checked="${visible}" aria-disabled="false" aria-label="${name} 在悬浮球中显示" title="${visible ? `隐藏 ${name}` : `显示 ${name}`}"><span aria-hidden="true"></span></button>
  </div>`;
}

function renderBubbleConfig(settings: AppSettings): string {
  return `<section class="bubble-config" aria-labelledby="bubble-config-title">
    <div class="bubble-config-head"><strong id="bubble-config-title">悬浮球显示</strong><small>排序 · 显示 · 字符 · 颜色</small></div>
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
  popover.setAttribute("aria-label", `选择 ${PROVIDER_META[provider].name} 标记颜色`);
  popover.innerHTML = COLOR_PALETTE.map((row, rowIndex) => `<div class="color-palette-row" role="group" aria-label="${COLOR_TONE_NAMES[rowIndex]}色">
    ${row.map((color, columnIndex) => `<button type="button" class="color-swatch ${color === selectedColor ? "selected" : ""}" data-color-value="${color}" style="--swatch-color:${color}" aria-label="${COLOR_TONE_NAMES[rowIndex]}${COLOR_HUE_NAMES[columnIndex]}色 ${color}" aria-pressed="${color === selectedColor}" tabindex="${color === selectedColor ? "0" : "-1"}"></button>`).join("")}
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
    trigger.setAttribute("aria-label", `选择 ${PROVIDER_META[provider].name} 标记颜色，当前 ${color}`);
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
    if (event.key === "ArrowUp") nextIndex = currentIndex >= COLOR_HUE_NAMES.length ? currentIndex - COLOR_HUE_NAMES.length : currentIndex;
    if (event.key === "ArrowDown") nextIndex = currentIndex + COLOR_HUE_NAMES.length < swatches.length ? currentIndex + COLOR_HUE_NAMES.length : currentIndex;
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
        "保存悬浮球显示设置",
      );
      if (payload) payload.settings = settings;
    } catch (reason) {
      showToast(friendlyError(reason, "保存悬浮球显示设置失败"), "error", 4_500);
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
  toggle.setAttribute("aria-label", `${name} 在悬浮球中显示`);
  toggle.title = visible ? `隐藏 ${name}` : `显示 ${name}`;
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
    toggle.title = locked ? "悬浮球至少保留一个显示项" : visible ? `隐藏 ${name}` : `显示 ${name}`;
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
        showToast("悬浮球至少保留一个显示项", "info", 2_200);
        return;
      }
      updateVisibilityControl(item, visibilityToggle, !visible);
      syncVisibilityToggleAvailability(editor);
      saveBubbleDisplayConfig(editor);
      showToast(`${visible ? "已从悬浮球隐藏" : "已在悬浮球显示"} ${PROVIDER_META[provider].name}`, "info", 1_600);
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
      <div class="panel-brand">${metraLogo()}<div><strong>Metra</strong><small>额度与用量</small></div></div>
      ${payload.snapshot.refreshing ? `<div class="refresh-status" role="status"><i></i><span>正在更新</span></div>` : ""}
      <button id="refresh" class="icon-btn" title="${payload.snapshot.refreshing ? "正在刷新" : "立即刷新"}" aria-label="${payload.snapshot.refreshing ? "正在刷新用量" : "立即刷新用量"}"><span aria-hidden="true">↻</span></button>
    </div>
    ${renderBubbleConfig(payload.settings)}
    <div class="provider-list">${providerCard("Cursor", payload.snapshot.cursor)}${providerCard("Codex", payload.snapshot.codex)}${providerCard("Claude Code", payload.snapshot.claude)}</div>
    <footer>每 ${payload.settings.refreshMinutes} 分钟刷新${payload.snapshot.refreshing ? " · 正在刷新…" : ""}</footer>
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
    button.textContent = "正在打开…";
  }
  try {
    const started = await invokeWithTimeout<CursorLoginStart>("start_cursor_login", undefined, ACTION_TIMEOUT_MS, "打开 Cursor 登录");
    const message = started.method === "agent"
      ? started.alreadyRunning ? "Cursor 登录流程已在进行，请在浏览器中完成授权" : "已打开 Cursor 官方登录，请在浏览器中完成授权"
      : "已打开 Cursor 账户设置；若仍显示已登录，请先退出后重新登录";
    showToast(message, "success", 4_500);
  } catch (reason) {
    cursorLoginPending = false;
    if (button) {
      button.disabled = false;
      button.removeAttribute("aria-busy");
      button.textContent = "重试登录 Cursor";
    }
    showToast(friendlyError(reason, "无法打开 Cursor 登录，请稍后重试"), "error", 4_500);
  }
}

async function recheckCursorLogin(): Promise<void> {
  if (!cursorLoginPending || cursorLoginRecheckInFlight) return cursorLoginRecheckInFlight ?? undefined;
  const task = (async () => {
    try {
      showToast("正在检测 Cursor 登录状态…", "loading", 0);
      const updated = await invokeWithTimeout<AppPayload>("recheck_cursor_login", undefined, REFRESH_TIMEOUT_MS, "检测 Cursor 登录");
      applyUsageUpdate(updated);
      const loggedIn = updated.snapshot.cursor.status === "available";
      cursorLoginPending = false;
      showToast(loggedIn ? "Cursor 登录成功，数据已更新" : "尚未检测到登录，请在 Cursor 中完成后再返回", loggedIn ? "success" : "info", 4_500);
      if (!loggedIn) requestAnimationFrame(() => document.querySelector<HTMLButtonElement>("#login-cursor")?.focus());
    } catch (reason) {
      cursorLoginPending = false;
      showToast(friendlyError(reason, "检测 Cursor 登录失败，请点击刷新重试"), "error", 4_500);
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
    <h2 id="consent-title">开启 Cursor 个人兼容模式？</h2>
    <p>Metra 将只读 Cursor 本地状态库中的现有登录令牌，仅向 <b>api2.cursor.sh</b> 和 <b>cursor.com</b> 请求用量。</p>
    <p>令牌仅在本次请求的内存中存在，不会保存或写入日志。</p>
    <div class="consent-error" aria-live="polite"></div>
    <div class="consent-actions"><button id="consent-cancel">取消</button><button id="consent-confirm" class="primary">确认开启</button></div>
  </section>`;
  app.append(consent);
  const cancelButton = consent.querySelector<HTMLButtonElement>("#consent-cancel")!;
  const confirmButton = consent.querySelector<HTMLButtonElement>("#consent-confirm")!;
  const error = consent.querySelector<HTMLElement>(".consent-error")!;
  cancelButton.onclick = () => consent.remove();
  confirmButton.onclick = async () => {
    confirmButton.disabled = true;
    cancelButton.disabled = true;
    confirmButton.textContent = "正在开启…";
    error.textContent = "正在读取 Cursor 用量，请稍候";
    try {
      payload!.settings = await invokeWithTimeout<AppSettings>("set_cursor_compat", { enabled: true }, ACTION_TIMEOUT_MS, "开启 Cursor 兼容模式");
      payload!.snapshot.refreshing = true;
      panelMode = "details";
      renderDetails();
      showToast("兼容模式已开启，正在读取 Cursor 用量", "success");
    } catch (reason) {
      error.textContent = `开启失败：${String(reason)}`;
      confirmButton.disabled = false;
      cancelButton.disabled = false;
      confirmButton.textContent = "重试";
    }
  };
}

function renderMenu(): void {
  if (!payload) return;
  closeColorPalette(false);
  const s = payload.settings;
  app.innerHTML = `<main class="panel menu-panel">
    <div class="menu-brand">${metraLogo()}<div><strong>Metra</strong><small>桌面用量气泡</small></div></div>
    <button data-action="refresh"><span>立即刷新</span></button>
    <div class="menu-label">刷新间隔</div>
    <div class="intervals">${[1, 5, 15, 30, 60].map((n) => `<button data-interval="${n}" class="${s.refreshMinutes === n ? "selected" : ""}">${n < 60 ? `${n}m` : "1h"}</button>`).join("")}</div>
    <div class="menu-label">气泡百分比</div>
    <div class="percent-modes"><button data-percent-mode="used" class="${s.bubblePercentMode === "used" ? "selected" : ""}">已用</button><button data-percent-mode="remaining" class="${s.bubblePercentMode === "remaining" ? "selected" : ""}">剩余</button></div>
    <button data-action="autostart"><span>开机启动</span><i class="switch ${s.autostart ? "on" : ""}"></i></button>
    <button data-action="compat"><span><b>Cursor 个人兼容模式</b><small>只读本地令牌，仅访问 Cursor</small></span><i class="switch ${s.cursorCompatEnabled ? "on" : ""}"></i></button>
    <button data-action="rescan"><span>重新检测 CLI</span><kbd>↻</kbd></button>
    <button data-action="quit" class="danger"><span>退出 Metra</span></button>
  </main>`;
  app.querySelectorAll<HTMLButtonElement>("[data-percent-mode]").forEach((button) => button.onclick = () => {
    void (async () => {
      const mode = button.dataset.percentMode as BubblePercentMode;
      if (mode === payload!.settings.bubblePercentMode) return;
      button.disabled = true;
      const label = mode === "used" ? "已用" : "剩余";
      const settings = await runUiAction<AppSettings>(
        `正在切换为${label}百分比…`,
        `气泡已显示${label}百分比`,
        () => invokeWithTimeout<AppSettings>("set_bubble_percent_mode", { mode }, ACTION_TIMEOUT_MS, "切换气泡百分比"),
      );
      if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
    })();
  });
  app.querySelectorAll<HTMLButtonElement>("[data-interval]").forEach((button) => button.onclick = () => {
    void (async () => {
      button.disabled = true;
      const settings = await runUiAction<AppSettings>(
        "正在更新刷新间隔…",
        "刷新间隔已更新",
        () => invokeWithTimeout<AppSettings>("set_refresh_interval", { minutes: Number(button.dataset.interval) }, ACTION_TIMEOUT_MS, "更新刷新间隔"),
      );
      if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
    })();
  });
  app.querySelectorAll<HTMLButtonElement>("[data-action]").forEach((button) => button.onclick = () => {
    void (async () => {
      const action = button.dataset.action;
      if (action === "refresh" || action === "rescan") {
        await refreshWithFeedback(action === "rescan" ? "重新检测" : "刷新", action === "rescan" ? true : undefined);
        return;
      }
      button.disabled = true;
      if (action === "autostart") {
        const settings = await runUiAction<AppSettings>(
          "正在更新开机启动…",
          "开机启动设置已更新",
          () => invokeWithTimeout<AppSettings>("set_autostart", { enabled: !payload!.settings.autostart }, ACTION_TIMEOUT_MS, "更新开机启动"),
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
          showToast(payload!.snapshot.cursor.message ?? "当前无法读取 Cursor 账号或额度", "info", 4_500);
          return;
        }
        if (enable) { button.disabled = false; enableCursorUsage(); return; }
        const settings = await runUiAction<AppSettings>(
          "正在关闭兼容模式…",
          "Cursor 兼容模式已关闭",
          () => invokeWithTimeout<AppSettings>("set_cursor_compat", { enabled: false }, ACTION_TIMEOUT_MS, "关闭 Cursor 兼容模式"),
        );
        if (settings) { payload!.settings = settings; renderMenu(); } else { button.disabled = false; }
        return;
      }
      if (action === "quit") {
        await runUiAction<void>("正在退出…", "", () => invokeWithTimeout<void>("quit_app", undefined, ACTION_TIMEOUT_MS, "退出 Metra"));
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
    await invokeWithTimeout<number>(
      "show_panel",
      { mode, toggle, requestId },
      PANEL_SHOW_TIMEOUT_MS,
      "显示弹窗",
    );
  } catch (reason) {
    if (requestId === panelRequestSequence) {
      showToast(friendlyError(reason, "打开弹窗失败，请重试"), "error", 4_500);
    }
  }
}

async function hidePanelWindow(): Promise<void> {
  await withTimeout(currentWindow.hide(), ACTION_TIMEOUT_MS, "收起弹窗");
  await withTimeout(emit("panel-visibility-changed", { visible: false }), ACTION_TIMEOUT_MS, "同步弹窗状态");
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
          const stillFocused = await withTimeout(currentWindow.isFocused(), ACTION_TIMEOUT_MS, "检查弹窗焦点");
          if (!stillFocused) await hidePanelWindow();
        } catch (reason) {
          showToast(friendlyError(reason, "收起弹窗失败"), "error", 4_500);
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
        .catch((reason) => showToast(friendlyError(reason, "收起弹窗失败"), "error", 4_500));
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
  if (!payload) return;
  payload.settings = event.payload;
  if (view === "bubble") renderBubble();
});
void listen<AppPayload>("usage-updated", (event) => applyUsageUpdate(event.payload));
void listen<{ success: boolean; message: string }>("cursor-login-finished", (event) => {
  if (event.payload.success) return;
  cursorLoginPending = false;
  showToast(event.payload.message, "error", 4_500);
});
void loadPayload();
