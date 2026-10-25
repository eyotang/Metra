import { test, expect, type Page } from '@playwright/test';
import en from '../../assets/locales/en.json' with { type: 'json' };
import zh from '../../assets/locales/zh-CN.json' with { type: 'json' };
import ja from '../../assets/locales/ja.json' with { type: 'json' };
import ko from '../../assets/locales/ko.json' with { type: 'json' };
import type { Payload } from '../../frontend/types';
const fixture: Payload = {
  refreshingProviders: [],
  snapshot: {
    refreshing: false,
    cursor: { provider: 'cursor', status: 'available', plan: 'Pro', capturedAt: '2026-10-25T06:00:00Z', quotas: [{ label: 'Cursor models', usedPercent: 23, remainingPercent: 77, resetsAt: '2030-10-26T06:00:00Z' }], tokens: null, cost: { currency: 'USD', includedUsedCents: 2300, includedLimitCents: 10000, onDemandUsedCents: null, onDemandLimitCents: null, onDemandEnabled: false, periodEnd: null }, stale: false, message: null },
    codex: { provider: 'codex', status: 'available', plan: 'Plus', capturedAt: '2026-10-25T06:00:00Z', quotas: [{ label: '5 hours', usedPercent: 30, remainingPercent: 70, resetsAt: null }], tokens: { today: 12000, lifetime: 900000, peakDaily: null }, cost: null, stale: true, message: 'Cached data' },
    claude: { provider: 'claude', status: 'not_logged_in', plan: null, capturedAt: '2026-10-25T06:00:00Z', quotas: [], tokens: null, cost: null, stale: false, message: '<script>alert("untrusted")</script>' },
  },
  settings: { displayMode: 'bubble', refreshMinutes: 5, autostart: false, cursorCompatEnabled: false, bubbleSnapEnabled: true, bubblePercentMode: 'remaining', uiLanguage: 'en', bubbleProviderOrder: ['cursor', 'codex', 'claude'], bubbleVisibleProviders: ['cursor', 'codex', 'claude'], cursorBubbleLabel: 'C', codexBubbleLabel: 'X', claudeBubbleLabel: 'A', cursorBubbleColor: '#9c83ff', codexBubbleColor: '#4bd8c0', claudeBubbleColor: '#e99068' },
  entries: [{ provider: 'cursor', label: 'C', value: 77, stale: false, color: '#9c83ff' }, { provider: 'codex', label: 'X', value: 70, stale: true, color: '#4bd8c0' }, { provider: 'claude', label: 'A', value: null, stale: false, color: '#e99068' }],
  locale: 'en', translations: en, palette: ['#9c83ff', '#4bd8c0', '#e99068', '#ff6b6b'],
  update: { version: '0.5.0', notes: 'A newer release', error: null, downloading: false },
  canInstall: false, version: '0.4.1', panel: 'details', peek: false, nativeGlass: true, loginRunning: false,
};
async function setup(page: Page, overrides: Partial<Payload> = {}) {
  await page.addInitScript(({ initial, catalogs }) => {
    const w = window as any;
    w.fixture = initial; w.calls = []; w.failNext = null;
    const callbacks = new Map(); const listeners = new Map(); let next = 1;
    w.publish = () => listeners.get('metra-state')?.forEach((id: number) => callbacks.get(id)({ payload: structuredClone(w.fixture) }));
    w.__TAURI_INTERNALS__ = {
      transformCallback: (fn: Function) => { const id = next++; callbacks.set(id, fn); return id; },
      invoke: async (cmd: string, args: any = {}) => {
        w.calls.push({ cmd, args });
        if (w.hold === cmd) await new Promise<void>(resolve => { w.releaseTask = resolve; });
        if (w.failNext === cmd) { w.failNext = null; throw 'Could not save settings'; }
        if (cmd === 'plugin:event|listen') { const list = listeners.get(args.event) || []; list.push(args.handler); listeners.set(args.event, list); return args.handler; }
        if (cmd === 'get_state') return structuredClone(w.fixture);
        if (cmd === 'open_panel') w.fixture.panel = args.mode;
        if (cmd === 'bubble_hover') w.fixture.peek = !args.hovered && w.fixture.settings.bubbleSnapEnabled;
        if (cmd === 'change_setting') {
          const { kind, value } = args.change; const s = w.fixture.settings;
          const fields: Record<string, string> = { display: 'displayMode', percent: 'bubblePercentMode', refresh: 'refreshMinutes', language: 'uiLanguage', snap: 'bubbleSnapEnabled', autostart: 'autostart', compat: 'cursorCompatEnabled', order: 'bubbleProviderOrder' };
          if (kind === 'visible') {
            if (!value.visible && s.bubbleVisibleProviders.length === 1) throw 'Keep at least one item visible';
            s.bubbleVisibleProviders = s.bubbleVisibleProviders.filter((p: string) => p !== value.provider);
            if (value.visible) s.bubbleVisibleProviders.push(value.provider);
          } else if (kind === 'label') s[`${value.provider}BubbleLabel`] = [...value.label.trim()].slice(0, 3).join('') || ({ cursor: 'C', codex: 'X', claude: 'A' } as any)[value.provider];
          else if (kind === 'color') s[`${value.provider}BubbleColor`] = value.color;
          else s[fields[kind]] = value;
          if (kind === 'language') { w.fixture.locale = value === 'system' ? 'en' : value; w.fixture.translations = (catalogs as any)[w.fixture.locale]; }
        }
        w.publish();
      },
    };
  }, { initial: { ...fixture, ...overrides }, catalogs: { en, 'zh-CN': zh, ja, ko } });
}
async function settings(page: Page) { await page.getByRole('button', { name: 'Settings', exact: true }).click(); await expect(page.getByText('Provider display', { exact: true })).toBeVisible(); }

test('details render real payload fields, cached data and escaped provider messages', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel');
  await expect(page.getByText('77% remaining')).toBeVisible();
  await expect(page.getByText('Data is out of date')).toBeVisible();
  await expect(page.getByText('$23.00 / $100.00')).toBeVisible();
  await expect(page.locator('[data-provider="claude"]')).toContainText('<script>alert("untrusted")</script>');
  await expect(page.locator('#app script')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Download update', exact: true })).toBeVisible();
  await page.screenshot({ path: test.info().outputPath('details.png') });
});
test('compatibility requires the consent screen and explicit enable action', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel'); await settings(page);
  await page.getByRole('switch', { name: 'Cursor compatibility' }).focus();
  await page.keyboard.press('Space');
  await expect(page.getByText(en['consent.cursorReadOnly'])).toBeVisible();
  expect(await page.evaluate(() => (window as any).fixture.settings.cursorCompatEnabled)).toBe(false);
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: 'Read exact Cursor usage' }).click();
  await page.getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.getByText('77% remaining')).toBeVisible();
  expect(await page.evaluate(() => (window as any).fixture.settings.cursorCompatEnabled)).toBe(true);
});
test('provider order, label, palette and visibility settings remain usable in a small panel', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel'); await settings(page);
  await page.getByRole('button', { name: 'Move up Claude Code' }).click();
  await expect(page.locator('[data-order-provider]').first()).toHaveAttribute('draggable', 'true');
  expect(await page.locator('[data-order-provider]').evaluateAll(rows => rows.map(el => (el as HTMLElement).dataset.orderProvider))).toEqual(['cursor', 'claude', 'codex']);
  const input = page.getByRole('textbox', { name: 'Cursor label' });
  await input.fill('CU'); await input.press('Tab');
  await expect(input).toHaveValue('CU');
  await page.getByRole('button', { name: 'Choose Cursor marker color', exact: true }).click();
  await page.getByRole('button', { name: 'Choose Cursor marker color; currently #ff6b6b' }).click();
  expect(await page.evaluate(() => (window as any).fixture.settings.cursorBubbleColor)).toBe('#ff6b6b');
  await page.locator('wa-checkbox[data-provider-visible=cursor]').click();
  await page.locator('wa-checkbox[data-provider-visible=codex]').click();
  await page.locator('wa-checkbox[data-provider-visible=claude]').click();
  await expect(page.getByRole('status')).toContainText('Keep at least one');
  await expect(page.getByRole('checkbox', { name: 'Show Claude Code' })).toBeChecked();
  await page.screenshot({ path: test.info().outputPath('settings.png') });
});
test('failed settings changes report errors and restore persisted values', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel'); await settings(page);
  await page.evaluate(() => { (window as any).failNext = 'change_setting'; });
  await page.locator('wa-switch[data-setting=autostart]').click();
  await expect(page.getByRole('status')).toContainText('Could not save settings');
  await expect(page.getByRole('switch', { name: 'Launch at startup' })).not.toBeChecked();
});
test('languages switch live and retain provider preferences', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel'); await settings(page);
  for (const locale of ['zh-CN', 'ja', 'ko', 'en']) {
    await page.getByRole('combobox').first().click();
    await page.getByRole('option', { name: ({'zh-CN':'简体中文',ja:'日本語',ko:'한국어',en:'English'} as Record<string,string>)[locale], exact:true }).click();
    await expect(page.getByRole('listbox')).toBeHidden();
    await expect(page.locator('html')).toHaveAttribute('lang', locale);
    await expect(page.getByRole('textbox').first()).toHaveValue('C');
  }
});
test('edge capsule preserves values, expands on hover and supports keyboard opening', async ({ page }) => {
  // Activation must work immediately after startup, before any drag has occurred.
  await page.addInitScript(() => Object.defineProperty(performance, 'now', { value: () => 100 }));
  await setup(page, { peek: true }); await page.setViewportSize({ width: 32, height: 56 });
  // Start outside the capsule; Chromium on Linux starts at (0, 0), inside its root.
  await page.mouse.move(100, 100); await page.goto('/?view=bubble');
  await expect(page.locator('#bubble')).toHaveClass(/peek/);
  await expect(page.locator('.bubble-label').first()).toBeHidden();
  await expect(page.locator('.bubble-entry').first()).toContainText('77%');
  await page.screenshot({ path: test.info().outputPath('capsule.png') });
  await page.locator('#bubble').hover();
  await expect(page.locator('#bubble')).not.toHaveClass(/peek/);
  await page.setViewportSize({ width: 56, height: 56 });
  await page.locator('#bubble').focus(); await page.keyboard.press('Enter');
  expect(await page.evaluate(() => (window as any).calls.some((c: any) => c.cmd === 'open_panel' && c.args.mode === 'details'))).toBe(true);
  await page.screenshot({ path: test.info().outputPath('bubble.png') });
});
test('dragging works without screen coordinates and does not open details', async ({ page }) => {
  // Embedded WebKit can withhold global screen coordinates; local pointer motion still works.
  await page.addInitScript(() => {
    Object.defineProperties(MouseEvent.prototype, {
      screenX: { get: () => 0 }, screenY: { get: () => 0 },
    });
  });
  await setup(page); await page.setViewportSize({ width: 56, height: 56 }); await page.goto('/?view=bubble');
  await expect(page.locator('#bubble')).toBeVisible();
  await page.mouse.move(15, 20); await page.mouse.down(); await page.mouse.move(25, 30); await page.mouse.up();
  await expect.poll(() => page.evaluate(() => (window as any).calls.some((c: any) => c.cmd === 'drag_bubble'))).toBe(true);
  expect(await page.evaluate(() => (window as any).calls.some((c: any) => c.cmd === 'open_panel'))).toBe(false);
});
test('background updates preserve an unfinished label edit', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel'); await settings(page);
  const input = page.getByRole('textbox', { name: 'Cursor label' }); await input.fill('NEW');
  await page.evaluate(() => (window as any).publish());
  await expect(input).toHaveValue('NEW'); await expect(input).toBeFocused();
});

test('long refreshes keep settings and close controls responsive', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel');
  await page.evaluate(() => { (window as any).hold = 'refresh'; });
  await page.getByRole('button', { name: 'Refresh usage now' }).click();
  await settings(page);
  await expect(page.getByRole('button', { name: 'Close', exact: true })).toBeEnabled();
  await page.evaluate(() => { (window as any).releaseTask(); });
});

test('each provider shows and clears its own loading animation', async ({ page }) => {
  await setup(page); await page.goto('/?view=panel');
  await page.evaluate(() => { (window as any).hold = 'refresh'; });
  await page.getByRole('button', { name: 'Refresh usage now' }).click();
  for (const p of ['cursor','codex','claude']) await expect(page.locator(`[data-provider="${p}"]`)).toHaveAttribute('aria-busy','true');
  await page.evaluate(() => {
    const w=window as any; w.fixture.snapshot.refreshing=true; w.fixture.refreshingProviders=['cursor','claude']; w.publish();
  });
  await expect(page.locator('[data-provider="codex"]')).toHaveAttribute('aria-busy','false');
  await expect(page.locator('[data-provider="cursor"] wa-spinner')).toBeVisible();
  await expect(page.locator('[data-provider="claude"] wa-progress-bar')).toBeVisible();
  await page.evaluate(() => { const w=window as any; w.fixture.refreshingProviders=[]; w.fixture.snapshot.refreshing=false; w.publish(); });
  for (const p of ['cursor','codex','claude']) await expect(page.locator(`[data-provider="${p}"]`)).toHaveAttribute('aria-busy','false');
  await page.evaluate(() => (window as any).releaseTask());
});

test('library controls and icons are bundled locally and remain stable across updates', async ({ page }) => {
  const external: string[]=[];
  const errors: string[]=[];
  page.on('console', message => { if(message.type() === 'error') errors.push(message.text()); });
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (/^https?:/.test(request.url()) && !request.url().startsWith('http://127.0.0.1:1420')) external.push(request.url()); });
  await setup(page); await page.goto('/?view=panel');
  await expect(page.locator('wa-button[data-action=settings] wa-icon svg')).toBeVisible();
  await settings(page);
  await expect(page.locator('wa-switch')).toHaveCount(3);
  await expect(page.locator('wa-select')).toHaveCount(4);
  await expect(page.locator('wa-input')).toHaveCount(3);
  const input=page.getByRole('textbox',{name:'Cursor label'}); await input.fill('ABC');
  const element=await input.elementHandle(); await page.evaluate(()=>(window as any).publish());
  expect(await input.evaluate((current, previous)=>current===previous,element)).toBe(true);
  await page.getByRole('button',{name:'Choose Cursor marker color',exact:true}).click();
  await expect(page.getByRole('button',{name:'Choose Cursor marker color; currently #ff6b6b'})).toBeVisible();
  await page.evaluate(()=>(window as any).publish());
  await expect(page.getByRole('button',{name:'Choose Cursor marker color; currently #ff6b6b'})).toBeVisible();
  expect(external).toEqual([]);
  expect(errors).toEqual([]);
});

test.describe('scaled display',()=>{
  test.use({deviceScaleFactor:1.25});
  test('dropdown stays inside the small desktop panel with keyboard access',async({page})=>{
    await setup(page); await page.goto('/?view=panel'); await settings(page);
    const select=page.getByRole('combobox').first(); await select.focus(); await page.keyboard.press('ArrowDown');
    await expect(page.getByRole('option',{name:'English',exact:true})).toBeVisible();
    const box=await page.getByRole('listbox').boundingBox();
    expect(box).not.toBeNull(); expect(box!.x).toBeGreaterThanOrEqual(0); expect(box!.x+box!.width).toBeLessThanOrEqual(361);
    await page.keyboard.press('Escape'); await expect(page.getByRole('option',{name:'English',exact:true})).toBeHidden();
    expect(await page.locator('.panel').evaluate(el=>el.scrollWidth<=el.clientWidth)).toBe(true);
    expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='hide_panel'))).toBe(false);
  });
});

test('app version stays beside the title across all panels',async({page})=>{
  await setup(page); await page.goto('/?view=panel');
  const version=page.locator('.panel-header .brand-title .version');
  await expect(version).toHaveText('v0.4.1'); await expect(version).toBeVisible();
  await settings(page); await page.locator('.scroll').evaluate(el=>el.scrollTop=el.scrollHeight);
  await expect(version).toBeVisible();
  await page.locator('wa-switch[data-setting=compat]').click();
  await expect(page.getByText(en['consent.cursorReadOnly'])).toBeVisible();
  await expect(version).toBeVisible();
});

test('update card stays beneath the title while usage scrolls',async({page})=>{
  await setup(page); await page.goto('/?view=panel');
  const banner=page.locator('.panel > .update'); await expect(banner).toBeVisible();
  const before=await banner.boundingBox();
  await page.locator('.scroll').evaluate(el=>el.scrollTop=el.scrollHeight);
  await expect(banner).toBeVisible();
  const after=await banner.boundingBox(); expect(after!.y).toBe(before!.y);
});
