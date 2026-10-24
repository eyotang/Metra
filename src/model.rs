use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Cursor,
    Codex,
    Claude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Available,
    DesktopInstalled,
    NotInstalled,
    NotLoggedIn,
    Unsupported,
    NetworkError,
    ProtocolError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaKind {
    CursorModels,
    OtherModels,
    GrokBot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<QuotaKind>,
    pub label: String,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_duration_mins: Option<u64>,
    pub resets_at: Option<DateTime<Utc>>,
}

impl QuotaWindow {
    pub fn from_used_percent(
        label: impl Into<String>,
        used_percent: f64,
        window_duration_mins: Option<u64>,
        resets_at: Option<DateTime<Utc>>,
    ) -> Self {
        let used_percent = if used_percent.is_finite() {
            used_percent.clamp(0.0, 100.0)
        } else {
            0.0
        };
        Self {
            kind: None,
            label: label.into(),
            used_percent,
            remaining_percent: 100.0 - used_percent,
            window_duration_mins,
            resets_at,
        }
    }

    pub fn with_kind(mut self, kind: QuotaKind) -> Self {
        self.kind = Some(kind);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub today: Option<u64>,
    pub lifetime: Option<u64>,
    pub peak_daily: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostUsage {
    pub currency: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub today_used_cents: Option<u64>,
    pub included_used_cents: Option<u64>,
    pub included_limit_cents: Option<u64>,
    pub on_demand_used_cents: Option<u64>,
    pub on_demand_limit_cents: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    pub period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub provider: Provider,
    pub status: ProviderStatus,
    pub plan: Option<String>,
    pub captured_at: DateTime<Utc>,
    pub quotas: Vec<QuotaWindow>,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<CostUsage>,
    pub stale: bool,
    pub message: Option<String>,
}

impl ProviderSnapshot {
    pub fn unavailable(
        provider: Provider,
        status: ProviderStatus,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            status,
            plan: None,
            captured_at: Utc::now(),
            quotas: Vec::new(),
            tokens: None,
            cost: None,
            stale: false,
            message: Some(message.into()),
        }
    }

    pub fn remaining_percent(&self) -> Option<f64> {
        self.quotas
            .iter()
            .map(|quota| quota.remaining_percent)
            .reduce(f64::min)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub cursor: ProviderSnapshot,
    pub codex: ProviderSnapshot,
    pub claude: ProviderSnapshot,
    pub refreshing: bool,
}
