use chrono::Utc;
use metra_lib::{
    model::{Provider, ProviderSnapshot, ProviderStatus, QuotaWindow, TokenUsage},
    service::merge_snapshot,
};

#[test]
fn failed_refresh_keeps_last_good_values_and_marks_them_stale() {
    let old = ProviderSnapshot {
        provider: Provider::Codex,
        status: ProviderStatus::Available,
        plan: Some("plus".into()),
        captured_at: Utc::now(),
        quotas: vec![QuotaWindow::from_used_percent("5h", 30.0, Some(300), None)],
        tokens: None,
        cost: None,
        stale: false,
        message: None,
    };
    let failed =
        ProviderSnapshot::unavailable(Provider::Codex, ProviderStatus::NetworkError, "网络不可用");
    let merged = merge_snapshot(old, failed);
    assert_eq!(merged.quotas[0].remaining_percent, 70.0);
    assert!(merged.stale);
    assert_eq!(merged.message.as_deref(), Some("网络不可用"));
}

#[test]
fn logged_out_refresh_replaces_stale_values_so_login_action_is_visible() {
    let old = ProviderSnapshot {
        provider: Provider::Cursor,
        status: ProviderStatus::Available,
        plan: Some("ultra".into()),
        captured_at: Utc::now(),
        quotas: vec![QuotaWindow::from_used_percent("Cursor Models", 30.0, None, None)],
        tokens: None,
        cost: None,
        stale: false,
        message: None,
    };
    let logged_out = ProviderSnapshot::unavailable(
        Provider::Cursor,
        ProviderStatus::NotLoggedIn,
        "Cursor 尚未登录",
    );

    let merged = merge_snapshot(old, logged_out);

    assert_eq!(merged.status, ProviderStatus::NotLoggedIn);
    assert!(merged.quotas.is_empty());
    assert!(!merged.stale);
}

#[test]
fn successful_refresh_does_not_replace_nonzero_lifetime_tokens_with_zero() {
    let snapshot = |today, lifetime| ProviderSnapshot {
        provider: Provider::Codex,
        status: ProviderStatus::Available,
        plan: Some("pro".into()),
        captured_at: Utc::now(),
        quotas: vec![QuotaWindow::from_used_percent("5h", 20.0, Some(300), None)],
        tokens: Some(TokenUsage {
            today: Some(today),
            lifetime: Some(lifetime),
            peak_daily: None,
        }),
        cost: None,
        stale: false,
        message: None,
    };

    let merged = merge_snapshot(snapshot(700_000, 470_535_347), snapshot(788_206, 0));
    let tokens = merged.tokens.expect("merged Codex tokens");
    assert_eq!(tokens.today, Some(788_206));
    assert_eq!(tokens.lifetime, Some(470_535_347));
}
