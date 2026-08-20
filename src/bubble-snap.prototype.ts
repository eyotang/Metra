import "./bubble-snap.prototype.css";

// PROTOTYPE — three snap-to-edge treatments, switchable with ?variant=A|B|C.
type VariantKey = "A" | "B" | "C";
type EdgeSide = "left" | "right";
type BubbleState = "resting" | "pressing" | "dragging" | "snapping" | "idle" | "expanded";

interface VariantDefinition {
  key: VariantKey;
  name: string;
  summary: string;
  visibleWidth: number;
  panelWidth: number;
  panelHeight: number;
}

interface Point {
  x: number;
  y: number;
}

interface PointerSample extends Point {
  time: number;
}

interface PointerSession {
  id: number;
  startPointer: Point;
  startPosition: Point;
  startedExpanded: boolean;
  dragged: boolean;
  samples: PointerSample[];
}

const variants: readonly VariantDefinition[] = [
  {
    key: "A",
    name: "半隐圆球",
    summary: "完整形态保留现有信息层级；闲置后露出半圆，只显示三组彩色用量。",
    visibleWidth: 30,
    panelWidth: 258,
    panelHeight: 210,
  },
  {
    key: "B",
    name: "边缘胶囊",
    summary: "吸边后收成纵向胶囊，用颜色区分 AI；展开为向内散开的快捷用量。",
    visibleWidth: 35,
    panelWidth: 254,
    panelHeight: 192,
  },
  {
    key: "C",
    name: "用量轨道",
    summary: "闲置时压缩成极窄用量轨道；唤醒后从边缘拉出横向统计条。",
    visibleWidth: 24,
    panelWidth: 322,
    panelHeight: 116,
  },
] as const;

const usage = [
  { provider: "Cursor", short: "C", value: 72, color: "#9c83ff", reset: "3 小时 12 分" },
  { provider: "Codex", short: "X", value: 41, color: "#4bd8c0", reset: "明天 08:00" },
  { provider: "Claude Code", short: "A", value: 88, color: "#e99068", reset: "1 小时 46 分" },
] as const;

const BUBBLE_SIZE = 56;
const SNAP_INSET = 16;
const SAFE_Y = 18;
const IDLE_DELAY_MS = 2_800;
const DRAG_THRESHOLD = 8;

const root = document.querySelector<HTMLDivElement>("#prototype-root")!;

function activeUsageMarkup(): string {
  return usage.map((item) => `
    <span class="active-usage-row" style="--usage-color:${item.color}">
      <b>${item.short}</b><strong>${item.value}</strong>
    </span>
  `).join("");
}

function idleUsageMarkup(): string {
  return usage.map((item) => `
    <strong class="idle-usage-value" style="--usage-color:${item.color}">${item.value}<small>%</small></strong>
  `).join("");
}

function detailMarkup(variant: VariantKey): string {
  if (variant === "A") {
    return `<section class="detail-surface detail-card" aria-label="用量速览">
      <header><div><strong>Metra</strong><small>用量速览</small></div><span>刚刚更新</span></header>
      <div class="detail-card-list">
        ${usage.map((item) => `<div class="detail-card-row" style="--usage-color:${item.color}">
          <i aria-hidden="true"></i><span><b>${item.provider}</b><small>${item.reset}后重置</small></span><strong>${item.value}%</strong>
        </div>`).join("")}
      </div>
      <footer>点击任意项目查看完整统计 <span aria-hidden="true">→</span></footer>
    </section>`;
  }
  if (variant === "B") {
    return `<section class="detail-surface detail-fan" aria-label="AI 用量快捷入口">
      <div class="fan-items">
        ${usage.map((item) => `<button type="button" class="fan-item prototype-action" style="--usage-color:${item.color}" aria-label="${item.provider} 已用 ${item.value}%">
          <i aria-hidden="true">${item.short}</i><span><b>${item.provider}</b><strong>${item.value}%</strong></span>
        </button>`).join("")}
      </div>
      <button type="button" class="fan-refresh prototype-action"><span aria-hidden="true">↻</span> 刷新全部</button>
    </section>`;
  }
  return `<section class="detail-surface detail-rail" aria-label="横向用量轨道">
    <header><strong>当前用量</strong><span>3 个服务 · 刚刚更新</span></header>
    <div class="rail-items">
      ${usage.map((item) => `<button type="button" class="rail-item prototype-action" style="--usage-color:${item.color}" aria-label="${item.provider} 已用 ${item.value}%">
        <i aria-hidden="true"></i><span><b>${item.provider}</b><small>${item.reset}</small></span><strong>${item.value}%</strong>
      </button>`).join("")}
    </div>
  </section>`;
}

function mockDashboardMarkup(): string {
  return `<div class="mock-window-chrome">
      <div class="mock-traffic" aria-hidden="true"><i></i><i></i><i></i></div>
      <div class="mock-address">workspace / usage-overview</div>
      <div class="mock-window-actions" aria-hidden="true">⌁ ···</div>
    </div>
    <div class="mock-app-shell">
      <aside class="mock-sidebar" aria-hidden="true">
        <div class="mock-brand"><span>M</span><strong>Metra</strong></div>
        <nav><i class="selected"></i><i></i><i></i><i></i></nav>
        <div class="mock-user"></div>
      </aside>
        <section class="mock-content" aria-hidden="true">
        <header><div><small>Overview</small><strong>AI 用量概览</strong></div><button type="button" tabindex="-1">本周⌄</button></header>
        <div class="mock-summary-grid">
          <article><span>总请求</span><strong>12,482</strong><small>较上周 +18%</small></article>
          <article><span>今日 Token</span><strong>2.84M</strong><small>运行状态正常</small></article>
          <article><span>下次重置</span><strong>1h 46m</strong><small>Claude Code</small></article>
        </div>
        <div class="mock-chart-card">
          <div class="mock-chart-head"><span><b>用量趋势</b><small>最近 7 天</small></span><i></i></div>
          <div class="mock-chart"><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>
        </div>
        <div class="mock-provider-grid">
          ${usage.map((item) => `<article style="--usage-color:${item.color}"><header><i></i><span><b>${item.provider}</b><small>已连接</small></span></header><strong>${item.value}%</strong><div><i style="width:${item.value}%"></i></div></article>`).join("")}
        </div>
      </section>
    </div>`;
}

function selectedVariant(): VariantDefinition {
  const requested = new URLSearchParams(location.search).get("variant")?.toUpperCase();
  return variants.find((variant) => variant.key === requested) ?? variants[0];
}

let controller: BubblePrototype | null = null;

function renderPrototype(variant: VariantDefinition): void {
  controller?.destroy();
  root.innerHTML = `<main class="prototype-page variant-${variant.key.toLowerCase()}">
    <header class="prototype-header">
      <div><span class="prototype-kicker">INTERACTION PROTOTYPE</span><h1>吸边悬浮球</h1><p>${variant.summary}</p></div>
      <div class="prototype-actions" aria-label="原型控制">
        <button type="button" data-control="idle">模拟闲置</button>
        <button type="button" data-control="side">切换边缘</button>
        <button type="button" data-control="reset">重置</button>
      </div>
    </header>
    <section class="monitor-frame" aria-label="模拟桌面区域">
      <div class="desktop-stage" data-variant="${variant.key}" data-side="right" data-interaction-state="resting">
        ${mockDashboardMarkup()}
        <div class="snap-zone snap-zone-left" data-snap-zone="left" aria-hidden="true"><span>吸附到左侧</span></div>
        <div class="snap-zone snap-zone-right" data-snap-zone="right" aria-hidden="true"><span>吸附到右侧</span></div>
        <button type="button" class="floating-bubble" data-role="bubble" data-state="resting" data-side="right" aria-controls="bubble-detail-panel" aria-expanded="false" aria-label="Metra 用量悬浮球：Cursor 72%，Codex 41%，Claude Code 88%；拖动可吸附到屏幕边缘，按回车展开">
          <span class="bubble-shadow" aria-hidden="true"></span>
          <span class="bubble-surface">
            <span class="bubble-shine" aria-hidden="true"></span>
            <span class="active-usage" aria-hidden="true">${activeUsageMarkup()}</span>
            <span class="idle-usage" aria-label="Cursor 72%，Codex 41%，Claude Code 88%">${idleUsageMarkup()}</span>
          </span>
          <span class="edge-grip" aria-hidden="true"></span>
        </button>
        <div class="bubble-detail" id="bubble-detail-panel" data-role="detail" data-side="right">${detailMarkup(variant.key)}</div>
        <output class="state-readout" aria-live="polite">
          <span><small>状态</small><strong data-state-value>待机</strong></span>
          <span><small>吸附边缘</small><strong data-side-value>右侧</strong></span>
          <span><small>闲置展示</small><strong>仅用量</strong></span>
        </output>
        <div class="gesture-hint" aria-hidden="true"><span>拖动悬浮球，然后松手</span><i>↗</i></div>
      </div>
    </section>
    ${import.meta.env.DEV ? `<nav class="prototype-switcher" aria-label="原型方案切换">
      <button type="button" data-variant-direction="previous" aria-label="上一个方案">←</button>
      <div><small>方案 ${variant.key} / ${variants.length}</small><strong>${variant.name}</strong></div>
      <button type="button" data-variant-direction="next" aria-label="下一个方案">→</button>
    </nav>` : ""}
    <p class="prototype-note">拖动时 1:1 跟手；松手后根据速度预测落点。闲置约 3 秒后自动半隐藏，鼠标移入立即唤醒。</p>
  </main>`;

  controller = new BubblePrototype(root, variant);
  root.querySelectorAll<HTMLButtonElement>("[data-variant-direction]").forEach((button) => {
    button.addEventListener("click", () => cycleVariant(button.dataset.variantDirection === "next" ? 1 : -1));
  });
}

function cycleVariant(delta: number): void {
  const current = selectedVariant();
  const index = variants.findIndex((variant) => variant.key === current.key);
  const next = variants[(index + delta + variants.length) % variants.length];
  const params = new URLSearchParams(location.search);
  params.set("variant", next.key);
  history.replaceState(null, "", `${location.pathname}?${params.toString()}`);
  renderPrototype(next);
}

function isTextEntry(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || (target instanceof HTMLElement && target.isContentEditable);
}

window.addEventListener("keydown", (event) => {
  if (isTextEntry(event.target) || event.metaKey || event.ctrlKey || event.altKey) return;
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    cycleVariant(event.key === "ArrowRight" ? 1 : -1);
  }
});

class BubblePrototype {
  private readonly stage: HTMLElement;
  private readonly bubble: HTMLButtonElement;
  private readonly detail: HTMLElement;
  private readonly stateValue: HTMLElement;
  private readonly sideValue: HTMLElement;
  private readonly snapZones: Record<EdgeSide, HTMLElement>;
  private readonly reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  private readonly abortController = new AbortController();
  private readonly resizeObserver: ResizeObserver;
  private position: Point = { x: 0, y: 0 };
  private side: EdgeSide = "right";
  private state: BubbleState = "resting";
  private pointer: PointerSession | null = null;
  private animationFrame: number | null = null;
  private idleTimer: number | null = null;
  private hovering = false;
  private suppressClick = false;

  constructor(private readonly host: HTMLElement, private readonly variant: VariantDefinition) {
    this.stage = host.querySelector<HTMLElement>(".desktop-stage")!;
    this.bubble = host.querySelector<HTMLButtonElement>("[data-role='bubble']")!;
    this.detail = host.querySelector<HTMLElement>("[data-role='detail']")!;
    this.stateValue = host.querySelector<HTMLElement>("[data-state-value]")!;
    this.sideValue = host.querySelector<HTMLElement>("[data-side-value]")!;
    this.snapZones = {
      left: host.querySelector<HTMLElement>("[data-snap-zone='left']")!,
      right: host.querySelector<HTMLElement>("[data-snap-zone='right']")!,
    };
    this.resizeObserver = new ResizeObserver(() => this.reset(false));
    this.bindEvents();
    this.resizeObserver.observe(this.stage);
    requestAnimationFrame(() => this.reset(false));
  }

  destroy(): void {
    this.abortController.abort();
    this.resizeObserver.disconnect();
    this.stopAnimation();
    this.clearIdleTimer();
  }

  private bindEvents(): void {
    const signal = this.abortController.signal;
    this.bubble.addEventListener("pointerenter", () => {
      this.hovering = true;
      this.clearIdleTimer();
      if (this.state === "idle") this.reveal();
    }, { signal });
    this.bubble.addEventListener("pointerleave", () => {
      this.hovering = false;
      this.scheduleIdle();
    }, { signal });
    this.bubble.addEventListener("pointerdown", (event) => this.onPointerDown(event), { signal });
    this.bubble.addEventListener("pointermove", (event) => this.onPointerMove(event), { signal });
    this.bubble.addEventListener("pointerup", (event) => this.onPointerUp(event), { signal });
    this.bubble.addEventListener("pointercancel", () => this.cancelPointer(), { signal });
    this.bubble.addEventListener("click", (event) => {
      if (this.suppressClick) {
        this.suppressClick = false;
        event.preventDefault();
        return;
      }
      this.toggleExpanded();
    }, { signal });
    window.addEventListener("pointermove", (event) => {
      if (!this.pointer || event.composedPath().includes(this.bubble)) return;
      this.onPointerMove(event);
    }, { signal });
    window.addEventListener("pointerup", (event) => this.onPointerUp(event), { signal });
    window.addEventListener("pointercancel", () => this.cancelPointer(), { signal });
    window.addEventListener("keydown", (event) => {
      if (event.key !== "Escape" || this.state !== "expanded") return;
      event.preventDefault();
      this.closeExpanded();
      this.bubble.focus();
    }, { signal });
    this.stage.addEventListener("pointerdown", (event) => {
      if (this.state !== "expanded") return;
      const target = event.target instanceof Element ? event.target : null;
      if (!target?.closest(".floating-bubble, .bubble-detail")) this.closeExpanded();
    }, { signal });
    this.host.querySelector<HTMLButtonElement>("[data-control='idle']")?.addEventListener("click", () => this.simulateIdle(), { signal });
    this.host.querySelector<HTMLButtonElement>("[data-control='side']")?.addEventListener("click", () => this.switchSide(), { signal });
    this.host.querySelector<HTMLButtonElement>("[data-control='reset']")?.addEventListener("click", () => this.reset(), { signal });
    this.host.querySelectorAll<HTMLButtonElement>(".prototype-action").forEach((button) => {
      button.addEventListener("click", () => {
        button.classList.remove("is-confirmed");
        requestAnimationFrame(() => button.classList.add("is-confirmed"));
      }, { signal });
    });
  }

  private onPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    this.clearIdleTimer();
    this.stopAnimation();
    const startedExpanded = this.state === "expanded";
    if (this.state === "idle") this.setState("resting");
    if (!startedExpanded) this.setState("pressing");
    this.pointer = {
      id: event.pointerId,
      startPointer: { x: event.clientX, y: event.clientY },
      startPosition: { ...this.position },
      startedExpanded,
      dragged: false,
      samples: [{ x: event.clientX, y: event.clientY, time: performance.now() }],
    };
    this.bubble.setPointerCapture(event.pointerId);
  }

  private onPointerMove(event: PointerEvent): void {
    if (!this.pointer || event.pointerId !== this.pointer.id) return;
    const dx = event.clientX - this.pointer.startPointer.x;
    const dy = event.clientY - this.pointer.startPointer.y;
    if (!this.pointer.dragged && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
    if (!this.pointer.dragged) {
      this.pointer.dragged = true;
      if (this.pointer.startedExpanded) this.closeExpanded(false);
      this.setState("dragging");
    }
    const now = performance.now();
    this.pointer.samples.push({ x: event.clientX, y: event.clientY, time: now });
    this.pointer.samples = this.pointer.samples.filter((sample) => now - sample.time <= 120);
    const bounds = this.dragBounds();
    const rawX = this.pointer.startPosition.x + dx;
    const rawY = this.pointer.startPosition.y + dy;
    this.position = {
      x: this.rubberBand(rawX, bounds.minX, bounds.maxX, this.stage.clientWidth),
      y: this.rubberBand(rawY, bounds.minY, bounds.maxY, this.stage.clientHeight),
    };
    this.applyPosition();
    const nearest: EdgeSide = this.position.x + BUBBLE_SIZE / 2 < this.stage.clientWidth / 2 ? "left" : "right";
    this.showSnapZone(nearest);
    event.preventDefault();
  }

  private onPointerUp(event: PointerEvent): void {
    if (!this.pointer || event.pointerId !== this.pointer.id) return;
    const session = this.pointer;
    this.pointer = null;
    if (this.bubble.hasPointerCapture(event.pointerId)) this.bubble.releasePointerCapture(event.pointerId);
    if (!session.dragged) return;
    this.suppressClick = true;
    window.setTimeout(() => { this.suppressClick = false; }, 0);
    const velocity = this.pointerVelocity(session.samples);
    const projectedX = this.position.x + BUBBLE_SIZE / 2 + this.project(velocity.x);
    const projectedY = this.position.y + this.project(velocity.y);
    this.side = projectedX < this.stage.clientWidth / 2 ? "left" : "right";
    const target = this.fullTarget(this.side, projectedY);
    this.setSide(this.side);
    this.setState("snapping");
    this.showSnapZone(this.side);
    this.animateTo(target, velocity, () => {
      this.hideSnapZones();
      this.setState("resting");
      this.scheduleIdle();
    });
  }

  private cancelPointer(): void {
    this.pointer = null;
    this.hideSnapZones();
    this.setState("snapping");
    this.animateTo(this.fullTarget(this.side, this.position.y), { x: 0, y: 0 }, () => {
      this.setState("resting");
      this.scheduleIdle();
    });
  }

  private pointerVelocity(samples: PointerSample[]): Point {
    if (samples.length < 2) return { x: 0, y: 0 };
    const last = samples[samples.length - 1];
    const first = samples.find((sample) => last.time - sample.time <= 80) ?? samples[0];
    const seconds = Math.max((last.time - first.time) / 1000, 0.016);
    return { x: (last.x - first.x) / seconds, y: (last.y - first.y) / seconds };
  }

  private project(velocity: number, decelerationRate = 0.99): number {
    return (velocity / 1000) * decelerationRate / (1 - decelerationRate);
  }

  private rubberBand(value: number, min: number, max: number, dimension: number): number {
    if (value >= min && value <= max) return value;
    const edge = value < min ? min : max;
    const overshoot = value - edge;
    return edge + (overshoot * dimension * 0.32) / (dimension + 0.32 * Math.abs(overshoot));
  }

  private dragBounds(): { minX: number; maxX: number; minY: number; maxY: number } {
    return {
      minX: -BUBBLE_SIZE * 0.35,
      maxX: this.stage.clientWidth - BUBBLE_SIZE * 0.65,
      minY: SAFE_Y,
      maxY: Math.max(SAFE_Y, this.stage.clientHeight - BUBBLE_SIZE - SAFE_Y),
    };
  }

  private fullTarget(side: EdgeSide, suggestedY: number): Point {
    return {
      x: side === "left" ? SNAP_INSET : this.stage.clientWidth - BUBBLE_SIZE - SNAP_INSET,
      y: this.clamp(suggestedY, SAFE_Y, Math.max(SAFE_Y, this.stage.clientHeight - BUBBLE_SIZE - SAFE_Y)),
    };
  }

  private idleTarget(): Point {
    return {
      x: this.side === "left" ? -(BUBBLE_SIZE - this.variant.visibleWidth) : this.stage.clientWidth - this.variant.visibleWidth,
      y: this.position.y,
    };
  }

  private animateTo(target: Point, initialVelocity: Point, onComplete?: () => void): void {
    this.stopAnimation();
    if (this.reducedMotion) {
      this.position = target;
      this.applyPosition();
      onComplete?.();
      return;
    }
    let velocityX = initialVelocity.x;
    let velocityY = initialVelocity.y;
    let previous = performance.now();
    const stiffness = 250;
    const damping = 30;
    const step = (now: number) => {
      const dt = Math.min((now - previous) / 1000, 1 / 30);
      previous = now;
      velocityX += (-stiffness * (this.position.x - target.x) - damping * velocityX) * dt;
      velocityY += (-stiffness * (this.position.y - target.y) - damping * velocityY) * dt;
      this.position.x += velocityX * dt;
      this.position.y += velocityY * dt;
      this.applyPosition();
      const settled = Math.hypot(this.position.x - target.x, this.position.y - target.y) < 0.35
        && Math.hypot(velocityX, velocityY) < 4;
      if (settled) {
        this.position = target;
        this.applyPosition();
        this.animationFrame = null;
        onComplete?.();
        return;
      }
      this.animationFrame = requestAnimationFrame(step);
    };
    this.animationFrame = requestAnimationFrame(step);
  }

  private stopAnimation(): void {
    if (this.animationFrame === null) return;
    cancelAnimationFrame(this.animationFrame);
    this.animationFrame = null;
  }

  private applyPosition(): void {
    this.bubble.style.transform = `translate3d(${this.position.x}px, ${this.position.y}px, 0)`;
    this.updateDetailPosition();
  }

  private updateDetailPosition(): void {
    const x = this.side === "left"
      ? this.position.x + BUBBLE_SIZE + 10
      : this.position.x - this.variant.panelWidth - 10;
    const y = this.clamp(
      this.position.y + BUBBLE_SIZE / 2 - this.variant.panelHeight / 2,
      14,
      Math.max(14, this.stage.clientHeight - this.variant.panelHeight - 14),
    );
    this.detail.style.transform = `translate3d(${x}px, ${y}px, 0)`;
  }

  private toggleExpanded(): void {
    if (this.state === "expanded") {
      this.closeExpanded();
      return;
    }
    this.clearIdleTimer();
    this.setState("expanded");
    this.animateTo(this.fullTarget(this.side, this.position.y), { x: 0, y: 0 });
  }

  private closeExpanded(animate = true): void {
    this.setState("resting");
    if (animate) this.animateTo(this.fullTarget(this.side, this.position.y), { x: 0, y: 0 }, () => this.scheduleIdle());
  }

  private reveal(): void {
    this.clearIdleTimer();
    this.setState("resting");
    this.animateTo(this.fullTarget(this.side, this.position.y), { x: 0, y: 0 }, () => this.scheduleIdle());
  }

  private simulateIdle(): void {
    this.clearIdleTimer();
    this.stopAnimation();
    if (this.state === "expanded") this.closeExpanded(false);
    const full = this.fullTarget(this.side, this.position.y);
    this.setState("snapping");
    this.animateTo(full, { x: 0, y: 0 }, () => this.enterIdle());
  }

  private enterIdle(): void {
    if (this.hovering || this.pointer || this.state === "expanded" || this.state === "dragging") {
      this.scheduleIdle();
      return;
    }
    this.setState("idle");
    this.animateTo(this.idleTarget(), { x: 0, y: 0 });
  }

  private switchSide(): void {
    this.clearIdleTimer();
    this.side = this.side === "left" ? "right" : "left";
    this.setSide(this.side);
    this.setState("snapping");
    this.showSnapZone(this.side);
    this.animateTo(this.fullTarget(this.side, this.position.y), { x: this.side === "left" ? -480 : 480, y: 0 }, () => {
      this.hideSnapZones();
      this.setState("resting");
      this.scheduleIdle();
    });
  }

  private reset(scheduleIdle = true): void {
    this.clearIdleTimer();
    this.stopAnimation();
    this.pointer = null;
    this.side = "right";
    this.setSide("right");
    this.setState("resting");
    this.position = this.fullTarget("right", this.stage.clientHeight * 0.48 - BUBBLE_SIZE / 2);
    this.applyPosition();
    this.hideSnapZones();
    if (scheduleIdle) this.scheduleIdle();
    else window.setTimeout(() => this.scheduleIdle(), 400);
  }

  private scheduleIdle(): void {
    this.clearIdleTimer();
    if (this.hovering || this.pointer || this.state === "expanded" || this.state === "dragging") return;
    this.idleTimer = window.setTimeout(() => {
      this.idleTimer = null;
      this.enterIdle();
    }, IDLE_DELAY_MS);
  }

  private clearIdleTimer(): void {
    if (this.idleTimer === null) return;
    window.clearTimeout(this.idleTimer);
    this.idleTimer = null;
  }

  private showSnapZone(side: EdgeSide): void {
    this.snapZones.left.classList.toggle("is-active", side === "left");
    this.snapZones.right.classList.toggle("is-active", side === "right");
  }

  private hideSnapZones(): void {
    this.snapZones.left.classList.remove("is-active");
    this.snapZones.right.classList.remove("is-active");
  }

  private setSide(side: EdgeSide): void {
    this.side = side;
    this.stage.dataset.side = side;
    this.bubble.dataset.side = side;
    this.detail.dataset.side = side;
    this.sideValue.textContent = side === "left" ? "左侧" : "右侧";
    this.updateDetailPosition();
  }

  private setState(state: BubbleState): void {
    this.state = state;
    this.stage.dataset.interactionState = state;
    this.bubble.dataset.state = state;
    this.detail.classList.toggle("is-open", state === "expanded");
    this.detail.inert = state !== "expanded";
    this.detail.setAttribute("aria-hidden", String(state !== "expanded"));
    this.bubble.setAttribute("aria-expanded", String(state === "expanded"));
    const labels: Record<BubbleState, string> = {
      resting: "待机",
      pressing: "按下",
      dragging: "拖拽中",
      snapping: "吸附中",
      idle: "闲置半隐",
      expanded: "已展开",
    };
    this.stateValue.textContent = labels[state];
  }

  private clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(value, max));
  }
}

renderPrototype(selectedVariant());
