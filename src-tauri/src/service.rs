use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use serde::Serialize;

use crate::{
    diagnostics,
    model::{DashboardSnapshot, Provider, ProviderSnapshot, ProviderStatus},
    providers::{
        UsageProvider, claude::ClaudeProvider, codex::CodexProvider, cursor::CursorProvider,
    },
    settings::{AppSettings, SettingsStore},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPayload {
    pub snapshot: DashboardSnapshot,
    pub settings: AppSettings,
}

pub struct RefreshService {
    cursor: Arc<dyn UsageProvider>,
    codex: Arc<dyn UsageProvider>,
    claude: Arc<dyn UsageProvider>,
    pub settings: Mutex<AppSettings>,
    pub snapshot: Mutex<DashboardSnapshot>,
    pub refreshing: AtomicBool,
    pub cursor_compat: Arc<AtomicBool>,
    cursor_login_refresh_lock: Mutex<()>,
    pub store: SettingsStore,
}

impl RefreshService {
    pub fn new(store: SettingsStore) -> Self {
        let settings = store.load();
        let cursor_compat = Arc::new(AtomicBool::new(settings.cursor_compat_enabled));
        let cursor = Arc::new(CursorProvider::new(cursor_compat.clone()));
        let codex = Arc::new(CodexProvider::default());
        let claude = Arc::new(ClaudeProvider::default());
        let snapshot = DashboardSnapshot {
            cursor: ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::Unsupported,
                "正在检测 Cursor",
            ),
            codex: ProviderSnapshot::unavailable(
                Provider::Codex,
                ProviderStatus::Unsupported,
                "正在检测 Codex",
            ),
            claude: ProviderSnapshot::unavailable(
                Provider::Claude,
                ProviderStatus::Unsupported,
                "正在检测 Claude Code",
            ),
            refreshing: true,
        };
        Self {
            cursor,
            codex,
            claude,
            settings: Mutex::new(settings),
            snapshot: Mutex::new(snapshot),
            refreshing: AtomicBool::new(false),
            cursor_compat,
            cursor_login_refresh_lock: Mutex::new(()),
            store,
        }
    }

    pub fn payload(&self) -> AppPayload {
        AppPayload {
            snapshot: self.snapshot.lock().expect("snapshot lock").clone(),
            settings: self.settings.lock().expect("settings lock").clone(),
        }
    }

    pub fn refresh(&self, include_cursor: bool) -> DashboardSnapshot {
        let started = Instant::now();
        diagnostics::info("refresh.start", format!("include_cursor={include_cursor}"));
        if self.refreshing.swap(true, Ordering::AcqRel) {
            diagnostics::warn("refresh.skipped", "reason=already_running");
            return self.snapshot.lock().expect("snapshot lock").clone();
        }
        {
            let mut snapshot = self.snapshot.lock().expect("snapshot lock");
            snapshot.refreshing = true;
        }
        let (cursor, codex, claude) = std::thread::scope(|scope| {
            let cursor = include_cursor.then(|| scope.spawn(|| self.cursor.refresh()));
            let codex = scope.spawn(|| self.codex.refresh());
            let claude = scope.spawn(|| self.claude.refresh());
            (
                cursor.map(|worker| {
                    worker.join().unwrap_or_else(|_| {
                        ProviderSnapshot::unavailable(
                            Provider::Cursor,
                            ProviderStatus::ProtocolError,
                            "Cursor 采集器异常",
                        )
                    })
                }),
                codex.join().unwrap_or_else(|_| {
                    ProviderSnapshot::unavailable(
                        Provider::Codex,
                        ProviderStatus::ProtocolError,
                        "Codex 采集器异常",
                    )
                }),
                claude.join().unwrap_or_else(|_| {
                    ProviderSnapshot::unavailable(
                        Provider::Claude,
                        ProviderStatus::ProtocolError,
                        "Claude Code 采集器异常",
                    )
                }),
            )
        });
        let mut current = self.snapshot.lock().expect("snapshot lock");
        if let Some(cursor) = cursor {
            current.cursor = merge_snapshot(current.cursor.clone(), cursor);
        }
        current.codex = merge_snapshot(current.codex.clone(), codex);
        current.claude = merge_snapshot(current.claude.clone(), claude);
        current.refreshing = false;
        self.refreshing.store(false, Ordering::Release);
        diagnostics::info(
            "refresh.complete",
            format!(
                "elapsed_ms={} cursor_status={:?} cursor_stale={} cursor_message={} codex_status={:?} codex_stale={} codex_message={} claude_status={:?} claude_stale={} claude_message={}",
                started.elapsed().as_millis(),
                current.cursor.status,
                current.cursor.stale,
                current.cursor.message.as_deref().unwrap_or("-"),
                current.codex.status,
                current.codex.stale,
                current.codex.message.as_deref().unwrap_or("-"),
                current.claude.status,
                current.claude.stale,
                current.claude.message.as_deref().unwrap_or("-")
            ),
        );
        current.clone()
    }

    pub fn refresh_cursor_login_status(&self) -> DashboardSnapshot {
        let _refresh_guard = self
            .cursor_login_refresh_lock
            .lock()
            .expect("cursor login refresh lock");
        let started = Instant::now();
        let compat_enabled = self.cursor_compat.load(Ordering::Acquire);
        diagnostics::info(
            "cursor.login.recheck.start",
            format!("compat_enabled={compat_enabled}"),
        );
        let refreshed = if compat_enabled {
            self.cursor.refresh()
        } else {
            self.cursor.account_status()
        };
        let mut current = self.snapshot.lock().expect("snapshot lock");
        current.cursor = merge_snapshot(current.cursor.clone(), refreshed);
        diagnostics::info(
            "cursor.login.recheck.complete",
            format!(
                "elapsed_ms={} status={:?}",
                started.elapsed().as_millis(),
                current.cursor.status
            ),
        );
        current.clone()
    }

    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, String> {
        let mut settings = self.settings.lock().map_err(|_| "配置锁异常".to_string())?;
        let mut candidate = settings.clone();
        change(&mut candidate);
        let candidate = candidate.normalized();
        self.store.save(&candidate)?;
        self.cursor_compat
            .store(candidate.cursor_compat_enabled, Ordering::Relaxed);
        *settings = candidate.clone();
        Ok(candidate)
    }
}

pub fn merge_snapshot(
    old: ProviderSnapshot,
    mut failed_or_new: ProviderSnapshot,
) -> ProviderSnapshot {
    if failed_or_new.status == ProviderStatus::Available {
        preserve_nonzero_lifetime_tokens(&old, &mut failed_or_new);
        return failed_or_new;
    }
    if failed_or_new.status == ProviderStatus::NotLoggedIn {
        return failed_or_new;
    }
    if old.status != ProviderStatus::Available {
        return failed_or_new;
    }
    let mut stale = old;
    stale.stale = true;
    stale.message = failed_or_new.message;
    stale
}

fn preserve_nonzero_lifetime_tokens(old: &ProviderSnapshot, new: &mut ProviderSnapshot) {
    let Some(old_lifetime) = old
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.lifetime)
        .filter(|tokens| *tokens > 0)
    else {
        return;
    };
    let new_lifetime = new
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.lifetime)
        .unwrap_or(0);
    if new_lifetime > 0 {
        return;
    }
    diagnostics::warn(
        "snapshot.tokens.preserved",
        format!(
            "provider={:?} previous_lifetime={} refreshed_lifetime={}",
            new.provider, old_lifetime, new_lifetime
        ),
    );
    new.tokens.get_or_insert_default().lifetime = Some(old_lifetime);
}
#[cfg(test)]
mod refresh_selection_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    use chrono::Utc;

    use crate::providers::Detection;

    struct CountingProvider {
        provider: Provider,
        refresh_calls: Arc<AtomicUsize>,
        account_calls: Arc<AtomicUsize>,
    }

    impl UsageProvider for CountingProvider {
        fn detect(&self) -> Detection {
            Detection {
                executable: None,
                status: ProviderStatus::Available,
                detail: None,
            }
        }

        fn account_status(&self) -> ProviderSnapshot {
            self.account_calls.fetch_add(1, Ordering::Relaxed);
            provider_snapshot(self.provider, "account")
        }

        fn refresh(&self) -> ProviderSnapshot {
            self.refresh_calls.fetch_add(1, Ordering::Relaxed);
            provider_snapshot(self.provider, "refreshed")
        }
    }

    fn provider_snapshot(provider: Provider, plan: &str) -> ProviderSnapshot {
        ProviderSnapshot {
            provider,
            status: ProviderStatus::Available,
            plan: Some(plan.into()),
            captured_at: Utc::now(),
            quotas: Vec::new(),
            tokens: None,
            cost: None,
            stale: false,
            message: None,
        }
    }

    #[test]
    fn refresh_without_cursor_preserves_cursor_and_refreshes_codex_and_claude() {
        let cursor_calls = Arc::new(AtomicUsize::new(0));
        let codex_calls = Arc::new(AtomicUsize::new(0));
        let claude_calls = Arc::new(AtomicUsize::new(0));
        let temp = tempfile::tempdir().unwrap();
        let service = RefreshService {
            cursor: Arc::new(CountingProvider {
                provider: Provider::Cursor,
                refresh_calls: cursor_calls.clone(),
                account_calls: Arc::new(AtomicUsize::new(0)),
            }),
            codex: Arc::new(CountingProvider {
                provider: Provider::Codex,
                refresh_calls: codex_calls.clone(),
                account_calls: Arc::new(AtomicUsize::new(0)),
            }),
            claude: Arc::new(CountingProvider {
                provider: Provider::Claude,
                refresh_calls: claude_calls.clone(),
                account_calls: Arc::new(AtomicUsize::new(0)),
            }),
            settings: Mutex::new(AppSettings::default()),
            snapshot: Mutex::new(DashboardSnapshot {
                cursor: provider_snapshot(Provider::Cursor, "preserved"),
                codex: provider_snapshot(Provider::Codex, "old"),
                claude: provider_snapshot(Provider::Claude, "old"),
                refreshing: false,
            }),
            refreshing: AtomicBool::new(false),
            cursor_compat: Arc::new(AtomicBool::new(false)),
            cursor_login_refresh_lock: Mutex::new(()),
            store: SettingsStore::new(temp.path().join("settings.db")),
        };

        let refreshed = service.refresh(false);

        assert_eq!(cursor_calls.load(Ordering::Relaxed), 0);
        assert_eq!(codex_calls.load(Ordering::Relaxed), 1);
        assert_eq!(claude_calls.load(Ordering::Relaxed), 1);
        assert_eq!(refreshed.cursor.plan.as_deref(), Some("preserved"));
        assert_eq!(refreshed.codex.plan.as_deref(), Some("refreshed"));
        assert_eq!(refreshed.claude.plan.as_deref(), Some("refreshed"));

        let refreshed = service.refresh(true);

        assert_eq!(cursor_calls.load(Ordering::Relaxed), 1);
        assert_eq!(codex_calls.load(Ordering::Relaxed), 2);
        assert_eq!(claude_calls.load(Ordering::Relaxed), 2);
        assert_eq!(refreshed.cursor.plan.as_deref(), Some("refreshed"));
    }

    #[test]
    fn cursor_login_recheck_uses_account_status_until_compat_is_authorized() {
        let cursor_refresh_calls = Arc::new(AtomicUsize::new(0));
        let cursor_account_calls = Arc::new(AtomicUsize::new(0));
        let temp = tempfile::tempdir().unwrap();
        let service = RefreshService {
            cursor: Arc::new(CountingProvider {
                provider: Provider::Cursor,
                refresh_calls: cursor_refresh_calls.clone(),
                account_calls: cursor_account_calls.clone(),
            }),
            codex: Arc::new(CountingProvider {
                provider: Provider::Codex,
                refresh_calls: Arc::new(AtomicUsize::new(0)),
                account_calls: Arc::new(AtomicUsize::new(0)),
            }),
            claude: Arc::new(CountingProvider {
                provider: Provider::Claude,
                refresh_calls: Arc::new(AtomicUsize::new(0)),
                account_calls: Arc::new(AtomicUsize::new(0)),
            }),
            settings: Mutex::new(AppSettings::default()),
            snapshot: Mutex::new(DashboardSnapshot {
                cursor: ProviderSnapshot::unavailable(
                    Provider::Cursor,
                    ProviderStatus::NotLoggedIn,
                    "尚未登录",
                ),
                codex: provider_snapshot(Provider::Codex, "old"),
                claude: provider_snapshot(Provider::Claude, "old"),
                refreshing: false,
            }),
            refreshing: AtomicBool::new(false),
            cursor_compat: Arc::new(AtomicBool::new(false)),
            cursor_login_refresh_lock: Mutex::new(()),
            store: SettingsStore::new(temp.path().join("settings.db")),
        };

        let account = service.refresh_cursor_login_status();
        assert_eq!(account.cursor.plan.as_deref(), Some("account"));
        assert_eq!(cursor_account_calls.load(Ordering::Relaxed), 1);
        assert_eq!(cursor_refresh_calls.load(Ordering::Relaxed), 0);

        service.cursor_compat.store(true, Ordering::Relaxed);
        let precise = service.refresh_cursor_login_status();
        assert_eq!(precise.cursor.plan.as_deref(), Some("refreshed"));
        assert_eq!(cursor_account_calls.load(Ordering::Relaxed), 1);
        assert_eq!(cursor_refresh_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn failed_settings_save_does_not_publish_candidate() {
        let temp = tempfile::tempdir().unwrap();
        let service = RefreshService::new(SettingsStore::new(temp.path().to_path_buf()));
        let before = service.payload().settings;

        let result = service.update_settings(|settings| {
            settings.bubble_visible_providers = vec![Provider::Claude];
            settings.cursor_compat_enabled = true;
        });

        assert!(result.is_err());
        assert_eq!(service.payload().settings, before);
        assert!(!service.cursor_compat.load(Ordering::Relaxed));
    }
}
