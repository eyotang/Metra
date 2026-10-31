export type Provider = 'cursor' | 'codex' | 'claude';
export interface Settings {
  displayMode: 'bubble' | 'statusBar'; refreshMinutes: number; autostart: boolean;
  cursorCompatEnabled: boolean; bubbleSnapEnabled: boolean; bubblePercentMode: 'used' | 'remaining';
  uiLanguage: 'system' | 'zh-CN' | 'en' | 'ja' | 'ko' | 'fr' | 'de' | 'ru';
  bubbleProviderOrder: Provider[]; bubbleVisibleProviders: Provider[];
  cursorBubbleLabel: string; codexBubbleLabel: string; claudeBubbleLabel: string;
  cursorBubbleColor: string; codexBubbleColor: string; claudeBubbleColor: string;
}
export interface Snapshot {
  provider: Provider; status: string; plan: string | null; capturedAt: string;
  quotas: { label: string; kind?: string; usedPercent: number; remainingPercent: number; resetsAt: string | null }[];
  tokens: { today: number | null; lifetime: number | null; peakDaily: number | null } | null;
  cost: { currency: string; todayUsedCents?: number; includedUsedCents: number | null; includedLimitCents: number | null; onDemandUsedCents: number | null; onDemandLimitCents: number | null; onDemandEnabled?: boolean; periodEnd: string | null } | null;
  stale: boolean; message: string | null;
}
export interface Entry { provider: Provider; label: string; value: number | null; stale: boolean; color: string }
export interface Payload {
  refreshingProviders: Provider[];
  snapshot: Record<Provider, Snapshot> & { refreshing: boolean };
  settings: Settings; entries: Entry[]; locale: string; translations: Record<string, string>; palette: string[];
  update: { version: string | null; notes: string | null; error: string | null; downloading: boolean };
  canInstall: boolean; version: string; panel: 'details' | 'settings' | 'consent'; peek: boolean; loginRunning: boolean;
}
export type Change =
  | { kind: 'display'; value: Settings['displayMode'] }
  | { kind: 'percent'; value: Settings['bubblePercentMode'] }
  | { kind: 'refresh'; value: number }
  | { kind: 'language'; value: Settings['uiLanguage'] }
  | { kind: 'snap' | 'autostart' | 'compat'; value: boolean }
  | { kind: 'visible'; value: { provider: Provider; visible: boolean } }
  | { kind: 'label'; value: { provider: Provider; label: string } }
  | { kind: 'color'; value: { provider: Provider; color: string } }
  | { kind: 'order'; value: Provider[] };
