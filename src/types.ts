export type ProviderName = "cursor" | "codex" | "claude";
export type ProviderStatus = "available" | "desktop_installed" | "not_installed" | "not_logged_in" | "unsupported" | "network_error" | "protocol_error";
export type QuotaKind = "cursor_models" | "other_models" | "grok_bot";
export interface QuotaWindow { kind?: QuotaKind; label: string; usedPercent: number; remainingPercent: number; windowDurationMins?: number; resetsAt?: string; }
export interface TokenUsage { today?: number; lifetime?: number; peakDaily?: number; }
export interface CostUsage { currency: string; includedUsedCents?: number; includedLimitCents?: number; onDemandUsedCents?: number; onDemandLimitCents?: number; onDemandEnabled?: boolean; periodEnd?: string; }
export interface ProviderSnapshot { provider: ProviderName; status: ProviderStatus; plan?: string; capturedAt: string; quotas: QuotaWindow[]; tokens?: TokenUsage; cost?: CostUsage; stale: boolean; message?: string; }
export interface DashboardSnapshot { cursor: ProviderSnapshot; codex: ProviderSnapshot; claude: ProviderSnapshot; refreshing: boolean; }
export type BubblePercentMode = "used" | "remaining";
export interface AppSettings { refreshMinutes: number; autostart: boolean; cursorCompatEnabled: boolean; bubbleSnapEnabled: boolean; bubblePercentMode: BubblePercentMode; bubblePosition?: { x: number; y: number }; bubblePositionVersion: number; bubbleProviderOrder: ProviderName[]; bubbleVisibleProviders: ProviderName[]; cursorBubbleLabel: string; codexBubbleLabel: string; claudeBubbleLabel: string; cursorBubbleColor: string; codexBubbleColor: string; claudeBubbleColor: string; }
export interface AppPayload { snapshot: DashboardSnapshot; settings: AppSettings; }
