import './components';
import Sortable from 'sortablejs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { html, nothing, render as patch, type TemplateResult } from 'lit';
import { live } from 'lit/directives/live.js';
import { repeat } from 'lit/directives/repeat.js';
import { keyed } from 'lit/directives/keyed.js';
import type WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type WaSwitch from '@awesome.me/webawesome/dist/components/switch/switch.js';
import type WaCheckbox from '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import type { Change, Payload, Provider, Settings } from './types';
import appIcon from '../assets/icons/icon.svg?url';
import './style.css';

const root = document.querySelector<HTMLElement>('#app')!;
const notice = document.querySelector<HTMLElement>('#notice')!;
const bubble = new URLSearchParams(location.search).get('view') === 'bubble';
document.body.className = bubble ? 'bubble-window' : 'panel-window';
document.documentElement.classList.add('wa-dark');
let state: Payload;
let busy = false;
const pending = new Set<string>();
const progressSeen = new Set<string>();
const drafts = new Map<Provider, string>();
let noticeTimer: ReturnType<typeof setTimeout>;
let hoverTimer: ReturnType<typeof setTimeout>;
let providerSort: Sortable | null = null;
let orderDrag: { nodes: ChildNode[]; order: Provider[]; provider: Provider; started: boolean; cancelled: boolean } | null = null;
let pointer: { x: number; y: number; dragging: boolean; reset: boolean } | null = null;
let hoverChanges = 0;
let dragFinishedAt = -Infinity;
let now = Date.now();
const names: Record<Provider, string> = { cursor: 'Cursor', codex: 'Codex', claude: 'Claude Code' };
const t = (key: string, args: Record<string, string | number> = {}): string => Object.entries(args).reduce((text, [key, value]) => text.replaceAll(`{${key}}`, String(value)), state.translations[key] ?? key);
const color = (p: Provider) => state.settings[`${p}BubbleColor`];
const label = (p: Provider) => state.settings[`${p}BubbleLabel`];
const date = (value: string) => new Date(value).toLocaleString(state.locale, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
const compact = (value: number | null) => value == null ? '—' : new Intl.NumberFormat(state.locale, { notation: 'compact', maximumFractionDigits: 1 }).format(value);
const money = (value: number | null | undefined, currency: string) => value == null ? '—' : new Intl.NumberFormat(state.locale, { style: 'currency', currency }).format(value / 100);
const icon = (name: string, text = '') => html`<wa-icon library="lucide" name=${name} label=${text} aria-hidden=${text ? 'false' : 'true'}></wa-icon>`;
const actionLoading = (action: string) => pending.has(action) || (action === 'refresh' && state.snapshot.refreshing);
const spinner = () => html`<wa-spinner aria-hidden="true"></wa-spinner>`;
const isRefreshing = (p: Provider) => state.refreshingProviders.includes(p) || (pending.has('refresh') && !progressSeen.has('refresh')) || (p === 'cursor' && pending.has('recheck') && !progressSeen.has('recheck'));

function button(action: string, text: string, options: { variant?: 'neutral' | 'brand' | 'danger'; disabled?: boolean } = {}) {
  return html`<wa-button size="s" data-action=${action} variant=${options.variant ?? 'neutral'}
    appearance=${options.variant === 'brand' ? 'accent' : 'outlined'}
    ?disabled=${busy || options.disabled || actionLoading(action)}
    ?loading=${actionLoading(action)} @click=${() => perform(action)}>${text}</wa-button>`;
}
function iconButton(action: string, text: string, glyph: string, disabled = false) {
  return html`<wa-button class="icon-button" size="s" appearance="plain" data-action=${action}
    title=${text} ?disabled=${busy || disabled || actionLoading(action)}
    ?loading=${actionLoading(action)} @click=${() => perform(action)}>${icon(glyph, text)}</wa-button>`;
}
function notify(message: string) {
  clearTimeout(noticeTimer);
  patch(html`<wa-callout variant="brand" size="s">${message}</wa-callout>`, notice);
  notice.classList.add('visible');
  noticeTimer = setTimeout(() => notice.classList.remove('visible'), 6000);
}
async function run(action: () => Promise<unknown>) {
  if (busy) return;
  busy = true; render();
  try { await action(); } catch (error) { notify(String(error)); }
  finally { busy = false; render(); }
}
async function runTask(action: string, task: () => Promise<unknown>) {
  if (pending.has(action)) return;
  progressSeen.delete(action);
  pending.add(action); render();
  try { await task(); } catch (error) { notify(String(error)); }
  finally { pending.delete(action); render(); }
}
async function change(change: Change) {
  await invoke('change_setting', { change });
  state = await invoke<Payload>('get_state');
}
async function navigate(mode: Payload['panel']) {
  await invoke('open_panel', { mode, toggle: false });
  state.panel = mode;
}
function perform(action: string) {
  if (busy) return;
  if (['refresh', 'recheck', 'login', 'check-update', 'install'].includes(action)) {
    void runTask(action, async () => {
      if (action === 'refresh' || action === 'recheck') await invoke('refresh', { recheck: action === 'recheck' });
      else if (action === 'login') await invoke('login_cursor');
      else if (action === 'install') await invoke('install_update');
      else { await invoke('check_update'); state = await invoke<Payload>('get_state'); if (!state.update.version) notify(t('native.upToDate')); }
    });
    return;
  }
  void run(async () => {
    if (['details', 'settings', 'consent'].includes(action)) await navigate(action as Payload['panel']);
    else if (action === 'close') await invoke('hide_panel');
    else if (action === 'quit') await invoke('quit');
    else if (action === 'enable-compat') { await change({ kind: 'compat', value: true }); await navigate('details'); }

  });
}
const metric = (name: string, value: string) => html`<div><dt>${name}</dt><dd>${value}</dd></div>`;
function providerCard(p: Provider) {
  const s = state.snapshot[p];
  const loading = isRefreshing(p);
  const statusKeys: Record<string, string> = { available: 'available', desktop_installed: 'desktopInstalled', not_installed: 'notInstalled', not_logged_in: 'notLoggedIn', unsupported: 'unsupported', network_error: 'networkError', protocol_error: 'protocolError' };
  const metrics: TemplateResult[] = [];
  if (s.tokens) {
    metrics.push(metric(t(p === 'claude' && s.cost?.todayUsedCents != null ? 'provider.apiTodayTokensUtc' : 'provider.localTodayTokens'), compact(s.tokens.today)));
    metrics.push(metric(t(p === 'codex' ? 'provider.officialLifetimeTokens' : 'provider.localLifetimeTokens'), compact(s.tokens.lifetime)));
  }
  if (s.cost) {
    const c = s.cost;
    if (c.includedUsedCents != null || c.includedLimitCents != null) metrics.push(metric(t('native.included'), `${money(c.includedUsedCents, c.currency)} / ${money(c.includedLimitCents, c.currency)}`));
    if (c.onDemandEnabled === false) metrics.push(metric(t('native.ondemand'), t('common.disabled')));
    else if (c.onDemandUsedCents != null) metrics.push(metric(t('native.ondemand'), `${money(c.onDemandUsedCents, c.currency)} / ${money(c.onDemandLimitCents, c.currency)}`));
    if (c.todayUsedCents != null) metrics.push(metric(t('provider.apiEstimatedCostUtc'), money(c.todayUsedCents, c.currency)));
  }
  return html`<article class="provider-card" style=${`--accent:${color(p)}`} data-provider=${p} aria-busy=${String(loading)}>
    <header class="line"><h2><span class="provider-mark">${label(p)}</span>${names[p]}</h2>
      <span class=${`status ${s.stale && !loading ? 'stale' : ''}`}>
        ${loading ? html`${spinner()}<span>${t('refresh.refreshing')}</span>` : t(s.stale ? 'status.stale' : `status.${statusKeys[s.status] ?? 'unsupported'}`)}
      </span></header>
    ${loading ? html`<wa-progress-bar class="detecting" indeterminate label=${t('provider.connectingTitle', { provider: names[p] })}></wa-progress-bar>` : nothing}
    <p class="meta">${s.plan ?? t('plan.unknown')}<span>${date(s.capturedAt)}</span></p>
    ${s.quotas.length ? s.quotas.map(q => {
      const minutes = q.resetsAt ? Math.max(0, Math.ceil((new Date(q.resetsAt).getTime() - now) / 60000)) : 0;
      return html`<section class="quota"><div class="line"><span>${q.label}</span><strong>${t('quota.remaining', { value: Math.round(q.remainingPercent) })}</strong></div>
        <wa-progress-bar value=${q.remainingPercent} label=${q.label}></wa-progress-bar>
        <div class="quota-meta"><span>${t('quota.used', { value: Math.round(q.usedPercent) })}</span>
          ${q.resetsAt ? html`<span>${t('quota.resetsAt', { date: `${date(q.resetsAt)} · ${Math.floor(minutes / 60)}h ${minutes % 60}m` })}</span>` : nothing}
        </div></section>`;
    }) : html`<p class="empty">${loading ? t('provider.connectingDescription') : s.message ?? t('quota.noData')}</p>`}
    ${metrics.length ? html`<dl class="metrics">${metrics}</dl>` : nothing}
    ${s.quotas.length && s.message ? html`<p class=${`hint ${s.stale ? 'stale' : ''}`}>${s.message}</p>` : nothing}
    ${p === 'cursor' ? html`<div class="actions">
      ${['not_logged_in', 'desktop_installed'].includes(s.status) ? button('login', t(state.loginRunning ? 'cursor.opening' : 'cursor.login'), { disabled: state.loginRunning }) : nothing}
      ${!state.settings.cursorCompatEnabled ? button('consent', t('cursor.readExactUsage')) : nothing}
    </div>` : nothing}
  </article>`;
}
function updateBanner() {
  const update = state.update;
  return html`${update.version ? html`<aside class="update"><strong>${t('update.availableTitle', { version: update.version })}</strong>
    <p>${t('update.versionPath', { current: state.version, next: update.version })}</p>
    ${update.notes ? html`<wa-details appearance="plain" summary=${t('common.operation')}><p>${update.notes}</p></wa-details>` : nothing}
    ${button('install', t(update.downloading ? 'update.starting' : state.canInstall ? 'native.installUpdate' : 'native.openRelease'), { disabled: update.downloading })}
    ${update.error ? html`<p class="stale">${update.error}</p>` : nothing}</aside>` : nothing}
`;
}
function details() {
  return html`${repeat(state.settings.bubbleProviderOrder, p => p, providerCard)}
    <footer class="actions">${button('recheck', t('native.recheck'))}${button('check-update', t('native.checkUpdate'))}</footer>`;
}
function select(key: string, kind: 'language' | 'display' | 'percent' | 'refresh', current: string | number, values: [string | number, string][]) {
  return html`<div class="setting"><span>${t(key)}</span><wa-select class="setting-select" size="s" label=${t(key)}
    data-setting=${kind} .value=${live(String(current))} ?disabled=${busy}
    @change=${(event: Event) => {
      const value = String((event.currentTarget as WaSelect).value);
      void run(() => change({ kind, value: kind === 'refresh' ? Number(value) : value } as Change));
    }}>${values.map(([value, text]) => html`<wa-option value=${String(value)}>${text}</wa-option>`)}</wa-select></div>`;
}
function toggle(key: string, kind: 'snap' | 'autostart' | 'compat', enabled: boolean, hint?: string) {
  return html`<div class="setting"><wa-switch size="s" data-setting=${kind} .checked=${live(enabled)} ?disabled=${busy}
    @change=${(event: Event) => {
      const checked = (event.currentTarget as WaSwitch).checked;
      void run(() => kind === 'compat' && checked ? navigate('consent') : change({ kind, value: checked }));
    }}><span>${t(key)}${hint ? html`<small>${t(hint)}</small>` : nothing}</span></wa-switch></div>`;
}
function providerControls() {
  return html`<section class="display-config"><h2>${t('config.title')}</h2><p class="hint" id="order-help">${t('config.reorderHint')}</p>
    <ol class="provider-list">${repeat(state.settings.bubbleProviderOrder, p => p, p => {
      return html`<li data-order-provider=${p} style=${`--accent:${color(p)}`}>
        <div class="provider-config-title"><div class="provider-name">
          <wa-button class="order-handle" size="s" appearance="plain" title=${t('config.dragProviderOrder', { provider: names[p] })}
            aria-label=${t('config.dragProviderOrder', { provider: names[p] })} aria-describedby="order-help" ?disabled=${busy}
            @keydown=${(event: KeyboardEvent) => {
              const direction = event.key === 'ArrowUp' ? -1 : event.key === 'ArrowDown' ? 1 : 0;
              if (!direction || busy || orderDrag) return;
              event.preventDefault(); event.stopPropagation();
              const order = [...state.settings.bubbleProviderOrder]; const from = order.indexOf(p); const to = from + direction;
              if (to >= 0 && to < order.length) { [order[from], order[to]] = [order[to], order[from]]; saveProviderOrder(order, p); }
            }}>${icon('grip')}</wa-button><strong>${names[p]}</strong></div><div class="order-actions">
          <wa-checkbox size="s" data-provider-visible=${p} .checked=${live(state.settings.bubbleVisibleProviders.includes(p))} ?disabled=${busy}
            @change=${(event: Event) => { const visible = (event.currentTarget as WaCheckbox).checked; void run(() => change({ kind: 'visible', value: { provider: p, visible } })); }}>
            <span class="sr-only">${t('config.providerVisible', { provider: names[p] })}</span></wa-checkbox>
        </div></div>
        <div class="provider-config-fields"><wa-input class="label-input" size="s" data-provider-label=${p}
          label=${t('config.bubbleLabel', { provider: names[p] })} .value=${live(drafts.get(p) ?? label(p))} maxlength="6" ?disabled=${busy}
          @input=${(event: Event) => drafts.set(p, ((event.currentTarget as WaInput).value ?? ''))}
          @change=${(event: Event) => {
            const value = (event.currentTarget as WaInput).value ?? '';
            void run(async () => { await change({ kind: 'label', value: { provider: p, label: value } }); drafts.delete(p); });
          }}
          @keydown=${(event: KeyboardEvent) => { if (event.key === 'Enter') (event.currentTarget as WaInput).blur(); }}></wa-input>
          <wa-button class="color-trigger" size="s" appearance="outlined" id=${`color-${p}`} ?disabled=${busy}>
            <span class="color-dot" slot="start"></span><span class="sr-only">${t('config.chooseColor', { provider: names[p] })}</span><span aria-hidden="true">${color(p)}</span></wa-button>
          <wa-popover class="color-picker" for=${`color-${p}`} placement="top-end" without-arrow>
            <div class="palette">${state.palette.map(c => html`<wa-button size="s" class="swatch" data-color=${c} data-color-provider=${p}
              style=${`--swatch:${c}`}
              aria-pressed=${String(c === color(p))} data-popover="close" ?disabled=${busy}
              @click=${() => void run(() => change({ kind: 'color', value: { provider: p, color: c } }))}><span class="sr-only">${t('config.chooseCurrentColor', { provider: names[p], color: c })}</span></wa-button>`)}</div>
          </wa-popover>
        </div></li>`;
    })}</ol></section>`;
}
function settings() {
  const s = state.settings;
  return html`<section class="settings-group">
    ${select('menu.language', 'language', s.uiLanguage, [['system', t('menu.languageSystem')], ['zh-CN', '简体中文'], ['en', 'English'], ['ja', '日本語'], ['ko', '한국어']])}
    ${select('menu.displayMode', 'display', s.displayMode, [['bubble', t('menu.displayBubble')], ['statusBar', t('menu.displayStatusBar')]])}
    ${select('menu.bubblePercentage', 'percent', s.bubblePercentMode, [['remaining', t('menu.remaining')], ['used', t('menu.used')]])}
    ${select('menu.refreshInterval', 'refresh', s.refreshMinutes, [1,5,15,30,60].map(n => [n, `${n} min`]))}
    ${toggle('menu.autoSnap', 'snap', s.bubbleSnapEnabled, 'menu.autoSnapHint')}
    ${toggle('menu.autostart', 'autostart', s.autostart)}
    ${toggle('menu.cursorCompat', 'compat', s.cursorCompatEnabled, 'menu.cursorCompatHint')}
  </section>${providerControls()}<footer class="actions">${button('refresh', t('menu.rescanCli'))}${button('quit', t('menu.quit'), { variant: 'danger' })}</footer>`;
}
function consent() {
  return html`<section class="consent"><h2>${t('consent.cursorTitle')}</h2><p>${t('consent.cursorReadOnly')}</p><p>${t('consent.cursorMemoryOnly')}</p>
    <div class="actions">${button('details', t('common.cancel'))}${button('enable-compat', t('consent.enable'), { variant: 'brand' })}</div></section>`;
}
function renderBubble() {
  const usage = state.entries.map(e => `${e.label} ${e.value == null ? '—' : `${e.value}%`}`).join(t('bubble.usageSeparator'));
  return html`<button type="button" id="bubble" class=${`bubble ${state.peek ? 'peek' : ''}`}
    aria-label=${t('bubble.ariaLabel', { usage, dragHint: t(state.settings.bubbleSnapEnabled ? 'bubble.snapHint' : 'bubble.freeHint') })}
    title=${usage} @click=${() => {
      if (performance.now() - dragFinishedAt > 300 && !pointer?.dragging) void invoke('open_panel', { mode: 'details', toggle: true }).catch(e => notify(String(e)));
    }}><span class="bubble-inner">${repeat(state.entries, e => e.provider, e => {
      const remaining = e.value == null ? 100 : state.settings.bubblePercentMode === 'used' ? 100 - e.value : e.value;
      return html`<span class=${`bubble-entry ${e.stale ? 'is-stale' : ''}`} style=${`--accent:${e.color};--value:${remaining <= 10 ? '#ff667d' : remaining <= 25 ? '#ffd15a' : e.color}`}>
        <span class="bubble-label">${e.label}</span><strong>${e.value == null ? '—' : `${e.value}%`}</strong></span>`;
    })}</span></button>`;
}
function render() {
  if (!state || orderDrag) return;
  document.documentElement.lang = state.locale;
  root.setAttribute('aria-busy', String(busy));
  if (bubble) { patch(renderBubble(), root); return; }
  const refreshing = state.snapshot.refreshing || pending.has('refresh') || pending.has('recheck');
  patch(html`<div class="panel"><header class="panel-header"><div class="brand"><img src=${appIcon} alt="" width="30" height="30"><div><div class="brand-title"><h1>Metra</h1><span class="version" title=${`Metra ${state.version}`}>v${state.version}</span></div><p>${t(state.panel === 'settings' ? 'native.settings' : 'app.usageSubtitle')}</p></div></div><nav>
    ${state.panel !== 'details' ? iconButton('details', t('native.details'), 'back') : iconButton('refresh', t('refresh.nowUsage'), 'refresh')}
    ${state.panel !== 'settings' ? iconButton('settings', t('native.settings'), 'settings') : nothing}
    ${iconButton('close', t('native.close'), 'close')}
  </nav></header>${state.panel === 'details' ? updateBanner() : nothing}<div class="scroll">${keyed(state.panel, state.panel === 'details' ? details() : state.panel === 'settings' ? settings() : consent())}</div>
  <div class="panel-status"><span class="refresh-status">${refreshing || busy ? spinner() : html`<span class="status-dot"></span>`}${t(refreshing ? 'native.refreshing' : 'refresh.everyMinutes', { minutes: state.settings.refreshMinutes })}</span></div></div>`, root);
  syncProviderSorting();
}
function saveProviderOrder(order: Provider[], provider: Provider, previous = [...state.settings.bubbleProviderOrder]) {
  if (order.every((p, i) => p === previous[i])) { render(); return; }
  state.settings.bubbleProviderOrder = order;
  void run(async () => {
    try { await change({ kind: 'order', value: order }); }
    catch (error) { state.settings.bubbleProviderOrder = previous; throw error; }
  }).finally(() => root.querySelector<HTMLElement>(`[data-order-provider="${provider}"] .order-handle`)?.focus({ preventScroll: true }));
}
function finishProviderDrag() {
  const drag = orderDrag; if (!drag || !providerSort) return;
  const order = providerSort.toArray() as Provider[];
  // Restore Lit's comment anchors and original nodes before its keyed update.
  providerSort.el.replaceChildren(...drag.nodes);
  orderDrag = null;
  document.body.classList.remove('ordering-providers');
  if (drag.started && !drag.cancelled) saveProviderOrder(order, drag.provider, drag.order); else { render(); root.querySelector<HTMLElement>(`[data-order-provider="${drag.provider}"] .order-handle`)?.focus({ preventScroll: true }); }
}
function cancelProviderDrag() {
  if (!orderDrag) return;
  orderDrag.cancelled = true;
  document.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
}
function syncProviderSorting() {
  const list = root.querySelector<HTMLElement>('.provider-list');
  if (providerSort && providerSort.el !== list) { providerSort.destroy(); providerSort = null; }
  if (!list) return;
  if (providerSort) { providerSort.option('disabled', busy); providerSort.option('animation', matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 160); return; }
  providerSort = new Sortable(list, {
    handle: '.order-handle', draggable: '[data-order-provider]', dataIdAttr: 'data-order-provider',
    forceFallback: true, fallbackOnBody: true, fallbackTolerance: 4,
    animation: matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 160,
    easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)', direction: 'vertical', swapThreshold: 1,
    fallbackClass: 'order-drag-preview', ghostClass: 'order-placeholder', chosenClass: 'order-chosen',
    onChoose: event => { orderDrag = { provider: event.item.dataset.orderProvider as Provider, nodes: [...list.childNodes], order: [...state.settings.bubbleProviderOrder], started: false, cancelled: false }; },
    onStart: () => {
      if (!orderDrag) return; orderDrag.started = true;
      document.body.classList.add('ordering-providers');
      const preview = Sortable.ghost;
      if (preview) { preview.inert = true; preview.setAttribute('aria-hidden', 'true'); }
      const header = preview?.querySelector('.provider-config-title');
      if (preview && header) { header.querySelector('.order-actions')?.remove(); preview.replaceChildren(header); }
    },
    onUnchoose: () => { if (orderDrag && !orderDrag.started) finishProviderDrag(); },
    onEnd: finishProviderDrag,
  });
}
window.addEventListener('blur', cancelProviderDrag);
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && orderDrag) { event.preventDefault(); event.stopPropagation(); cancelProviderDrag(); return; }
  if (event.key === 'Escape' && !bubble && !event.defaultPrevented && !root.querySelector('wa-popover[open],wa-select[open]')) void invoke('hide_panel');
});
if (bubble) {
  function hover(hovered: boolean) {
    hoverChanges++;
    void invoke('bubble_hover', { hovered }).catch(e => notify(String(e))).finally(() => {
      hoverChanges--;
      // Expansion can shift the native window origin beneath a held pointer.
      if (pointer && !pointer.dragging) pointer.reset = true;
    });
  }
  root.addEventListener('pointerenter', () => {
    clearTimeout(hoverTimer);
    hover(true);
  });
  root.addEventListener('pointerleave', () => {
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => { if (!pointer?.dragging) hover(false); }, 500);
  });
  // Only detect motion before starting the native drag; global screen coordinates
  // are unavailable in some WebKitGTK environments.
  root.addEventListener('pointerdown', event => { if (event.button === 0) pointer = { x: event.clientX, y: event.clientY, dragging: false, reset: false }; });
  root.addEventListener('pointermove', event => {
    if (!pointer || pointer.dragging || hoverChanges) return;
    if (pointer.reset) {
      pointer.x = event.clientX; pointer.y = event.clientY; pointer.reset = false; return;
    }
    if (Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) < 4) return;
    pointer.dragging = true; clearTimeout(hoverTimer);
    void invoke('drag_bubble').catch(e => notify(String(e))).finally(() => { pointer = null; dragFinishedAt = performance.now(); });
  });
  window.addEventListener('pointerup', () => { if (!pointer?.dragging) pointer = null; });
  root.addEventListener('contextmenu', event => { event.preventDefault(); void invoke('open_panel', { mode: 'settings', toggle: false }); });
}
async function start() {
  try {
    await listen<Payload>('metra-state', event => {
      state = event.payload;
      if (state.snapshot.refreshing || state.refreshingProviders.length) {
        if (pending.has('refresh')) progressSeen.add('refresh');
        if (pending.has('recheck')) progressSeen.add('recheck');
      }
      render();
    });
    await listen<string>('metra-error', event => notify(event.payload));
    state = await invoke<Payload>('get_state'); render();
  } catch (error) {
    patch(html`<section class="startup-error"><h1>Metra</h1><p>${String(error)}</p><wa-button @click=${() => location.reload()}>Retry / 重试</wa-button></section>`, root);
  }
}
void start();
setInterval(() => { now = Date.now(); render(); }, 30000);
