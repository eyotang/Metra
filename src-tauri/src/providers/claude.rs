use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    thread,
    time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    diagnostics,
    model::{Provider, ProviderSnapshot, ProviderStatus, TokenUsage},
};

use super::{
    Detection, UsageProvider,
    discovery::{ResolvedExecutable, command_for, find_executable_with_override},
};

#[derive(Debug)]
pub struct ClaudeProvider {
    executable_override: Option<PathBuf>,
    cached_executable: Mutex<Option<ResolvedExecutable>>,
    usage_cache: Mutex<Option<LocalUsageCache>>,
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

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self {
            executable_override: None,
            cached_executable: Mutex::new(None),
            usage_cache: Mutex::new(None),
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
        let discovered = find_executable_with_override(
            "METRA_CLAUDE_PATH",
            &["claude"],
            &claude_known_paths(),
        );
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
        let mut snapshot = snapshot_from_auth_status(&auth_status, None)?;
        if let Some(tokens) = self.local_tokens() {
            snapshot.tokens = Some(tokens);
            snapshot.message = Some("Claude Code 未提供套餐额度；Token 为本地会话统计".into());
        }
        Ok(snapshot)
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

fn read_auth_status(
    executable: &ResolvedExecutable,
    timeout: Duration,
) -> Result<Value, String> {
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
    serde_json::from_slice(&output.stdout)
        .map_err(|_| "Claude Code 登录状态格式异常".to_string())
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
    use std::io::Write;

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
        let (_temp, provider) =
            stub_provider("printf '%s\\n' 'not-json'", Duration::from_secs(5));

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
        let (_temp, provider) =
            stub_provider("while :; do :; done", Duration::from_millis(40));
        let started = Instant::now();

        let snapshot = provider.refresh();

        assert_eq!(snapshot.status, ProviderStatus::ProtocolError);
        assert!(snapshot.message.as_deref().is_some_and(|value| value.contains("超时")));
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
