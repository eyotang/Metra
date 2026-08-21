use std::{
    collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher},
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, Local, NaiveDate, Utc};
use reqwest::{blocking::Client, redirect::Policy};
use serde::Deserialize;
use serde_json::Value;
use url::Url;
use zeroize::Zeroizing;

use crate::{
    diagnostics,
    model::{CostUsage, Provider, ProviderSnapshot, ProviderStatus, TokenUsage},
};

use super::{
    ANTHROPIC_ADMIN_KEY_ENV, CLAUDE_API_KEY_NAME_ENV, Detection, UsageProvider,
    discovery::{ResolvedExecutable, command_for, find_executable_with_override},
};

const CLAUDE_CODE_USAGE_URL: &str =
    "https://api.anthropic.com/v1/organizations/usage_report/claude_code";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const OFFICIAL_USAGE_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_USAGE_PAGES: usize = 100;
const MAX_USAGE_RECORDS: usize = 100_000;
const MAX_USAGE_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct ClaudeProvider {
    executable_override: Option<PathBuf>,
    cached_executable: Mutex<Option<ResolvedExecutable>>,
    usage_cache: Mutex<Option<LocalUsageCache>>,
    official_usage_cache: Mutex<Option<OfficialUsageCache>>,
    timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct LocalUsageCache {
    files: Vec<FileStamp>,
    local_date: NaiveDate,
    tokens: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OfficialDailyUsage {
    tokens: u64,
    estimated_cost_cents: u64,
    currency: String,
}

#[derive(Debug, Clone)]
struct OfficialUsageCache {
    utc_date: NaiveDate,
    api_key_name: Option<String>,
    admin_key_fingerprint: u64,
    fetched_at: Instant,
    captured_at: DateTime<Utc>,
    usage: OfficialDailyUsage,
}

#[derive(Debug, Clone)]
struct OfficialUsageResult {
    usage: OfficialDailyUsage,
    captured_at: DateTime<Utc>,
    stale_reason: Option<String>,
}

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self {
            executable_override: None,
            cached_executable: Mutex::new(None),
            usage_cache: Mutex::new(None),
            official_usage_cache: Mutex::new(None),
            timeout: Duration::from_secs(5),
        }
    }
}

impl ClaudeProvider {
    fn executable(&self) -> Option<ResolvedExecutable> {
        if let Some(path) = self
            .executable_override
            .as_ref()
            .filter(|path| path.is_file())
        {
            return Some(ResolvedExecutable::from_path(path.clone()));
        }
        if let Ok(cache) = self.cached_executable.lock()
            && let Some(executable) = cache.as_ref().filter(|value| value.path().is_file())
        {
            diagnostics::info(
                "cli.discovery.cache_hit",
                format!("provider=claude path={}", executable.path().display()),
            );
            return Some(executable.clone());
        }
        let discovered =
            find_executable_with_override("METRA_CLAUDE_PATH", &["claude"], &claude_known_paths());
        if let Some(path) = discovered.as_ref()
            && let Ok(mut cache) = self.cached_executable.lock()
        {
            *cache = Some(path.clone());
        }
        discovered
    }

    fn collect(&self) -> Result<ProviderSnapshot, String> {
        let executable = self
            .executable()
            .ok_or_else(|| "未检测到 Claude Code CLI".to_string())?;
        let auth_status = read_auth_status(&executable, self.timeout)?;
        let local_tokens = self.local_tokens();
        let mut snapshot = snapshot_from_auth_status(&auth_status, local_tokens.clone())?;
        if is_first_party_api_key_login(&auth_status) {
            self.enrich_with_official_usage(&mut snapshot, local_tokens);
        }
        Ok(snapshot)
    }

    fn enrich_with_official_usage(
        &self,
        snapshot: &mut ProviderSnapshot,
        local_tokens: Option<TokenUsage>,
    ) {
        let admin_key = match admin_key_from_environment() {
            Ok(Some(admin_key)) => admin_key,
            Ok(None) => {
                snapshot.message = Some(if local_tokens.is_some() {
                    "API Key 登录；当前显示本机会话 Token。配置 ANTHROPIC_ADMIN_KEY 后可读取官方用量"
                } else {
                    "API Key 登录；配置 ANTHROPIC_ADMIN_KEY 后可读取官方用量"
                }
                .into());
                return;
            }
            Err(message) => {
                snapshot.message = Some(message);
                return;
            }
        };
        let api_key_name = configured_api_key_name();
        match self.official_usage(&admin_key, Utc::now().date_naive(), api_key_name.as_deref()) {
            Ok(result) => {
                let local_lifetime = local_tokens.as_ref().and_then(|tokens| tokens.lifetime);
                let local_peak = local_tokens.as_ref().and_then(|tokens| tokens.peak_daily);
                snapshot.tokens = Some(TokenUsage {
                    today: Some(result.usage.tokens),
                    lifetime: local_lifetime,
                    peak_daily: local_peak,
                });
                snapshot.cost = Some(CostUsage {
                    currency: result.usage.currency,
                    today_used_cents: Some(result.usage.estimated_cost_cents),
                    included_used_cents: None,
                    included_limit_cents: None,
                    on_demand_used_cents: None,
                    on_demand_limit_cents: None,
                    on_demand_enabled: None,
                    period_end: None,
                });
                snapshot.captured_at = result.captured_at;
                snapshot.stale = result.stale_reason.is_some();
                snapshot.message = Some(match result.stale_reason {
                    Some(reason) => format!("官方用量刷新失败，显示缓存数据：{reason}"),
                    None if local_lifetime.is_some() => {
                        "官方 API 今日用量按 UTC 统计，最多延迟 1 小时；累计 Token 为本机会话统计"
                            .into()
                    }
                    None => "官方 API 今日用量按 UTC 统计，最多延迟 1 小时".into(),
                });
            }
            Err(message) => {
                snapshot.message = Some(if local_tokens.is_some() {
                    format!("官方用量不可用，继续显示本机会话 Token：{message}")
                } else {
                    format!("官方用量不可用：{message}")
                });
            }
        }
    }

    fn official_usage(
        &self,
        admin_key: &str,
        utc_date: NaiveDate,
        api_key_name: Option<&str>,
    ) -> Result<OfficialUsageResult, String> {
        self.official_usage_with(admin_key, utc_date, api_key_name, fetch_official_usage)
    }

    fn official_usage_with<F>(
        &self,
        admin_key: &str,
        utc_date: NaiveDate,
        api_key_name: Option<&str>,
        fetch: F,
    ) -> Result<OfficialUsageResult, String>
    where
        F: FnOnce(&str, NaiveDate, Option<&str>) -> Result<OfficialDailyUsage, String>,
    {
        let api_key_name = api_key_name.map(str::to_owned);
        let admin_key_fingerprint = fingerprint(admin_key);
        let matching_cache = |cache: &OfficialUsageCache| {
            cache.utc_date == utc_date
                && cache.api_key_name == api_key_name
                && cache.admin_key_fingerprint == admin_key_fingerprint
        };
        if let Ok(cache) = self.official_usage_cache.lock()
            && let Some(cache) = cache.as_ref().filter(|cache| matching_cache(cache))
            && cache.fetched_at.elapsed() < OFFICIAL_USAGE_CACHE_TTL
        {
            return Ok(OfficialUsageResult {
                usage: cache.usage.clone(),
                captured_at: cache.captured_at,
                stale_reason: None,
            });
        }

        match fetch(admin_key, utc_date, api_key_name.as_deref()) {
            Ok(usage) => {
                let captured_at = Utc::now();
                if let Ok(mut cache) = self.official_usage_cache.lock() {
                    *cache = Some(OfficialUsageCache {
                        utc_date,
                        api_key_name,
                        admin_key_fingerprint,
                        fetched_at: Instant::now(),
                        captured_at,
                        usage: usage.clone(),
                    });
                }
                Ok(OfficialUsageResult {
                    usage,
                    captured_at,
                    stale_reason: None,
                })
            }
            Err(message) => {
                if let Ok(cache) = self.official_usage_cache.lock()
                    && let Some(cache) = cache.as_ref().filter(|cache| matching_cache(cache))
                {
                    return Ok(OfficialUsageResult {
                        usage: cache.usage.clone(),
                        captured_at: cache.captured_at,
                        stale_reason: Some(message),
                    });
                }
                Err(message)
            }
        }
    }

    fn local_tokens(&self) -> Option<TokenUsage> {
        let root = claude_projects_root()?;
        let files = usage_file_stamps(&root);
        let today = Local::now().date_naive();
        if let Ok(cache) = self.usage_cache.lock()
            && let Some(cache) = cache.as_ref()
            && cache_is_current(cache, &files, today)
        {
            return cache.tokens.clone();
        }
        let paths = files
            .iter()
            .map(|stamp| stamp.path.clone())
            .collect::<Vec<_>>();
        let tokens = aggregate_transcript_tokens(&paths, today);
        if let Ok(mut cache) = self.usage_cache.lock() {
            *cache = Some(LocalUsageCache {
                files,
                local_date: today,
                tokens: tokens.clone(),
            });
        }
        tokens
    }
}

fn cache_is_current(cache: &LocalUsageCache, files: &[FileStamp], today: NaiveDate) -> bool {
    cache.local_date == today && cache.files == files
}

fn claude_projects_root() -> Option<PathBuf> {
    let config_root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))?;
    Some(config_root.join("projects"))
}

fn is_first_party_api_key_login(auth_status: &Value) -> bool {
    let auth_method = auth_status
        .get("authMethod")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let api_provider = auth_status
        .get("apiProvider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    auth_method.eq_ignore_ascii_case("api_key")
        && matches!(
            api_provider.to_ascii_lowercase().as_str(),
            "firstparty" | "anthropic"
        )
}

fn admin_key_from_environment() -> Result<Option<Zeroizing<String>>, String> {
    let Some(raw) = std::env::var_os(ANTHROPIC_ADMIN_KEY_ENV) else {
        return Ok(None);
    };
    let raw = raw
        .into_string()
        .map_err(|_| "ANTHROPIC_ADMIN_KEY 不是有效文本".to_string())?;
    let raw = Zeroizing::new(raw);
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if !is_admin_api_key(value) {
        return Err(
            "官方用量需要组织 Admin API Key；普通 ANTHROPIC_API_KEY 不具备用量查询权限".into(),
        );
    }
    Ok(Some(Zeroizing::new(value.to_owned())))
}

fn is_admin_api_key(value: &str) -> bool {
    value.starts_with("sk-ant-admin")
        && value.len() <= 4096
        && !value.chars().any(char::is_whitespace)
}

fn configured_api_key_name() -> Option<String> {
    std::env::var_os(CLAUDE_API_KEY_NAME_ENV)
        .and_then(|value| value.into_string().ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn fingerprint(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeUsagePage {
    #[serde(default)]
    data: Vec<ClaudeCodeUsageRecord>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeUsageRecord {
    actor: ClaudeCodeUsageActor,
    #[serde(default)]
    model_breakdown: Vec<ClaudeCodeModelUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeUsageActor {
    #[serde(rename = "type")]
    actor_type: String,
    #[serde(default)]
    api_key_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeModelUsage {
    estimated_cost: ClaudeCodeEstimatedCost,
    tokens: ClaudeCodeModelTokens,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeEstimatedCost {
    amount: Value,
    currency: String,
}

#[derive(Debug, Default, Deserialize)]
struct ClaudeCodeModelTokens {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    cache_creation: u64,
    #[serde(default)]
    cache_read: u64,
}

impl ClaudeCodeModelTokens {
    fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_creation)
            .saturating_add(self.cache_read)
    }
}

fn fetch_official_usage(
    admin_key: &str,
    utc_date: NaiveDate,
    api_key_name: Option<&str>,
) -> Result<OfficialDailyUsage, String> {
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "无法初始化 Claude 官方用量客户端".to_string())?;
    collect_official_usage_pages(utc_date, api_key_name, |url| {
        request_official_usage_page(&client, admin_key, url)
    })
}

fn request_official_usage_page(
    client: &Client,
    admin_key: &str,
    url: &Url,
) -> Result<ClaudeCodeUsagePage, String> {
    let response = client
        .get(url.clone())
        .header("anthropic-version", ANTHROPIC_API_VERSION)
        .header("x-api-key", admin_key)
        .header(
            reqwest::header::USER_AGENT,
            concat!("Metra/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                "Claude 官方用量请求超时"
            } else if error.is_connect() {
                "无法连接 Claude 官方用量接口"
            } else {
                "Claude 官方用量请求失败"
            }
            .to_string()
        })?;
    validate_official_usage_status(response.status())?;
    let mut body = Vec::new();
    response
        .take(MAX_USAGE_RESPONSE_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|error| format!("无法读取 Claude 官方用量响应（{:?}）", error.kind()))?;
    parse_official_usage_page(&body)
}

fn validate_official_usage_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Admin API Key 无效、已撤销或已过期".into());
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err("Admin API Key 无权读取 Claude Code Analytics".into());
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("Claude 官方用量接口请求过于频繁".into());
    }
    if !status.is_success() {
        return Err(format!(
            "Claude 官方用量接口返回 HTTP {}",
            status.as_u16()
        ));
    }
    Ok(())
}

fn parse_official_usage_page(body: &[u8]) -> Result<ClaudeCodeUsagePage, String> {
    if body.len() as u64 > MAX_USAGE_RESPONSE_BYTES {
        return Err("Claude 官方用量响应过大".into());
    }
    serde_json::from_slice::<ClaudeCodeUsagePage>(body).map_err(|error| {
        format!(
            "Claude 官方用量响应格式不兼容（第 {} 行，第 {} 列）",
            error.line(),
            error.column()
        )
    })
}

fn collect_official_usage_pages<F>(
    utc_date: NaiveDate,
    api_key_name: Option<&str>,
    mut fetch_page: F,
) -> Result<OfficialDailyUsage, String>
where
    F: FnMut(&Url) -> Result<ClaudeCodeUsagePage, String>,
{
    let mut records = Vec::new();
    let mut page = None;
    let mut seen_pages = HashSet::new();

    for _ in 0..MAX_USAGE_PAGES {
        let url = official_usage_url(utc_date, page.as_deref())?;
        if !allowed_official_usage_url(&url) {
            return Err("拒绝访问非 Anthropic 官方用量地址".into());
        }
        let response = fetch_page(&url)?;
        if records
            .len()
            .checked_add(response.data.len())
            .is_none_or(|count| count > MAX_USAGE_RECORDS)
        {
            return Err("Claude 官方用量记录超过安全上限".into());
        }
        records.extend(response.data);
        if !response.has_more {
            return aggregate_official_usage(&records, api_key_name);
        }
        let next_page = response
            .next_page
            .filter(|value| !value.is_empty() && value.len() <= 4096)
            .ok_or_else(|| "Claude 官方用量分页响应无效".to_string())?;
        if !seen_pages.insert(next_page.clone()) {
            return Err("Claude 官方用量分页游标重复".into());
        }
        page = Some(next_page);
    }
    Err("Claude 官方用量分页超过安全上限".into())
}

fn official_usage_url(utc_date: NaiveDate, page: Option<&str>) -> Result<Url, String> {
    let mut url =
        Url::parse(CLAUDE_CODE_USAGE_URL).map_err(|_| "Claude 官方用量地址配置无效".to_string())?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("starting_at", &utc_date.to_string())
            .append_pair("limit", "1000");
        if let Some(page) = page {
            query.append_pair("page", page);
        }
    }
    Ok(url)
}

fn allowed_official_usage_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("api.anthropic.com")
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/v1/organizations/usage_report/claude_code"
        && url.fragment().is_none()
}

fn aggregate_official_usage(
    records: &[ClaudeCodeUsageRecord],
    configured_name: Option<&str>,
) -> Result<OfficialDailyUsage, String> {
    let actor_names = records
        .iter()
        .filter(|record| record.actor.actor_type == "api_actor")
        .filter_map(|record| record.actor.api_key_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let selected_name = if let Some(name) = configured_name.map(str::trim).filter(|v| !v.is_empty())
    {
        if !actor_names.contains(name) {
            return Err(
                "未找到指定 API Key 的今日数据，请检查 METRA_CLAUDE_API_KEY_NAME 或稍后刷新".into(),
            );
        }
        name.to_owned()
    } else {
        match actor_names.len() {
            0 => return Err("官方 API 今日尚未返回 API Key 用量".into()),
            1 => actor_names
                .iter()
                .next()
                .cloned()
                .ok_or_else(|| "Claude 官方用量缺少 API Key 名称".to_string())?,
            _ => {
                return Err(
                    "检测到多个 API Key，请设置 METRA_CLAUDE_API_KEY_NAME 选择对应 Key".into(),
                );
            }
        }
    };

    let mut tokens = 0_u64;
    let mut estimated_cost_cents = 0_u64;
    let mut currency = None::<String>;
    let mut model_count = 0_usize;
    let selected_records = records
        .iter()
        .filter(|record| {
            record.actor.actor_type == "api_actor"
                && record.actor.api_key_name.as_deref().map(str::trim)
                    == Some(selected_name.as_str())
        })
        .collect::<Vec<_>>();
    if selected_records.len() > 1 {
        return Err(
            "检测到多个同名 API Key，无法安全区分用量；请在 Anthropic Console 中使用唯一名称"
                .into(),
        );
    }
    for model in selected_records
        .into_iter()
        .flat_map(|record| &record.model_breakdown)
    {
        model_count += 1;
        tokens = tokens.saturating_add(model.tokens.total());
        let model_currency = model.estimated_cost.currency.trim().to_ascii_uppercase();
        if model_currency.is_empty() {
            return Err("Claude 官方用量缺少费用币种".into());
        }
        if let Some(currency) = currency.as_ref()
            && currency != &model_currency
        {
            return Err("Claude 官方用量包含多个费用币种".into());
        }
        currency = Some(model_currency);
        let amount = minor_currency_units(&model.estimated_cost.amount)
            .ok_or_else(|| "Claude 官方用量费用格式无效".to_string())?;
        estimated_cost_cents = estimated_cost_cents
            .checked_add(amount)
            .ok_or_else(|| "Claude 官方用量费用超出范围".to_string())?;
    }
    if model_count == 0 {
        return Err("Claude 官方 API 今日尚未返回模型用量".into());
    }
    Ok(OfficialDailyUsage {
        tokens,
        estimated_cost_cents,
        currency: currency.ok_or_else(|| "Claude 官方用量缺少费用币种".to_string())?,
    })
}

fn minor_currency_units(value: &Value) -> Option<u64> {
    let amount = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(value) => value.parse::<f64>().ok()?,
        _ => return None,
    };
    (amount.is_finite() && amount >= 0.0 && amount <= u64::MAX as f64)
        .then(|| amount.round() as u64)
}

impl UsageProvider for ClaudeProvider {
    fn detect(&self) -> Detection {
        match self.executable() {
            Some(executable) => Detection {
                executable: Some(executable.path().to_path_buf()),
                status: ProviderStatus::Available,
                detail: None,
            },
            None => Detection {
                executable: None,
                status: ProviderStatus::NotInstalled,
                detail: Some("未检测到 Claude Code CLI".into()),
            },
        }
    }

    fn account_status(&self) -> ProviderSnapshot {
        self.refresh()
    }

    fn refresh(&self) -> ProviderSnapshot {
        match self.collect() {
            Ok(snapshot) => snapshot,
            Err(message) if self.executable().is_none() => ProviderSnapshot::unavailable(
                Provider::Claude,
                ProviderStatus::NotInstalled,
                message,
            ),
            Err(message) if message.contains("未登录") => ProviderSnapshot::unavailable(
                Provider::Claude,
                ProviderStatus::NotLoggedIn,
                message,
            ),
            Err(message) => ProviderSnapshot::unavailable(
                Provider::Claude,
                ProviderStatus::ProtocolError,
                message,
            ),
        }
    }
}

pub fn snapshot_from_auth_status(
    auth_status: &Value,
    tokens: Option<TokenUsage>,
) -> Result<ProviderSnapshot, String> {
    let logged_in = auth_status
        .get("loggedIn")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Claude Code 登录状态格式异常".to_string())?;
    if !logged_in {
        return Err("Claude Code 未登录".into());
    }

    let auth_method = auth_status
        .get("authMethod")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let api_provider = auth_status
        .get("apiProvider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let subscription = auth_status
        .get("subscriptionType")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let plan = subscription
        .map(display_plan)
        .or_else(|| display_api_provider(api_provider))
        .unwrap_or_else(|| match auth_method {
            "api_key" => "API Key".into(),
            "oauth" => "Claude.ai".into(),
            _ => "Claude Code".into(),
        });
    let message = if tokens.is_some() {
        "Claude Code 未提供套餐额度；Token 为本地会话统计"
    } else {
        "Claude Code 当前登录方式未提供额度或 Token 统计"
    };

    Ok(ProviderSnapshot {
        provider: Provider::Claude,
        status: ProviderStatus::Available,
        plan: Some(plan),
        captured_at: Utc::now(),
        quotas: Vec::new(),
        tokens,
        cost: None,
        stale: false,
        message: Some(message.into()),
    })
}

fn display_plan(plan: &str) -> String {
    match plan.to_ascii_lowercase().as_str() {
        "pro" => "Pro".into(),
        "max" => "Max".into(),
        "team" => "Team".into(),
        "enterprise" => "Enterprise".into(),
        _ => plan.to_owned(),
    }
}

fn display_api_provider(provider: &str) -> Option<String> {
    match provider.to_ascii_lowercase().as_str() {
        "anthropicaws" | "bedrock" => Some("Amazon Bedrock".into()),
        "vertex" | "anthropicvertex" => Some("Google Vertex AI".into()),
        "foundry" => Some("Microsoft Foundry".into()),
        _ => None,
    }
}

fn read_auth_status(executable: &ResolvedExecutable, timeout: Duration) -> Result<Value, String> {
    let mut command = command_for(executable, &["auth", "status", "--json"]);
    hide_window(&mut command);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法启动 Claude Code CLI".to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Claude Code 登录状态检测超时".into());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("无法读取 Claude Code 登录状态".into());
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "无法读取 Claude Code 登录状态".to_string())?;
    serde_json::from_slice(&output.stdout).map_err(|_| "Claude Code 登录状态格式异常".to_string())
}

fn claude_known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local/bin/claude"));
        paths.push(home.join(".claude/local/claude"));
        #[cfg(windows)]
        {
            paths.push(home.join("scoop/shims/claude"));
            paths.push(home.join(".bun/bin/claude"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/opt/homebrew/bin/claude"));
        paths.push(PathBuf::from("/usr/local/bin/claude"));
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            paths.push(local.join("Microsoft/WinGet/Links/claude"));
            paths.push(local.join("Programs/Claude/claude.exe"));
        }
        if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
            paths.push(roaming.join("npm/claude"));
            paths.push(roaming.join("pnpm/claude"));
        }
    }
    paths
}

fn usage_file_stamps(root: &Path) -> Vec<FileStamp> {
    let mut paths = Vec::new();
    collect_usage_files(root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            Some(FileStamp {
                path,
                len: metadata.len(),
                modified: metadata.modified().ok(),
            })
        })
        .collect()
}

fn collect_usage_files(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            collect_usage_files(&path, paths);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            paths.push(path);
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptRecord {
    #[serde(rename = "type")]
    record_type: Option<String>,
    uuid: Option<String>,
    session_id: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    message: Option<TranscriptMessage>,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    id: Option<String>,
    usage: Option<TranscriptUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct TranscriptUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

impl TranscriptUsage {
    fn total(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_creation_input_tokens)
            .saturating_add(self.cache_read_input_tokens)
    }
}

fn aggregate_transcript_tokens(paths: &[PathBuf], today: NaiveDate) -> Option<TokenUsage> {
    let mut messages = HashMap::<(String, String), (Option<NaiveDate>, u64)>::new();
    let mut by_day = HashMap::<NaiveDate, u64>::new();
    for path in paths {
        let Ok(file) = File::open(path) else {
            continue;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(record) = serde_json::from_str::<TranscriptRecord>(&line) else {
                continue;
            };
            if record.record_type.as_deref() != Some("assistant") {
                continue;
            }
            let Some(message) = record.message else {
                continue;
            };
            let Some(usage) = message.usage else {
                continue;
            };
            let total = usage.total();
            if total == 0 {
                continue;
            }
            let identifier = message.id.or(record.uuid);
            let Some(identifier) = identifier else {
                continue;
            };
            let scope = record
                .session_id
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            let date = record
                .timestamp
                .map(|timestamp| timestamp.with_timezone(&Local).date_naive());
            let entry = messages.entry((scope, identifier)).or_insert((date, total));
            if total > entry.1 {
                *entry = (date, total);
            } else if entry.0.is_none() {
                entry.0 = date;
            }
        }
    }
    let mut lifetime = 0_u64;
    for (date, total) in messages.into_values() {
        lifetime = lifetime.saturating_add(total);
        if let Some(date) = date {
            let day_total = by_day.entry(date).or_default();
            *day_total = day_total.saturating_add(total);
        }
    }
    (lifetime > 0).then(|| TokenUsage {
        today: Some(by_day.get(&today).copied().unwrap_or(0)),
        lifetime: Some(lifetime),
        peak_daily: by_day.values().copied().max(),
    })
}

#[cfg(windows)]
fn hide_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut std::process::Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, io::Write};

    #[cfg(unix)]
    fn stub_provider(script: &str, timeout: Duration) -> (tempfile::TempDir, ClaudeProvider) {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("claude");
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let provider = ClaudeProvider {
            executable_override: Some(path),
            cached_executable: Mutex::new(None),
            usage_cache: Mutex::new(None),
            official_usage_cache: Mutex::new(None),
            timeout,
        };
        (temp, provider)
    }

    #[cfg(unix)]
    #[test]
    fn auth_status_json_is_honored_even_when_claude_exits_one() {
        let (_temp, provider) = stub_provider(
            r#"printf '%s\n' '{"loggedIn":false,"authMethod":"none"}'
exit 1"#,
            Duration::from_secs(5),
        );

        assert_eq!(provider.refresh().status, ProviderStatus::NotLoggedIn);
    }

    #[cfg(unix)]
    #[test]
    fn malformed_auth_status_is_a_protocol_error() {
        let (_temp, provider) = stub_provider("printf '%s\\n' 'not-json'", Duration::from_secs(5));

        let snapshot = provider.refresh();
        assert_eq!(snapshot.status, ProviderStatus::ProtocolError);
        assert!(
            snapshot
                .message
                .as_deref()
                .is_some_and(|value| value.contains("格式异常"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn auth_status_timeout_is_bounded() {
        let (_temp, provider) = stub_provider("while :; do :; done", Duration::from_millis(40));
        let started = Instant::now();

        let snapshot = provider.refresh();

        assert_eq!(snapshot.status, ProviderStatus::ProtocolError);
        assert!(
            snapshot
                .message
                .as_deref()
                .is_some_and(|value| value.contains("超时"))
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn transcript_tokens_deduplicate_messages_and_ignore_zero_usage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        let mut file = File::create(&path).unwrap();
        let timestamp = Local::now().with_timezone(&Utc).to_rfc3339();
        let record = |uuid: &str, message_id: &str, input, output| {
            serde_json::json!({
                "type": "assistant",
                "uuid": uuid,
                "sessionId": "session-1",
                "timestamp": timestamp,
                "message": {
                    "id": message_id,
                    "usage": {
                        "input_tokens": input,
                        "output_tokens": output,
                        "cache_creation_input_tokens": 5,
                        "cache_read_input_tokens": 7,
                        "cache_creation": {"ephemeral_5m_input_tokens": 5}
                    }
                }
            })
        };
        writeln!(file, "{}", record("uuid-1", "message-1", 10, 20)).unwrap();
        writeln!(file, "{}", record("uuid-2", "message-1", 10, 20)).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "assistant",
                "uuid": "uuid-3",
                "sessionId": "session-1",
                "timestamp": timestamp,
                "message": {
                    "id": "message-2",
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "cache_creation_input_tokens": 0,
                        "cache_read_input_tokens": 0
                    }
                }
            })
        )
        .unwrap();

        let tokens = aggregate_transcript_tokens(&[path], Local::now().date_naive()).unwrap();
        assert_eq!(tokens.today, Some(42));
        assert_eq!(tokens.lifetime, Some(42));
        assert_eq!(tokens.peak_daily, Some(42));
    }

    #[test]
    fn transcript_message_ids_are_scoped_to_their_session() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sessions.jsonl");
        let mut file = File::create(&path).unwrap();
        let timestamp = Local::now().with_timezone(&Utc).to_rfc3339();
        for session in ["session-1", "session-2"] {
            writeln!(
                file,
                "{}",
                serde_json::json!({
                    "type": "assistant",
                    "uuid": format!("uuid-{session}"),
                    "sessionId": session,
                    "timestamp": timestamp,
                    "message": {
                        "id": "shared-message-id",
                        "usage": {"input_tokens": 10, "output_tokens": 2}
                    }
                })
            )
            .unwrap();
        }

        let tokens = aggregate_transcript_tokens(&[path], Local::now().date_naive()).unwrap();
        assert_eq!(tokens.today, Some(24));
        assert_eq!(tokens.lifetime, Some(24));
    }

    #[test]
    fn zero_only_transcripts_do_not_create_misleading_token_metrics() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("zero.jsonl");
        fs::write(
            &path,
            r#"{"type":"assistant","uuid":"zero","message":{"id":"zero","usage":{"input_tokens":0,"output_tokens":0}}}"#,
        )
        .unwrap();

        assert_eq!(
            aggregate_transcript_tokens(&[path], Local::now().date_naive()),
            None
        );
    }

    #[test]
    fn official_usage_aggregates_only_the_selected_api_actor() {
        let page = serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
            "data": [
                {
                    "actor": {"type": "api_actor", "api_key_name": "Claude Code"},
                    "model_breakdown": [
                        {
                            "estimated_cost": {"amount": 186, "currency": "USD"},
                            "tokens": {"input": 100, "output": 20, "cache_creation": 30, "cache_read": 40}
                        },
                        {
                            "estimated_cost": {"amount": "42.0", "currency": "usd"},
                            "tokens": {"input": 50, "output": 10, "cache_creation": 5, "cache_read": 15}
                        }
                    ]
                },
                {
                    "actor": {"type": "api_actor", "api_key_name": "Other key"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 999, "currency": "USD"},
                        "tokens": {"input": 999, "output": 1, "cache_creation": 0, "cache_read": 0}
                    }]
                },
                {
                    "actor": {"type": "user_actor", "email_address": "private@example.com"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 777, "currency": "USD"},
                        "tokens": {"input": 777, "output": 0, "cache_creation": 0, "cache_read": 0}
                    }]
                }
            ],
            "has_more": false,
            "next_page": null
        }))
        .unwrap();

        let usage = aggregate_official_usage(&page.data, Some("Claude Code")).unwrap();

        assert_eq!(usage.tokens, 270);
        assert_eq!(usage.estimated_cost_cents, 228);
        assert_eq!(usage.currency, "USD");
    }

    #[test]
    fn official_usage_requires_a_name_when_multiple_api_actors_exist() {
        let page = serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
            "data": [
                {
                    "actor": {"type": "api_actor", "api_key_name": "First"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 1, "currency": "USD"},
                        "tokens": {"input": 1, "output": 0, "cache_creation": 0, "cache_read": 0}
                    }]
                },
                {
                    "actor": {"type": "api_actor", "api_key_name": "Second"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 2, "currency": "USD"},
                        "tokens": {"input": 2, "output": 0, "cache_creation": 0, "cache_read": 0}
                    }]
                }
            ]
        }))
        .unwrap();

        let error = aggregate_official_usage(&page.data, None).unwrap_err();

        assert!(error.contains(CLAUDE_API_KEY_NAME_ENV));
        assert!(
            aggregate_official_usage(&page.data, Some("Missing"))
                .unwrap_err()
                .contains("未找到指定 API Key")
        );
    }

    #[test]
    fn official_usage_selects_the_only_api_actor_automatically() {
        let page = serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
            "data": [{
                "actor": {"type": "api_actor", "api_key_name": " Only key "},
                "model_breakdown": [{
                    "estimated_cost": {"amount": 3, "currency": "USD"},
                    "tokens": {"input": 4, "output": 5, "cache_creation": 6, "cache_read": 7}
                }]
            }]
        }))
        .unwrap();

        let usage = aggregate_official_usage(&page.data, None).unwrap();

        assert_eq!(usage.tokens, 22);
        assert_eq!(usage.estimated_cost_cents, 3);
    }

    #[test]
    fn official_usage_follows_pages_and_aggregates_the_selected_actor() {
        let mut pages = VecDeque::from([
            serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
                "data": [{
                    "actor": {"type": "api_actor", "api_key_name": "Other"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 2, "currency": "USD"},
                        "tokens": {"input": 3, "output": 4, "cache_creation": 0, "cache_read": 0}
                    }]
                }],
                "has_more": true,
                "next_page": "second page"
            }))
            .unwrap(),
            serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
                "data": [{
                    "actor": {"type": "api_actor", "api_key_name": "Selected"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 5, "currency": "USD"},
                        "tokens": {"input": 6, "output": 7, "cache_creation": 8, "cache_read": 9}
                    }]
                }],
                "has_more": false
            }))
            .unwrap(),
        ]);
        let mut requested_urls = Vec::new();

        let usage = collect_official_usage_pages(
            NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
            Some("Selected"),
            |url| {
                requested_urls.push(url.clone());
                pages
                    .pop_front()
                    .ok_or_else(|| "unexpected extra page".to_string())
            },
        )
        .unwrap();

        assert_eq!(usage.tokens, 30);
        assert_eq!(usage.estimated_cost_cents, 5);
        assert_eq!(requested_urls.len(), 2);
        assert!(!requested_urls[0].query().unwrap().contains("page="));
        assert!(requested_urls[1].as_str().contains("page=second+page"));
    }

    #[test]
    fn official_usage_rejects_invalid_pagination() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap();
        let missing = collect_official_usage_pages(date, None, |_| {
            serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
                "data": [],
                "has_more": true,
                "next_page": null
            }))
            .map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(missing.contains("分页响应无效"));

        let mut calls = 0;
        let repeated = collect_official_usage_pages(date, None, |_| {
            calls += 1;
            serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
                "data": [],
                "has_more": true,
                "next_page": "repeated"
            }))
            .map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(repeated.contains("游标重复"));
        assert_eq!(calls, 2);
    }

    #[test]
    fn official_usage_rejects_ambiguous_duplicate_api_key_names() {
        let page = serde_json::from_value::<ClaudeCodeUsagePage>(serde_json::json!({
            "data": [
                {
                    "actor": {"type": "api_actor", "api_key_name": "Duplicate"},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 1, "currency": "USD"},
                        "tokens": {"input": 1, "output": 0, "cache_creation": 0, "cache_read": 0}
                    }]
                },
                {
                    "actor": {"type": "api_actor", "api_key_name": " Duplicate "},
                    "model_breakdown": [{
                        "estimated_cost": {"amount": 2, "currency": "USD"},
                        "tokens": {"input": 2, "output": 0, "cache_creation": 0, "cache_read": 0}
                    }]
                }
            ]
        }))
        .unwrap();

        let error = aggregate_official_usage(&page.data, Some("Duplicate")).unwrap_err();

        assert!(error.contains("多个同名 API Key"));
    }

    #[test]
    fn official_usage_maps_http_and_body_failures_without_response_content() {
        assert!(
            validate_official_usage_status(reqwest::StatusCode::UNAUTHORIZED)
                .unwrap_err()
                .contains("无效")
        );
        assert!(
            validate_official_usage_status(reqwest::StatusCode::FORBIDDEN)
                .unwrap_err()
                .contains("无权")
        );
        assert!(
            validate_official_usage_status(reqwest::StatusCode::TOO_MANY_REQUESTS)
                .unwrap_err()
                .contains("频繁")
        );
        assert!(
            parse_official_usage_page(b"{not json")
                .unwrap_err()
                .contains("第 1 行")
        );
        assert_eq!(
            parse_official_usage_page(&vec![b' '; MAX_USAGE_RESPONSE_BYTES as usize + 1])
                .unwrap_err(),
            "Claude 官方用量响应过大"
        );
    }

    #[test]
    fn official_usage_uses_stale_cache_after_a_refresh_failure() {
        let provider = ClaudeProvider::default();
        let date = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap();
        let expected = OfficialDailyUsage {
            tokens: 42,
            estimated_cost_cents: 7,
            currency: "USD".into(),
        };
        let fresh = provider
            .official_usage_with("sk-ant-admin01-test", date, Some("Selected"), {
                let expected = expected.clone();
                move |_, _, _| Ok(expected)
            })
            .unwrap();
        let captured_at = fresh.captured_at;
        assert_eq!(fresh.usage, expected);
        assert_eq!(fresh.stale_reason, None);
        provider
            .official_usage_cache
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .fetched_at = Instant::now()
            .checked_sub(OFFICIAL_USAGE_CACHE_TTL + Duration::from_secs(1))
            .unwrap();

        let stale = provider
            .official_usage_with(
                "sk-ant-admin01-test",
                date,
                Some("Selected"),
                |_, _, _| Err("network unavailable".into()),
            )
            .unwrap();

        assert_eq!(stale.usage, expected);
        assert_eq!(stale.captured_at, captured_at);
        assert_eq!(stale.stale_reason.as_deref(), Some("network unavailable"));
    }

    #[test]
    fn official_usage_accepts_only_anthropic_https_endpoint() {
        let allowed = official_usage_url(
            NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
            Some("page with spaces"),
        )
        .unwrap();
        assert!(allowed_official_usage_url(&allowed));
        assert!(allowed.as_str().contains("page+with+spaces"));
        assert!(!allowed_official_usage_url(
            &Url::parse("http://api.anthropic.com/v1/organizations/usage_report/claude_code")
                .unwrap()
        ));
        assert!(!allowed_official_usage_url(
            &Url::parse("https://example.com/v1/organizations/usage_report/claude_code").unwrap()
        ));
    }

    #[test]
    fn standard_api_keys_are_not_accepted_as_admin_credentials() {
        assert!(is_admin_api_key("sk-ant-admin01-example"));
        assert!(!is_admin_api_key("sk-ant-api03-example"));
        assert!(!is_admin_api_key("sk-ant-admin01-example\nheader"));
    }

    #[test]
    fn local_usage_cache_expires_when_the_calendar_day_changes() {
        let today = Local::now().date_naive();
        let cache = LocalUsageCache {
            files: Vec::new(),
            local_date: today,
            tokens: None,
        };

        assert!(cache_is_current(&cache, &[], today));
        assert!(!cache_is_current(
            &cache,
            &[],
            today.succ_opt().expect("next date")
        ));
    }
}
