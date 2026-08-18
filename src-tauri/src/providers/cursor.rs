use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::{Map, Value};

use crate::diagnostics;
use crate::model::{
    CostUsage, Provider, ProviderSnapshot, ProviderStatus, QuotaKind, QuotaWindow, TokenUsage,
};

const CURSOR_ON_DEMAND_FALLBACK_CENTS: u64 = 50_000;
const CURSOR_PRO_INCLUDED_FALLBACK_CENTS: u64 = 2_000;

pub fn snapshot_from_payloads(
    auth_usage: Option<&Value>,
    summary: Option<&Value>,
    dashboard: Option<&Value>,
    plan: Option<String>,
) -> Result<ProviderSnapshot, String> {
    snapshot_from_payloads_with_on_demand(auth_usage, summary, dashboard, None, None, plan, None)
}

pub fn snapshot_from_payloads_with_sand(
    auth_usage: Option<&Value>,
    summary: Option<&Value>,
    dashboard: Option<&Value>,
    sand: Option<&Value>,
    plan: Option<String>,
) -> Result<ProviderSnapshot, String> {
    snapshot_from_payloads_with_on_demand(auth_usage, summary, dashboard, sand, None, plan, None)
}

pub fn snapshot_from_payloads_with_ultra_details(
    auth_usage: Option<&Value>,
    summary: Option<&Value>,
    dashboard: Option<&Value>,
    sand: Option<&Value>,
    hard_limit: Option<&Value>,
    plan: Option<String>,
) -> Result<ProviderSnapshot, String> {
    snapshot_from_payloads_with_on_demand(
        auth_usage,
        summary,
        dashboard,
        sand,
        hard_limit,
        plan,
        None,
    )
}

fn snapshot_from_payloads_with_on_demand(
    auth_usage: Option<&Value>,
    summary: Option<&Value>,
    dashboard: Option<&Value>,
    sand: Option<&Value>,
    hard_limit: Option<&Value>,
    plan: Option<String>,
    on_demand_events_cents: Option<u64>,
) -> Result<ProviderSnapshot, String> {
    let plan = resolved_cursor_plan(plan, summary);
    let is_ultra = plan
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("ultra"));
    let request_quota = auth_usage.and_then(find_request_quota);
    let plan_usage = dashboard
        .and_then(|value| value.get("planUsage"))
        .and_then(Value::as_object)
        .or_else(|| {
            summary
                .and_then(|value| value.pointer("/individualUsage/plan"))
                .and_then(Value::as_object)
        });
    let mut quotas = Vec::new();
    let cursor_models_percent = is_ultra
        .then(|| {
            plan_usage.and_then(|usage| {
                pool_used_percent(usage, "autoPercentUsed", "autoSpend", "autoLimit")
            })
        })
        .flatten();
    let other_models_percent = is_ultra
        .then(|| {
            plan_usage.and_then(|usage| {
                pool_used_percent(usage, "apiPercentUsed", "apiSpend", "apiLimit")
            })
        })
        .flatten();
    if let Some(used_percent) = cursor_models_percent {
        quotas.push(
            QuotaWindow::from_used_percent(
                "Cursor Models",
                used_percent,
                None,
                billing_end(dashboard, summary),
            )
            .with_kind(QuotaKind::CursorModels),
        );
    }
    if let Some(used_percent) = other_models_percent {
        quotas.push(
            QuotaWindow::from_used_percent(
                "Other Models",
                used_percent,
                None,
                billing_end(dashboard, summary),
            )
            .with_kind(QuotaKind::OtherModels),
        );
    }
    if is_ultra
        && let Some(sand) = sand
        && sand.get("includedLimitZero").and_then(Value::as_bool) != Some(true)
        && sand
            .get("hasNonZeroIncludedLimit")
            .and_then(Value::as_bool)
            != Some(false)
        && let Some(used_percent) = number(sand.get("usagePercent"))
    {
        quotas.push(
            QuotaWindow::from_used_percent(
                "Grok Bot",
                used_percent,
                Some(10_080),
                timestamp_value(
                    sand.get("nextResetTimestampUtc")
                        .or_else(|| sand.get("nextResetTimestampUTC")),
                ),
            )
            .with_kind(QuotaKind::GrokBot),
        );
    }
    if !is_ultra && let Some((used, maximum)) = request_quota {
        quotas.push(QuotaWindow::from_used_percent(
            "Agent 请求",
            used * 100.0 / maximum,
            None,
            None,
        ));
    } else if !is_ultra
        && let Some(plan_usage) = plan_usage
        && let (Some(used), Some(limit)) = (
            first_number(plan_usage, &["includedSpend", "used"]),
            number(plan_usage.get("limit")),
        )
        && limit > 0.0
    {
        quotas.push(QuotaWindow::from_used_percent(
            "Included",
            used * 100.0 / limit,
            None,
            billing_end(dashboard, summary),
        ));
    }
    let included_used = plan_usage
        .and_then(|value| first_number(value, &["includedSpend", "used"]))
        .map(as_cents);
    let included_limit = plan_usage
        .and_then(|value| number(value.get("limit")))
        .map(as_cents)
        .filter(|limit| *limit > 0);
    let (on_demand_used, on_demand_limit) = on_demand(summary, dashboard);
    let on_demand_enabled =
        ultra_on_demand_enabled(is_ultra, summary, dashboard, hard_limit);
    let on_demand_used_cents = on_demand_events_cents.or_else(|| on_demand_used.map(as_cents));
    let on_demand_limit_cents = on_demand_limit.map(as_cents).filter(|limit| *limit > 0);
    if !is_ultra && (included_used.is_some() || on_demand_used_cents.is_some()) {
        let total_limit_cents = included_limit
            .unwrap_or(CURSOR_PRO_INCLUDED_FALLBACK_CENTS)
            .saturating_add(on_demand_limit_cents.unwrap_or(CURSOR_ON_DEMAND_FALLBACK_CENTS));
        let total_used_cents = included_used
            .unwrap_or_default()
            .saturating_add(on_demand_used_cents.unwrap_or_default());
        quotas.clear();
        quotas.push(QuotaWindow::from_used_percent(
            format!("Cursor 总额度 · ${}", total_limit_cents / 100),
            total_used_cents as f64 * 100.0 / total_limit_cents as f64,
            None,
            billing_end(dashboard, summary),
        ));
    }
    let cost = if included_used.is_some()
        || included_limit.is_some()
        || on_demand_used_cents.is_some()
        || on_demand_limit_cents.is_some()
        || on_demand_enabled.is_some()
    {
        Some(CostUsage {
            currency: "USD".into(),
            included_used_cents: included_used,
            included_limit_cents: included_limit,
            on_demand_used_cents,
            on_demand_limit_cents,
            on_demand_enabled,
            period_end: billing_end(dashboard, summary),
        })
    } else {
        None
    };
    let tokens = auth_usage
        .and_then(parse_auth_token_usage)
        .or_else(|| summary.and_then(parse_token_usage));
    if quotas.is_empty() && cost.is_none() && tokens.is_none() {
        return Err("Cursor 响应中没有可识别的用量字段".into());
    }
    Ok(ProviderSnapshot {
        provider: Provider::Cursor,
        status: ProviderStatus::Available,
        plan,
        captured_at: Utc::now(),
        quotas,
        tokens,
        cost,
        stale: false,
        message: None,
    })
}

fn resolved_cursor_plan(plan: Option<String>, summary: Option<&Value>) -> Option<String> {
    let summary_plan = summary.and_then(|summary| {
        [
            "membershipType",
            "stripeMembershipType",
            "subscriptionType",
            "plan",
            "planName",
        ]
        .iter()
        .find_map(|key| summary.get(*key)?.as_str())
        .map(|value| value.to_ascii_lowercase())
    });
    summary_plan.or(plan)
}

fn find_request_quota(value: &Value) -> Option<(f64, f64)> {
    let object = value.as_object()?;
    let mut fallback = None;
    for (key, raw) in object {
        let Some(nested) = raw.as_object() else {
            continue;
        };
        let Some(used) = first_number(nested, &["numRequests", "used", "requests"]) else {
            continue;
        };
        let Some(maximum) = first_number(
            nested,
            &["maxRequestUsage", "limit", "maxRequests", "requestLimit"],
        ) else {
            continue;
        };
        if maximum <= 0.0 {
            continue;
        }
        if key.to_ascii_lowercase().contains("premium")
            || key.to_ascii_lowercase().contains("gpt-4")
        {
            return Some((used, maximum));
        }
        fallback.get_or_insert((used, maximum));
    }
    fallback
}

fn first_number(object: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| number(object.get(*key)))
}

fn pool_used_percent(
    usage: &Map<String, Value>,
    percent_key: &str,
    spend_key: &str,
    limit_key: &str,
) -> Option<f64> {
    number(usage.get(percent_key)).or_else(|| {
        let spend = number(usage.get(spend_key))?;
        let limit = number(usage.get(limit_key))?;
        (limit > 0.0).then_some(spend * 100.0 / limit)
    })
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.parse().ok(),
        _ => None,
    }
}

fn as_cents(value: f64) -> u64 {
    value.max(0.0).round() as u64
}

fn on_demand(summary: Option<&Value>, dashboard: Option<&Value>) -> (Option<f64>, Option<f64>) {
    let dashboard_usage = dashboard.and_then(|value| value.get("spendLimitUsage"));
    let summary_usage = summary.and_then(|value| value.pointer("/individualUsage/onDemand"));
    let used = dashboard_usage
        .and_then(|value| number(value.get("individualUsed")))
        .or_else(|| summary_usage.and_then(|value| number(value.get("used"))));
    let limit = dashboard_usage
        .and_then(|value| number(value.get("individualLimit")))
        .or_else(|| dashboard_usage.and_then(|value| number(value.get("limit"))))
        .or_else(|| summary_usage.and_then(|value| number(value.get("limit"))));
    (used, limit)
}

fn ultra_on_demand_enabled(
    is_ultra: bool,
    summary: Option<&Value>,
    dashboard: Option<&Value>,
    hard_limit: Option<&Value>,
) -> Option<bool> {
    if !is_ultra {
        return None;
    }
    if let Some(hard_limit) = hard_limit {
        let disabled = hard_limit
            .get("noUsageBasedAllowed")
            .and_then(Value::as_bool)
            == Some(true)
            || hard_limit
                .get("onDemandSpendDisabledByOrganization")
                .and_then(Value::as_bool)
                == Some(true);
        return Some(!disabled);
    }
    if let Some(enabled) = summary
        .and_then(|value| value.pointer("/individualUsage/onDemand/enabled"))
        .and_then(Value::as_bool)
    {
        return Some(enabled);
    }
    if let Some(limit) = dashboard
        .and_then(|value| value.get("spendLimitUsage"))
        .and_then(|value| number(value.get("individualLimit")))
    {
        return Some(limit > 0.0);
    }
    None
}

fn billing_end(dashboard: Option<&Value>, summary: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = dashboard
        .and_then(|value| value.get("billingCycleEnd"))
        .or_else(|| summary.and_then(|value| value.get("billingCycleEnd")))?;
    timestamp_value(Some(raw))
}

fn timestamp_value(raw: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = raw?;
    if let Some(text) = raw.as_str()
        && let Ok(date) = DateTime::parse_from_rfc3339(text)
    {
        return Some(date.with_timezone(&Utc));
    }
    let timestamp = raw
        .as_i64()
        .or_else(|| raw.as_str()?.parse().ok())
        .or_else(|| {
            let seconds = raw.get("seconds")?;
            seconds
                .as_i64()
                .or_else(|| seconds.as_str()?.parse().ok())
        })?;
    let seconds = if timestamp > 10_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    };
    Utc.timestamp_opt(seconds, 0).single()
}

fn parse_token_usage(summary: &Value) -> Option<TokenUsage> {
    let usage = summary
        .get("tokenUsage")
        .or_else(|| summary.get("tokens"))?;
    Some(TokenUsage {
        today: usage.get("today").and_then(Value::as_u64),
        lifetime: usage
            .get("lifetime")
            .or_else(|| usage.get("total"))
            .and_then(Value::as_u64),
        peak_daily: usage.get("peakDaily").and_then(Value::as_u64),
    })
}
fn parse_auth_token_usage(auth_usage: &Value) -> Option<TokenUsage> {
    let mut found = false;
    let lifetime = auth_usage
        .as_object()?
        .values()
        .filter_map(Value::as_object)
        .filter_map(|bucket| number(bucket.get("numTokens")))
        .filter(|tokens| tokens.is_finite() && *tokens >= 0.0)
        .map(|tokens| {
            found = true;
            tokens.round() as u64
        })
        .fold(0_u64, u64::saturating_add);
    found.then_some(TokenUsage {
        today: None,
        lifetime: Some(lifetime),
        peak_daily: None,
    })
}

fn extract_cursor_user_id(access_token: &str) -> Option<String> {
    let payload = access_token.split('.').nth(1)?.trim_end_matches('=');
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    ["sub", "userId", "user_id"]
        .iter()
        .filter_map(|key| claims.get(*key).and_then(Value::as_str))
        .find_map(cursor_user_id_in)
}

fn cursor_user_id_in(value: &str) -> Option<String> {
    let start = value.find("user_")?;
    let id = value[start..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>();
    (id.len() > "user_".len()).then_some(id)
}

fn cursor_session_cookie(user_id: &str, access_token: &str) -> String {
    format!("WorkosCursorSessionToken={user_id}%3A%3A{access_token}")
}

fn timestamp_millis(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(raw) => raw.parse::<i64>().ok().or_else(|| {
            DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|date| date.timestamp_millis())
        }),
        _ => None,
    }
}

fn billing_cycle_range(dashboard: Option<&Value>, summary: Option<&Value>) -> Option<(i64, i64)> {
    [dashboard, summary]
        .into_iter()
        .flatten()
        .find_map(|value| {
            let start = timestamp_millis(value.get("billingCycleStart"))?;
            let end = timestamp_millis(value.get("billingCycleEnd"))?;
            (start < end).then_some((start, end))
        })
}

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use reqwest::{blocking::Client, redirect::Policy};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use zeroize::Zeroizing;

use super::{
    Detection, UsageProvider,
    discovery::{ResolvedExecutable, command_for, find_executable_with_override},
};

const CURSOR_CLI_ACCOUNT_TIMEOUT: Duration = Duration::from_secs(4);
const CURSOR_ACCOUNT_PROTOCOLS: &[(&str, &[&str])] = &[
    ("status", &["status"]),
    ("status_json", &["status", "--format", "json"]),
    ("about_json", &["about", "--format", "json"]),
    ("about", &["about"]),
];

#[derive(Debug, Clone)]
pub struct CursorProvider {
    compat_enabled: Arc<AtomicBool>,
    cached_executable: Arc<Mutex<Option<ResolvedExecutable>>>,
}

impl CursorProvider {
    pub fn new(compat_enabled: Arc<AtomicBool>) -> Self {
        Self {
            compat_enabled,
            cached_executable: Arc::new(Mutex::new(None)),
        }
    }

    fn executable(&self) -> Option<ResolvedExecutable> {
        if let Ok(cache) = self.cached_executable.lock()
            && let Some(executable) = cache.as_ref().filter(|value| value.path().is_file())
        {
            diagnostics::info(
                "cli.discovery.cache_hit",
                format!("provider=cursor path={}", executable.path().display()),
            );
            return Some(executable.clone());
        }
        let discovered = cursor_agent_executable();
        if let Some(path) = discovered.as_ref()
            && let Ok(mut cache) = self.cached_executable.lock()
        {
            *cache = Some(path.clone());
        }
        discovered
    }

    fn cli_account(&self) -> Result<(PathBuf, Option<String>), String> {
        let executable = self
            .executable()
            .ok_or_else(|| "未检测到 Cursor Agent CLI".to_string())?;
        let deadline = Instant::now() + CURSOR_CLI_ACCOUNT_TIMEOUT;

        for (protocol, args) in CURSOR_ACCOUNT_PROTOCOLS {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Cursor CLI 响应超时".into());
            }
            diagnostics::info("cursor.cli.protocol", format!("name={protocol}"));
            let output = run_command(command_for(&executable, args), remaining)?;
            let combined = output.combined();

            if output.success {
                match Self::parse_cursor_status(&combined) {
                    Ok(plan) => return Ok((executable.path().to_path_buf(), plan)),
                    Err(message) if message.contains("未返回状态") => continue,
                    Err(message) => return Err(message),
                }
            }

            if Self::looks_logged_out(&combined) {
                return Err("Cursor CLI 尚未登录".into());
            }
            if is_cli_protocol_mismatch(&combined) {
                diagnostics::info(
                    "cursor.cli.protocol_unsupported",
                    format!(
                        "name={protocol} exit_code={}",
                        output.exit_code.unwrap_or(-1)
                    ),
                );
                continue;
            }
            return Err("Cursor CLI 无法读取账户状态".into());
        }

        Err("当前 Cursor CLI 未提供可识别的账户状态协议".into())
    }

    fn parse_cursor_status(output: &[u8]) -> Result<Option<String>, String> {
        if let Ok(json) = serde_json::from_slice::<Value>(output) {
            if json_bool_field(
                &json,
                &["authenticated", "isAuthenticated", "loggedIn", "isLoggedIn"],
            ) == Some(false)
            {
                return Err("Cursor CLI 尚未登录".into());
            }
            let plan = json_string_field(
                &json,
                &[
                    "subscriptionTier",
                    "subscriptionType",
                    "plan",
                    "planName",
                    "tier",
                ],
            )
            .map(|value| value.to_ascii_lowercase());
            return Ok(plan);
        }

        let text = String::from_utf8_lossy(output);
        let normalized = text.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err("Cursor CLI 未返回状态".into());
        }
        if Self::looks_logged_out(output) {
            return Err("Cursor CLI 尚未登录".into());
        }
        let plan = normalized.lines().find_map(|line| {
            if !line.contains("plan") && !line.contains("subscription") && !line.contains("tier") {
                return None;
            }
            line.split(|character: char| !character.is_ascii_alphanumeric())
                .find(|word| {
                    matches!(
                        *word,
                        "free"
                            | "hobby"
                            | "plus"
                            | "pro"
                            | "ultra"
                            | "team"
                            | "business"
                            | "enterprise"
                    )
                })
                .map(str::to_owned)
        });
        Ok(plan)
    }

    fn looks_logged_out(output: &[u8]) -> bool {
        let normalized = String::from_utf8_lossy(output).to_ascii_lowercase();
        [
            "not authenticated",
            "not logged in",
            "logged out",
            "unauthenticated",
            "\"authenticated\":false",
            "\"authenticated\": false",
            "\"isAuthenticated\":false",
            "\"isAuthenticated\": false",
            "\"loggedIn\":false",
            "\"loggedIn\": false",
        ]
        .iter()
        .any(|marker| normalized.contains(&marker.to_ascii_lowercase()))
    }

    fn collect_compat(&self, plan: Option<String>) -> Result<ProviderSnapshot, String> {
        let token = Zeroizing::new(read_access_token()?);
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| "无法初始化 Cursor 网络客户端".to_string())?;
        let auth_result = request_json(
            &client,
            "GET",
            "https://api2.cursor.sh/auth/usage",
            CursorRequestAuth::Bearer(&token),
            None,
        );
        let auth_error = auth_result.as_ref().err().cloned();
        let auth = auth_result.ok();
        let dashboard_result = request_json(
            &client,
            "POST",
            "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
            CursorRequestAuth::Bearer(&token),
            Some(serde_json::json!({})),
        );
        let dashboard_error = dashboard_result.as_ref().err().cloned();
        let dashboard = dashboard_result.ok();
        let session_cookie =
            extract_cursor_user_id(&token)
                .map(|user_id| Zeroizing::new(cursor_session_cookie(&user_id, &token)));
        let summary_result = session_cookie.as_deref().map(|cookie| {
            request_json(
                &client,
                "GET",
                "https://cursor.com/api/usage-summary",
                CursorRequestAuth::SessionCookie(cookie),
                None,
            )
        });
        let summary_error = summary_result
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .cloned();
        let summary = summary_result.and_then(Result::ok);
        let plan = resolved_cursor_plan(plan, summary.as_ref());
        let is_ultra = plan
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("ultra"));
        let sand = is_ultra
            .then(|| {
                request_json(
                    &client,
                    "POST",
                    "https://api2.cursor.sh/aiserver.v1.DashboardService/GetSandUsageStatus",
                    CursorRequestAuth::Bearer(&token),
                    Some(serde_json::json!({})),
                )
                .ok()
            })
            .flatten();
        let hard_limit = is_ultra
            .then(|| {
                request_json(
                    &client,
                    "POST",
                    "https://api2.cursor.sh/aiserver.v1.DashboardService/GetHardLimit",
                    CursorRequestAuth::Bearer(&token),
                    Some(serde_json::json!({})),
                )
                .ok()
            })
            .flatten();
        if auth.is_none() && summary.is_none() && dashboard.is_none() {
            let reasons = [
                auth_error.map(|error| format!("账户用量：{error}")),
                dashboard_error.map(|error| format!("当前周期：{error}")),
                summary_error.map(|error| format!("用量摘要：{error}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            return Err(if reasons.is_empty() {
                "Cursor 用量接口不可用或登录已失效".into()
            } else {
                format!("Cursor 用量接口不可用（{}）", reasons.join("；"))
            });
        }
        let on_demand_events_cents = (!is_ultra)
            .then(|| {
                session_cookie.as_deref().and_then(|cookie| {
                    let range = billing_cycle_range(dashboard.as_ref(), summary.as_ref())?;
                    sum_on_demand_events(&client, cookie, range).ok()
                })
            })
            .flatten();
        snapshot_from_payloads_with_on_demand(
            auth.as_ref(),
            summary.as_ref(),
            dashboard.as_ref(),
            sand.as_ref(),
            hard_limit.as_ref(),
            plan,
            on_demand_events_cents,
        )
    }
}

impl UsageProvider for CursorProvider {
    fn detect(&self) -> Detection {
        match self.executable() {
            Some(executable) => Detection {
                executable: Some(executable.path().to_path_buf()),
                status: ProviderStatus::Available,
                detail: None,
            },
            None => {
                let (status, detail) = cursor_agent_missing_state(cursor_desktop_installed());
                Detection {
                    executable: None,
                    status,
                    detail: Some(detail.into()),
                }
            }
        }
    }

    fn account_status(&self) -> ProviderSnapshot {
        match self.cli_account() {
            Ok((_, plan)) => ProviderSnapshot {
                provider: Provider::Cursor,
                status: ProviderStatus::Available,
                plan,
                captured_at: Utc::now(),
                quotas: Vec::new(),
                tokens: None,
                cost: None,
                stale: false,
                message: Some("开启个人兼容模式后可读取精确用量".into()),
            },
            Err(_) if self.executable().is_none() => {
                let (status, message) = cursor_agent_missing_state(cursor_desktop_installed());
                ProviderSnapshot::unavailable(Provider::Cursor, status, message)
            }
            Err(message) if message.contains("未登录") => ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::NotLoggedIn,
                message,
            ),
            Err(message) => ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::Unsupported,
                format!("已检测到 Cursor CLI，但状态不可用：{message}"),
            ),
        }
    }

    fn refresh(&self) -> ProviderSnapshot {
        if !self.compat_enabled.load(Ordering::Relaxed) {
            return self.account_status();
        }
        let plan = self.cli_account().ok().and_then(|(_, plan)| plan);
        match self.collect_compat(plan) {
            Ok(snapshot) => snapshot,
            Err(message) if message.contains("登录") => ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::NotLoggedIn,
                message,
            ),
            Err(message) if message.contains("接口") => ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::NetworkError,
                message,
            ),
            Err(message) => ProviderSnapshot::unavailable(
                Provider::Cursor,
                ProviderStatus::ProtocolError,
                message,
            ),
        }
    }
}

#[cfg(windows)]
fn push_cursor_versioned_paths(paths: &mut Vec<PathBuf>, versions_root: &Path) {
    let Ok(entries) = std::fs::read_dir(versions_root) else {
        return;
    };
    let mut version_roots = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    version_roots.sort();
    version_roots.reverse();
    for version_root in version_roots {
        for root in [version_root.clone(), version_root.join("bin")] {
            paths.push(root.join("cursor-agent"));
            paths.push(root.join("agent"));
        }
    }
}

pub(crate) fn cursor_agent_executable() -> Option<ResolvedExecutable> {
    find_executable_with_override(
        "METRA_CURSOR_PATH",
        &["cursor-agent", "agent"],
        &cursor_known_paths(),
    )
}

pub(crate) fn cursor_login_executable() -> Option<ResolvedExecutable> {
    cursor_agent_executable().filter(|executable| is_cursor_login_executable(executable.path()))
}

fn is_cursor_login_executable(path: &Path) -> bool {
    path.file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("cursor-agent"))
}

fn cursor_known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let roots = vec![home.join(".local/bin"), home.join(".cursor/bin")];
        #[cfg(windows)]
        let roots = {
            let mut roots = roots;
            roots.extend([
                home.join("scoop/shims"),
                home.join(".bun/bin"),
                home.join(".cargo/bin"),
            ]);
            roots
        };
        for root in roots {
            paths.push(root.join("cursor-agent"));
            paths.push(root.join("agent"));
        }
        #[cfg(target_os = "macos")]
        paths.push(home.join(
            "Library/Application Support/Cursor/User/globalStorage/anysphere.cursor-agent-worker/agent-cli/.local/bin/cursor-agent",
        ));
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            for root in [
                local.join("cursor-agent"),
                local.join("Microsoft/WinGet/Links"),
                local.join("Programs/cursor/resources/app/bin"),
                local.join("Programs/cursor-agent"),
                local.join("pnpm"),
            ] {
                paths.push(root.join("cursor-agent"));
                paths.push(root.join("agent"));
            }
            push_cursor_versioned_paths(&mut paths, &local.join("cursor-agent/versions"));
        }
        if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
            for root in [roaming.join("npm"), roaming.join("pnpm")] {
                paths.push(root.join("cursor-agent"));
                paths.push(root.join("agent"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for root in [
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ] {
            paths.push(root.join("cursor-agent"));
            paths.push(root.join("agent"));
        }
    }
    paths
}

fn cursor_desktop_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "macos")]
        paths.push(home.join("Applications/Cursor.app/Contents/MacOS/Cursor"));
        #[cfg(all(not(windows), not(target_os = "macos")))]
        paths.extend([
            home.join(".local/share/applications/cursor.desktop"),
            home.join("Applications/cursor.AppImage"),
        ]);
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        paths.extend([
            local.join("Programs/cursor/Cursor.exe"),
            local.join("Programs/Cursor/Cursor.exe"),
        ]);
    }
    #[cfg(target_os = "macos")]
    paths.push(PathBuf::from(
        "/Applications/Cursor.app/Contents/MacOS/Cursor",
    ));
    #[cfg(all(not(windows), not(target_os = "macos")))]
    paths.extend([
        PathBuf::from("/usr/share/applications/cursor.desktop"),
        PathBuf::from("/opt/Cursor/cursor"),
    ]);
    paths
}

fn cursor_desktop_installed_from(app_candidates: &[PathBuf]) -> bool {
    app_candidates.iter().any(|path| path.is_file())
}

fn cursor_desktop_installed() -> bool {
    cursor_desktop_installed_from(&cursor_desktop_candidates())
}

fn cursor_agent_missing_state(desktop_installed: bool) -> (ProviderStatus, &'static str) {
    if desktop_installed {
        (
            ProviderStatus::DesktopInstalled,
            "Cursor 桌面版已安装；未检测到独立 Agent CLI，可开启个人兼容模式读取用量",
        )
    } else {
        (
            ProviderStatus::NotInstalled,
            "未检测到 Cursor Agent CLI 或 Cursor 桌面环境",
        )
    }
}

fn cursor_state_db() -> Result<PathBuf, String> {
    #[cfg(not(windows))]
    let home = dirs::home_dir().ok_or_else(|| "无法确定用户目录".to_string())?;
    #[cfg(windows)]
    {
        let appdata = std::env::var_os("APPDATA").ok_or_else(|| "APPDATA 未设置".to_string())?;
        Ok(PathBuf::from(appdata).join("Cursor/User/globalStorage/state.vscdb"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        Ok(home.join(".config/Cursor/User/globalStorage/state.vscdb"))
    }
}

fn read_access_token() -> Result<String, String> {
    read_access_token_from(&cursor_state_db()?)
}

fn read_access_token_from(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err("未找到 Cursor 本地状态库".into());
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "无法只读打开 Cursor 状态库".to_string())?;
    let mut statement = connection
        .prepare("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1")
        .map_err(|_| "Cursor 状态库结构不兼容".to_string())?;
    let token = statement
        .query_row(["cursorAuth/accessToken"], |row| match row.get_ref(0)? {
            ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
                Ok(String::from_utf8_lossy(bytes).into_owned())
            }
            _ => Ok(String::new()),
        })
        .map_err(|_| "Cursor 登录令牌不存在".to_string())?;
    if token.is_empty() {
        Err("Cursor 登录令牌不存在".into())
    } else {
        Ok(token)
    }
}

fn allowed_cursor_url(url: &str) -> bool {
    url::Url::parse(url).ok().is_some_and(|url| {
        url.scheme() == "https" && matches!(url.host_str(), Some("api2.cursor.sh" | "cursor.com"))
    })
}

#[derive(Clone, Copy)]
enum CursorRequestAuth<'a> {
    Bearer(&'a str),
    SessionCookie(&'a str),
}

fn sum_on_demand_events(
    client: &Client,
    session_cookie: &str,
    (start, end): (i64, i64),
) -> Result<u64, String> {
    const PAGE_SIZE: u64 = 200;
    const MAX_PAGES: u64 = 40;
    let mut page = 1;
    let mut scanned = 0_u64;
    let mut total = u64::MAX;
    let mut charged_cents = 0.0;

    while scanned < total && page <= MAX_PAGES {
        let response = request_json(
            client,
            "POST",
            "https://cursor.com/api/dashboard/get-filtered-usage-events",
            CursorRequestAuth::SessionCookie(session_cookie),
            Some(serde_json::json!({
                "startDate": start,
                "endDate": end,
                "page": page,
                "pageSize": PAGE_SIZE,
            })),
        )?;
        total = response
            .get("totalUsageEventsCount")
            .and_then(Value::as_u64)
            .unwrap_or(total);
        let Some(events) = response.get("usageEventsDisplay").and_then(Value::as_array) else {
            return Err("Cursor 用量事件响应结构不兼容".into());
        };
        if events.is_empty() {
            break;
        }
        for event in events {
            scanned = scanned.saturating_add(1);
            if event.get("kind").and_then(Value::as_str) != Some("USAGE_EVENT_KIND_USAGE_BASED") {
                continue;
            }
            if let Some(cents) = number(event.get("chargedCents")) {
                charged_cents += cents;
            }
        }
        if events.len() < PAGE_SIZE as usize {
            break;
        }
        page += 1;
    }

    if !charged_cents.is_finite() || charged_cents < 0.0 {
        return Err("Cursor 用量事件金额无效".into());
    }
    Ok(charged_cents.round() as u64)
}

fn request_json(
    client: &Client,
    method: &str,
    url: &str,
    auth: CursorRequestAuth<'_>,
    body: Option<Value>,
) -> Result<Value, String> {
    if !allowed_cursor_url(url) {
        return Err("拒绝访问非 Cursor 官方域名".into());
    }
    let request = if method == "POST" {
        client
            .post(url)
            .json(&body.unwrap_or_else(|| serde_json::json!({})))
    } else {
        client.get(url)
    };
    let request = match auth {
        CursorRequestAuth::Bearer(token) => request
            .bearer_auth(token)
            .header("Connect-Protocol-Version", "1"),
        CursorRequestAuth::SessionCookie(cookie) => request
            .header(reqwest::header::COOKIE, cookie)
            .header(reqwest::header::ORIGIN, "https://cursor.com")
            .header(reqwest::header::REFERER, "https://cursor.com/dashboard"),
    };
    let response = request
        .send()
        .map_err(|_| "Cursor 网络请求失败".to_string())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Cursor 登录已失效".into());
    }
    if !response.status().is_success() {
        return Err(format!(
            "Cursor 接口返回 HTTP {}",
            response.status().as_u16()
        ));
    }
    response
        .json()
        .map_err(|_| "Cursor 接口返回了无效 JSON".into())
}

fn json_bool_field(value: &Value, fields: &[&str]) -> Option<bool> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if fields.iter().any(|field| key.eq_ignore_ascii_case(field))
                    && let Some(value) = child.as_bool()
                {
                    return Some(value);
                }
            }
            object
                .values()
                .find_map(|child| json_bool_field(child, fields))
        }
        Value::Array(array) => array
            .iter()
            .find_map(|child| json_bool_field(child, fields)),
        _ => None,
    }
}

fn json_string_field<'a>(value: &'a Value, fields: &[&str]) -> Option<&'a str> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if fields.iter().any(|field| key.eq_ignore_ascii_case(field))
                    && let Some(value) = child.as_str()
                {
                    return Some(value);
                }
            }
            object
                .values()
                .find_map(|child| json_string_field(child, fields))
        }
        Value::Array(array) => array
            .iter()
            .find_map(|child| json_string_field(child, fields)),
        _ => None,
    }
}
#[derive(Debug)]
struct CliCommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<i32>,
}

impl CliCommandOutput {
    fn combined(&self) -> Vec<u8> {
        let mut combined = Vec::with_capacity(self.stdout.len() + self.stderr.len() + 1);
        combined.extend_from_slice(&self.stdout);
        if !self.stdout.is_empty() && !self.stderr.is_empty() {
            combined.push(b'\n');
        }
        combined.extend_from_slice(&self.stderr);
        combined
    }
}

fn is_cli_protocol_mismatch(output: &[u8]) -> bool {
    let normalized = String::from_utf8_lossy(output).to_ascii_lowercase();
    [
        "unknown command",
        "unrecognized command",
        "invalid command",
        "unknown option",
        "unrecognized option",
        "unexpected argument",
        "invalid argument",
        "found argument",
        "no such command",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn run_command(mut command: Command, timeout: Duration) -> Result<CliCommandOutput, String> {
    hide_cursor_window(&mut command);
    diagnostics::info(
        "cursor.cli.start",
        format!("timeout_ms={}", timeout.as_millis()),
    );
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| {
            diagnostics::warn(
                "cursor.cli.spawn_failed",
                format!("kind={:?}", error.kind()),
            );
            "无法启动 Cursor CLI".to_string()
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|_| "无法读取 Cursor CLI 输出".to_string())?;
                diagnostics::info(
                    "cursor.cli.complete",
                    format!(
                        "exit_code={} stdout_bytes={} stderr_bytes={}",
                        status.code().unwrap_or(-1),
                        output.stdout.len(),
                        output.stderr.len()
                    ),
                );
                return Ok(CliCommandOutput {
                    success: status.success(),
                    stdout: output.stdout,
                    stderr: output.stderr,
                    exit_code: status.code(),
                });
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                diagnostics::warn(
                    "cursor.cli.timeout",
                    format!("timeout_ms={}", timeout.as_millis()),
                );
                return Err("Cursor CLI 响应超时".into());
            }
            Err(error) => {
                diagnostics::warn(
                    "cursor.cli.status_failed",
                    format!("kind={:?}", error.kind()),
                );
                return Err("无法查询 Cursor CLI 状态".into());
            }
        }
    }
}
fn hide_cursor_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod security_tests {
    use super::{
        allowed_cursor_url, cursor_session_cookie, extract_cursor_user_id,
        snapshot_from_payloads_with_on_demand,
    };
    #[cfg(windows)]
    use super::cursor_known_paths;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::json;
    #[cfg(windows)]
    use std::path::PathBuf;
    #[test]
    fn only_cursor_https_hosts_are_allowed() {
        assert!(allowed_cursor_url("https://api2.cursor.sh/auth/usage"));
        assert!(allowed_cursor_url("https://cursor.com/api/usage-summary"));
        assert!(!allowed_cursor_url("http://api2.cursor.sh/auth/usage"));
        assert!(!allowed_cursor_url("https://cursor.com.evil.test/api"));
    }
    #[test]
    fn dashboard_session_cookie_uses_user_id_from_access_token() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"auth0|user_abc123"}"#);
        let token = format!("header.{payload}.signature");
        let user_id = extract_cursor_user_id(&token).unwrap();
        assert_eq!(user_id, "user_abc123");
        assert_eq!(
            cursor_session_cookie(&user_id, &token),
            format!("WorkosCursorSessionToken=user_abc123%3A%3A{token}")
        );
    }
    #[test]
    fn event_sum_overrides_stale_on_demand_shortcuts() {
        let snapshot = snapshot_from_payloads_with_on_demand(
            None,
            Some(&json!({"individualUsage": {"onDemand": {"used": 321}}})),
            Some(&json!({
                "planUsage": {"limit": 2000, "includedSpend": 750},
                "spendLimitUsage": {"individualUsed": 321}
            })),
            None,
            None,
            Some("pro".into()),
            Some(1_738),
        )
        .unwrap();
        assert_eq!(snapshot.cost.unwrap().on_demand_used_cents, Some(1_738));
    }
    #[test]
    fn cursor_com_summary_is_an_included_spend_fallback() {
        let snapshot = snapshot_from_payloads_with_on_demand(
            None,
            Some(&json!({
                "billingCycleEnd": "2099-01-31T00:00:00Z",
                "individualUsage": {
                    "plan": {"used": 850, "limit": 2000},
                    "onDemand": {"used": 125, "limit": 50000}
                }
            })),
            None,
            None,
            None,
            Some("pro".into()),
            None,
        )
        .unwrap();
        let cost = snapshot.cost.unwrap();
        assert_eq!(cost.included_used_cents, Some(850));
        assert_eq!(cost.included_limit_cents, Some(2_000));
        assert_eq!(cost.on_demand_used_cents, Some(125));
        assert!(cost.period_end.is_some());
    }
    #[cfg(windows)]
    #[test]
    fn discovery_covers_both_cursor_user_install_roots() {
        let home = dirs::home_dir().unwrap();
        let paths = cursor_known_paths();
        assert!(paths.contains(&home.join(".local/bin/cursor-agent")));
        assert!(paths.contains(&home.join(".cursor/bin/cursor-agent")));
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        let roaming = PathBuf::from(std::env::var_os("APPDATA").unwrap());
        assert!(paths.contains(&local.join("pnpm/cursor-agent")));
        assert!(paths.contains(&roaming.join("pnpm/cursor-agent")));
        assert!(paths.contains(&home.join(".bun/bin/cursor-agent")));
    }
}

#[cfg(all(test, windows))]
mod cli_boundary_tests {
    use super::*;

    #[test]
    fn account_probe_uses_documented_status_command_without_json_flags() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nif \"%1\"==\"status\" (\r\n  echo Logged in as test@example.com\r\n  echo Subscription: Pro\r\n  exit /b 0\r\n)\r\nexit /b 9\r\n",
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let started = Instant::now();
        let (_, plan) = provider.cli_account().unwrap();
        assert_eq!(plan.as_deref(), Some("pro"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn one_refresh_runs_at_most_one_cli_status_probe() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        let counter = temp.path().join("calls.txt");
        std::fs::write(
            &launcher,
            format!(
                "@echo off\r\necho call>>\"{}\"\r\nexit /b 9\r\n",
                counter.display()
            ),
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let _ = provider.refresh();
        let calls = std::fs::read_to_string(counter).unwrap();
        assert_eq!(calls.lines().count(), 1);
    }
    #[test]
    fn account_probe_falls_back_to_legacy_about_json_protocol() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        let counter = temp.path().join("calls.txt");
        std::fs::write(
            &launcher,
            format!(
                "@echo off\r\necho %*>>\"{}\"\r\nif \"%1 %2 %3\"==\"about --format json\" (\r\n  echo {{\"authenticated\":true,\"subscriptionTier\":\"Pro\"}}\r\n  exit /b 0\r\n)\r\necho error: unknown command %1 1>&2\r\nexit /b 2\r\n",
                counter.display()
            ),
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let started = Instant::now();
        let (_, plan) = provider.cli_account().unwrap();
        assert_eq!(plan.as_deref(), Some("pro"));
        assert!(started.elapsed() < Duration::from_secs(2));
        let calls = std::fs::read_to_string(counter).unwrap();
        assert!(calls.lines().any(|line| line == "status"));
        assert!(calls.lines().any(|line| line == "about --format json"));
    }

    #[test]
    fn account_probe_accepts_success_output_written_to_stderr() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nif \"%1\"==\"status\" (\r\n  echo Logged in as test@example.com 1>&2\r\n  echo Subscription: Team 1>&2\r\n  exit /b 0\r\n)\r\nexit /b 2\r\n",
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let (_, plan) = provider.cli_account().unwrap();
        assert_eq!(plan.as_deref(), Some("team"));
    }
    #[test]
    fn account_probe_supports_status_json_protocol() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nif \"%1 %2 %3\"==\"status --format json\" (\r\n  echo {\"authenticated\":true,\"plan\":\"Business\"}\r\n  exit /b 0\r\n)\r\necho error: unknown command or option 1>&2\r\nexit /b 2\r\n",
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let (_, plan) = provider.cli_account().unwrap();
        assert_eq!(plan.as_deref(), Some("business"));
    }

    #[test]
    fn account_probe_supports_plain_legacy_about_protocol() {
        let temp = tempfile::tempdir().unwrap();
        let launcher = temp.path().join("cursor-agent.cmd");
        std::fs::write(
            &launcher,
            "@echo off\r\nif \"%1\"==\"about\" if \"%2\"==\"\" (\r\n  echo Authentication: logged in\r\n  echo Plan: Enterprise\r\n  exit /b 0\r\n)\r\necho error: unknown command or option 1>&2\r\nexit /b 2\r\n",
        )
        .unwrap();
        let provider = CursorProvider {
            compat_enabled: Arc::new(AtomicBool::new(false)),
            cached_executable: Arc::new(Mutex::new(Some(ResolvedExecutable::from_path(launcher)))),
        };

        let (_, plan) = provider.cli_account().unwrap();
        assert_eq!(plan.as_deref(), Some("enterprise"));
    }

    #[test]
    fn status_parser_supports_nested_legacy_json_fields() {
        let output = br#"{"account":{"loggedIn":true,"subscription":{"tier":"Business"}}}"#;
        let plan = CursorProvider::parse_cursor_status(output).unwrap();
        assert_eq!(plan.as_deref(), Some("business"));
    }

    #[test]
    fn status_parser_recognizes_legacy_logged_out_json_fields() {
        for output in [
            br#"{"isAuthenticated":false}"#.as_slice(),
            br#"{"account":{"loggedIn":false}}"#.as_slice(),
        ] {
            assert_eq!(
                CursorProvider::parse_cursor_status(output).unwrap_err(),
                "Cursor CLI 尚未登录"
            );
        }
    }
}
#[cfg(test)]
mod login_executable_tests {
    use super::is_cursor_login_executable;
    use std::path::Path;

    #[test]
    fn login_side_effect_is_limited_to_the_named_cursor_agent_launcher() {
        assert!(is_cursor_login_executable(Path::new("/tmp/cursor-agent")));
        assert!(is_cursor_login_executable(Path::new("/tmp/cursor-agent.cmd")));
        assert!(!is_cursor_login_executable(Path::new("/tmp/agent")));
        assert!(!is_cursor_login_executable(Path::new("/tmp/cursor")));
    }
}

#[cfg(test)]
mod desktop_detection_tests {
    use super::{cursor_agent_missing_state, cursor_desktop_installed_from};
    #[cfg(target_os = "macos")]
    use super::cursor_known_paths;
    use crate::model::ProviderStatus;

    #[test]
    fn installed_desktop_app_is_distinguished_from_agent_cli() {
        let temp = tempfile::tempdir().unwrap();
        let app = temp
            .path()
            .join("Applications/Cursor.app/Contents/MacOS/Cursor");
        std::fs::create_dir_all(app.parent().unwrap()).unwrap();
        std::fs::write(&app, []).unwrap();

        assert!(cursor_desktop_installed_from(&[app]));
        let (status, message) = cursor_agent_missing_state(true);
        assert_eq!(status, ProviderStatus::DesktopInstalled);
        assert!(message.contains("桌面版已安装"));
        assert!(message.contains("Agent CLI"));
        assert!(message.contains("兼容模式"));
    }

    #[test]
    fn missing_agent_and_desktop_remains_not_installed() {
        let temp = tempfile::tempdir().unwrap();
        let missing_app = temp
            .path()
            .join("Applications/Cursor.app/Contents/MacOS/Cursor");

        assert!(!cursor_desktop_installed_from(&[missing_app]));
        let (status, message) = cursor_agent_missing_state(false);
        assert_eq!(status, ProviderStatus::NotInstalled);
        assert!(message.contains("Agent CLI"));
        assert!(message.contains("桌面环境"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn discovery_covers_cursor_worker_agent_install() {
        let home = dirs::home_dir().unwrap();
        assert!(cursor_known_paths().contains(&home.join(
            "Library/Application Support/Cursor/User/globalStorage/anysphere.cursor-agent-worker/agent-cli/.local/bin/cursor-agent",
        )));
    }
}

#[cfg(test)]
mod sqlite_boundary_tests {
    use super::*;

    #[test]
    fn token_store_reads_only_the_exact_cursor_key() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.vscdb");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE ItemTable (key TEXT UNIQUE, value BLOB)", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                ("unrelated", "wrong"),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                ("cursorAuth/accessToken", "test-secret"),
            )
            .unwrap();
        drop(connection);
        assert_eq!(read_access_token_from(&path).unwrap(), "test-secret");
    }
}
