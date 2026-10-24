use metra_lib::model::{CostUsage, Provider, ProviderStatus, QuotaKind, QuotaWindow};

#[test]
fn quota_window_clamps_usage_and_derives_remaining_percentage() {
    let quota = QuotaWindow::from_used_percent("five-hour", 125.0, Some(300), None);

    assert_eq!(quota.used_percent, 100.0);
    assert_eq!(quota.remaining_percent, 0.0);
    assert_eq!(quota.window_duration_mins, Some(300));
}

#[test]
fn provider_and_status_have_stable_wire_names() {
    assert_eq!(
        serde_json::to_string(&Provider::Cursor).unwrap(),
        "\"cursor\""
    );
    assert_eq!(
        serde_json::to_string(&ProviderStatus::NotLoggedIn).unwrap(),
        "\"not_logged_in\""
    );
    assert_eq!(
        serde_json::to_string(&ProviderStatus::DesktopInstalled).unwrap(),
        "\"desktop_installed\""
    );
    assert_eq!(
        serde_json::to_string(&Provider::Claude).unwrap(),
        "\"claude\""
    );
}

#[test]
fn cursor_ultra_fields_have_stable_wire_names() {
    let quota = QuotaWindow::from_used_percent("Cursor Models", 1.0, None, None)
        .with_kind(QuotaKind::CursorModels);
    let quota_json = serde_json::to_value(quota).unwrap();
    assert_eq!(quota_json["kind"], "cursor_models");

    let cost = CostUsage {
        currency: "USD".into(),
        today_used_cents: Some(123),
        included_used_cents: None,
        included_limit_cents: None,
        on_demand_used_cents: Some(0),
        on_demand_limit_cents: None,
        on_demand_enabled: Some(false),
        period_end: None,
    };
    let cost_json = serde_json::to_value(cost).unwrap();
    assert_eq!(cost_json["todayUsedCents"], 123);
    assert_eq!(cost_json["onDemandEnabled"], false);
}
