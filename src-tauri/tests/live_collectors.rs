use std::sync::{Arc, atomic::AtomicBool};

use metra_lib::{
    model::ProviderStatus,
    providers::{
        UsageProvider, claude::ClaudeProvider, codex::CodexProvider, cursor::CursorProvider,
    },
};

#[test]
#[ignore = "requires the user's live Cursor login"]
fn live_cursor_compat_fetches_usage() {
    let provider = CursorProvider::new(Arc::new(AtomicBool::new(true)));
    let snapshot = provider.refresh();
    assert_eq!(
        snapshot.status,
        ProviderStatus::Available,
        "{}",
        snapshot.message.unwrap_or_default()
    );
    assert!(
        !snapshot.quotas.is_empty() || snapshot.cost.is_some(),
        "Cursor returned no quota or cost data"
    );
    assert!(
        snapshot
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.lifetime)
            .is_some(),
        "Cursor returned no current-period token total"
    );
}

#[test]
#[ignore = "requires the user's live Codex login"]
fn live_codex_fetches_usage() {
    let snapshot = CodexProvider::default().refresh();
    assert_eq!(
        snapshot.status,
        ProviderStatus::Available,
        "{}",
        snapshot.message.unwrap_or_default()
    );
    assert!(!snapshot.quotas.is_empty(), "Codex returned no quota data");
    let today = snapshot.tokens.as_ref().and_then(|tokens| tokens.today);
    assert!(
        today.is_some_and(|tokens| tokens > 0),
        "Codex returned no tokens for today: {:?}",
        snapshot.tokens
    );
}
#[test]
#[ignore = "requires the user's live Codex login and METRA_EXPECTED_CODEX_PLAN"]
fn live_codex_plan_matches_expected() {
    let expected =
        std::env::var("METRA_EXPECTED_CODEX_PLAN").expect("METRA_EXPECTED_CODEX_PLAN is required");
    let snapshot = CodexProvider::default().refresh();
    assert_eq!(
        snapshot.plan.as_deref(),
        Some(expected.as_str()),
        "Codex live plan does not match the subscribed plan"
    );
}

#[test]
#[ignore = "requires the user's live Claude Code login"]
fn live_claude_detects_login_without_requiring_quota_data() {
    let snapshot = ClaudeProvider::default().refresh();
    assert_eq!(
        snapshot.status,
        ProviderStatus::Available,
        "{}",
        snapshot.message.clone().unwrap_or_default()
    );
    assert_eq!(snapshot.provider, metra_lib::model::Provider::Claude);
    assert!(snapshot.plan.is_some());
}
