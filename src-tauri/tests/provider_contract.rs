use metra_lib::providers::{
    claude::snapshot_from_auth_status,
    codex::snapshot_from_messages,
    cursor::{
        snapshot_from_payloads, snapshot_from_payloads_with_sand,
        snapshot_from_payloads_with_ultra_details,
    },
};
use metra_lib::model::QuotaKind;
use serde_json::json;

#[test]
fn codex_protocol_exposes_standard_and_spark_quota_summaries() {
    let snapshot = snapshot_from_messages(&[
        json!({"id": 1, "result": {"account": {"type": "chatgpt", "planType": "plus"}}}),
        json!({"id": 2, "result": {
            "rateLimitsByLimitId": {
                "codex": {
                    "limitId": "codex",
                    "limitName": "5 小时",
                    "planType": "pro",
                    "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": 1893456000},
                    "secondary": {"usedPercent": 40, "windowDurationMins": 10080, "resetsAt": 1894060800}
                },
                "codex_spark": {
                    "limitId": "codex_spark",
                    "limitName": "GPT-5.3-Codex-Spark",
                    "primary": {"usedPercent": 0, "windowDurationMins": 10080, "resetsAt": 1894060800}
                }
            }
        }}),
        json!({"id": 3, "result": {
            "summary": {"lifetimeTokens": 1234567, "peakDailyTokens": 45678},
            "dailyUsageBuckets": [{"startDate": "2099-01-01", "tokens": 12345}]
        }})
    ]).expect("valid Codex messages");

    assert_eq!(snapshot.plan.as_deref(), Some("pro"));
    assert_eq!(snapshot.quotas.len(), 3);
    assert_eq!(snapshot.quotas[0].remaining_percent, 75.0);
    assert_eq!(snapshot.quotas[1].window_duration_mins, Some(10080));
    assert_eq!(snapshot.quotas[2].label, "GPT-5.3-Codex-Spark");
    assert_eq!(snapshot.quotas[2].remaining_percent, 100.0);
    assert_eq!(snapshot.quotas[2].window_duration_mins, Some(10080));
    assert!(snapshot.quotas[2].resets_at.is_some());
    assert_eq!(snapshot.tokens.unwrap().lifetime, Some(1_234_567));
}

#[test]
fn cursor_team_keeps_the_legacy_included_and_on_demand_layout() {
    let snapshot = snapshot_from_payloads(
        Some(&json!({"gpt-4": {"numRequests": 20, "maxRequestUsage": 100, "numTokens": 12345}})),
        Some(&json!({"individualUsage": {"onDemand": {"used": 321}}})),
        Some(&json!({
            "planUsage": {"limit": 2000, "includedSpend": 750},
            "spendLimitUsage": {"individualUsed": 321, "individualLimit": 50000}
        })),
        Some("team".into()),
    )
    .expect("valid Cursor payloads");

    assert!((snapshot.quotas[0].used_percent - (750.0 + 321.0) / 520.0).abs() < 0.001);
    assert!((snapshot.quotas[0].remaining_percent - 97.940_384_615).abs() < 0.001);
    let cost = snapshot.cost.expect("cost data");
    assert_eq!(cost.included_used_cents, Some(750));
    assert_eq!(cost.included_limit_cents, Some(2000));
    assert_eq!(cost.on_demand_used_cents, Some(321));
    assert_eq!(cost.on_demand_limit_cents, Some(50_000));
    assert_eq!(cost.on_demand_enabled, None);
    let tokens = snapshot.tokens.expect("Cursor period tokens");
    assert_eq!(tokens.today, None);
    assert_eq!(tokens.lifetime, Some(12_345));
}

#[test]
fn cursor_ultra_exposes_separate_cursor_and_other_model_pools() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({
            "membershipType": "ultra",
            "individualUsage": {"onDemand": {"enabled": false, "used": 0, "limit": null}}
        })),
        Some(&json!({
            "planUsage": {
                "autoSpend": 100,
                "autoLimit": 10000,
                "autoPercentUsed": 1,
                "apiSpend": 19200,
                "apiLimit": 40000,
                "apiPercentUsed": 48
            },
            "billingCycleEnd": 4_102_444_800_000_i64
        })),
        Some("ultra".into()),
    )
    .expect("valid Cursor Ultra payload");

    assert_eq!(snapshot.quotas.len(), 2);
    assert_eq!(snapshot.quotas[0].label, "Cursor Models");
    assert_eq!(snapshot.quotas[0].kind, Some(QuotaKind::CursorModels));
    assert_eq!(snapshot.quotas[0].used_percent, 1.0);
    assert_eq!(snapshot.quotas[0].remaining_percent, 99.0);
    assert_eq!(snapshot.quotas[1].label, "Other Models");
    assert_eq!(snapshot.quotas[1].kind, Some(QuotaKind::OtherModels));
    assert_eq!(snapshot.quotas[1].used_percent, 48.0);
    assert_eq!(snapshot.quotas[1].remaining_percent, 52.0);
    assert_eq!(snapshot.remaining_percent(), Some(52.0));
    assert!(snapshot.quotas.iter().all(|quota| !quota.label.starts_with("Cursor 总额度")));
}

#[test]
fn cursor_ultra_exposes_grok_bot_weekly_usage() {
    let snapshot = snapshot_from_payloads_with_sand(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({
            "planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48},
            "billingCycleEnd": "2099-12-31T00:00:00Z"
        })),
        Some(&json!({
            "usagePercent": 0,
            "currentPeriodStart": "2099-12-24T00:00:00Z",
            "nextResetTimestampUtc": "2099-12-31T00:00:00Z",
            "includedLimitZero": false,
            "hasAvailableUsage": true,
            "hasNonZeroIncludedLimit": true
        })),
        None,
    )
    .expect("valid Cursor Ultra and Grok Bot payloads");

    assert_eq!(snapshot.plan.as_deref(), Some("ultra"));
    assert_eq!(snapshot.quotas.len(), 3);
    assert_eq!(snapshot.quotas[2].label, "Grok Bot");
    assert_eq!(snapshot.quotas[2].kind, Some(QuotaKind::GrokBot));
    assert_eq!(snapshot.quotas[2].used_percent, 0.0);
    assert_eq!(snapshot.quotas[2].remaining_percent, 100.0);
    assert_eq!(snapshot.quotas[2].window_duration_mins, Some(10_080));
    assert!(snapshot.quotas[2].resets_at.is_some());
}

#[test]
fn cursor_ultra_keeps_an_exhausted_grok_bot_quota_visible() {
    let snapshot = snapshot_from_payloads_with_sand(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({"planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48}})),
        Some(&json!({
            "usagePercent": 100,
            "includedLimitZero": false,
            "hasAvailableUsage": false,
            "hasNonZeroIncludedLimit": true
        })),
        None,
    )
    .expect("valid exhausted Cursor Ultra Grok Bot payload");

    assert_eq!(snapshot.quotas.len(), 3);
    assert_eq!(snapshot.quotas[2].kind, Some(QuotaKind::GrokBot));
    assert_eq!(snapshot.quotas[2].used_percent, 100.0);
    assert_eq!(snapshot.quotas[2].remaining_percent, 0.0);
}

#[test]
fn cursor_ultra_does_not_invent_a_grok_bot_quota_without_entitlement() {
    let snapshot = snapshot_from_payloads_with_sand(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({"planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48}})),
        Some(&json!({
            "includedLimitZero": true,
            "hasAvailableUsage": false,
            "hasNonZeroIncludedLimit": false
        })),
        None,
    )
    .expect("valid Cursor Ultra payload without Grok Bot entitlement");

    assert_eq!(snapshot.quotas.len(), 2);
    assert!(snapshot.quotas.iter().all(|quota| quota.kind != Some(QuotaKind::GrokBot)));
}

#[test]
fn cursor_ultra_reports_disabled_on_demand_without_inventing_a_limit() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({
            "membershipType": "ultra",
            "individualUsage": {"onDemand": {"enabled": false, "used": 0}}
        })),
        Some(&json!({
            "planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48},
            "billingCycleEnd": "2099-12-31T00:00:00Z"
        })),
        None,
    )
    .expect("valid Cursor Ultra payload");

    let cost = snapshot.cost.expect("Ultra On-Demand state");
    assert_eq!(cost.on_demand_enabled, Some(false));
    assert_eq!(cost.on_demand_limit_cents, None);
}

#[test]
fn cursor_ultra_uses_one_consistent_individual_on_demand_scope() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({
            "planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48},
            "spendLimitUsage": {
                "individualUsed": 2500,
                "individualLimit": 10000,
                "individualRemaining": 7500,
                "pooledUsed": 80000,
                "pooledLimit": 100000
            }
        })),
        None,
    )
    .expect("valid Cursor Ultra On-Demand payload");

    let cost = snapshot.cost.expect("Ultra On-Demand cost state");
    assert_eq!(cost.on_demand_enabled, Some(true));
    assert_eq!(cost.on_demand_used_cents, Some(2_500));
    assert_eq!(cost.on_demand_limit_cents, Some(10_000));
}

#[test]
fn cursor_ultra_keeps_explicit_on_demand_enabled_without_a_usage_limit() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({
            "membershipType": "ultra",
            "individualUsage": {"onDemand": {"enabled": true}}
        })),
        Some(&json!({
            "planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48},
            "spendLimitUsage": {"individualUsed": 1200, "individualLimit": 0}
        })),
        None,
    )
    .expect("valid Cursor Ultra enabled On-Demand payload");

    let cost = snapshot.cost.expect("Ultra enabled On-Demand state");
    assert_eq!(cost.on_demand_enabled, Some(true));
    assert_eq!(cost.on_demand_limit_cents, None);
}

#[test]
fn cursor_ultra_uses_the_hard_limit_endpoint_for_disabled_and_unlimited_states() {
    let dashboard = json!({
        "planUsage": {"autoPercentUsed": 1, "apiPercentUsed": 48}
    });
    let disabled = snapshot_from_payloads_with_ultra_details(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&dashboard),
        None,
        Some(&json!({
            "hardLimit": 0,
            "noUsageBasedAllowed": true,
            "onDemandSpendDisabledByOrganization": false
        })),
        None,
    )
    .expect("valid disabled Cursor Ultra hard limit");
    assert_eq!(
        disabled.cost.expect("disabled On-Demand state").on_demand_enabled,
        Some(false)
    );

    let unlimited = snapshot_from_payloads_with_ultra_details(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&dashboard),
        None,
        Some(&json!({
            "hardLimit": 2_147_483_647_i64,
            "noUsageBasedAllowed": false,
            "onDemandSpendDisabledByOrganization": false
        })),
        None,
    )
    .expect("valid unlimited Cursor Ultra hard limit");
    let cost = unlimited.cost.expect("unlimited On-Demand state");
    assert_eq!(cost.on_demand_enabled, Some(true));
    assert_eq!(cost.on_demand_limit_cents, None);
}

#[test]
fn cursor_ultra_calculates_pool_percentages_when_direct_percentages_are_absent() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({
            "planUsage": {
                "autoSpend": 100,
                "autoLimit": 10000,
                "apiSpend": 19200,
                "apiLimit": 40000
            }
        })),
        None,
    )
    .expect("valid Cursor Ultra monetary pool payload");

    assert_eq!(snapshot.quotas.len(), 2);
    assert_eq!(snapshot.quotas[0].kind, Some(QuotaKind::CursorModels));
    assert_eq!(snapshot.quotas[0].used_percent, 1.0);
    assert_eq!(snapshot.quotas[1].kind, Some(QuotaKind::OtherModels));
    assert_eq!(snapshot.quotas[1].used_percent, 48.0);
}

#[test]
fn cursor_ultra_never_falls_back_to_the_legacy_combined_quota() {
    let snapshot = snapshot_from_payloads(
        None,
        Some(&json!({"membershipType": "ultra"})),
        Some(&json!({
            "planUsage": {"includedSpend": 1000, "limit": 20000},
            "billingCycleEnd": "2099-12-31T00:00:00Z"
        })),
        None,
    )
    .expect("valid partial Cursor Ultra payload");

    assert!(snapshot.quotas.is_empty());
    let cost = snapshot.cost.expect("partial Ultra cost state");
    assert_eq!(cost.on_demand_enabled, None);
    assert_eq!(cost.on_demand_limit_cents, None);
}

#[test]
fn claude_api_key_login_is_available_without_inventing_quota() {
    let snapshot = snapshot_from_auth_status(
        &json!({
            "loggedIn": true,
            "authMethod": "api_key",
            "subscriptionType": null,
            "apiProvider": "firstParty"
        }),
        None,
    )
    .expect("valid Claude auth status");

    assert_eq!(snapshot.provider, metra_lib::model::Provider::Claude);
    assert_eq!(
        snapshot.status,
        metra_lib::model::ProviderStatus::Available
    );
    assert_eq!(snapshot.plan.as_deref(), Some("API Key"));
    assert!(snapshot.quotas.is_empty());
    assert!(snapshot.tokens.is_none());
    assert!(snapshot.message.as_deref().is_some_and(|message| message.contains("额度")));
}

#[test]
fn claude_subscription_and_local_tokens_are_exposed() {
    let tokens = metra_lib::model::TokenUsage {
        today: Some(1200),
        lifetime: Some(9800),
        peak_daily: Some(4000),
    };
    let snapshot = snapshot_from_auth_status(
        &json!({
            "loggedIn": true,
            "authMethod": "oauth",
            "subscriptionType": "max",
            "apiProvider": "firstParty"
        }),
        Some(tokens.clone()),
    )
    .expect("valid Claude auth status");

    assert_eq!(snapshot.plan.as_deref(), Some("Max"));
    assert_eq!(snapshot.tokens, Some(tokens));
}

#[test]
fn claude_logged_out_status_is_rejected() {
    let error = snapshot_from_auth_status(
        &json!({"loggedIn": false, "authMethod": "none"}),
        None,
    )
    .expect_err("logged-out Claude must not be reported as available");

    assert!(error.contains("未登录"));
}
