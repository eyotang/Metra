use chrono::{Local, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::diagnostics;
use crate::model::{Provider, ProviderSnapshot, ProviderStatus, QuotaWindow, TokenUsage};

const CODEX_NOT_LOGGED_IN: &str = "Codex 未登录";
const CODEX_NETWORK_UNAVAILABLE: &str = "Codex 网络暂时不可用，请稍后重试";

pub fn snapshot_from_messages(messages: &[Value]) -> Result<ProviderSnapshot, String> {
    let account = result_for(messages, 1);
    let limits = result_for(messages, 2).ok_or_else(|| "Codex 未返回额度数据".to_string())?;
    let usage = result_for(messages, 3);
    let plan = plan_from_limits(limits)
        .or_else(|| {
            account.and_then(|value| {
                value
                    .pointer("/account/planType")
                    .or_else(|| value.get("planType"))
                    .and_then(Value::as_str)
            })
        })
        .map(str::to_owned);
    let mut quotas = Vec::new();
    if let Some(by_id) = limits.get("rateLimitsByLimitId").and_then(Value::as_object) {
        for (id, bucket) in by_id {
            append_bucket(&mut quotas, id, bucket);
        }
    } else if let Some(bucket) = limits.get("rateLimits") {
        let id = bucket
            .get("limitId")
            .and_then(Value::as_str)
            .unwrap_or("Codex");
        append_bucket(&mut quotas, id, bucket);
    }
    let tokens = usage.map(|value| {
        let summary = value.get("summary").unwrap_or(&Value::Null);
        let today_string = Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let today = value
            .get("dailyUsageBuckets")
            .and_then(Value::as_array)
            .and_then(|buckets| {
                buckets.iter().find(|bucket| {
                    bucket.get("startDate").and_then(Value::as_str) == Some(today_string.as_str())
                })
            })
            .and_then(|bucket| bucket.get("tokens"))
            .and_then(Value::as_u64);
        TokenUsage {
            today,
            lifetime: summary.get("lifetimeTokens").and_then(Value::as_u64),
            peak_daily: summary.get("peakDailyTokens").and_then(Value::as_u64),
        }
    });
    Ok(ProviderSnapshot {
        provider: Provider::Codex,
        status: ProviderStatus::Available,
        plan,
        captured_at: Utc::now(),
        quotas,
        tokens,
        cost: None,
        stale: false,
        message: None,
    })
}

fn result_for(messages: &[Value], id: u64) -> Option<&Value> {
    messages
        .iter()
        .find(|message| message.get("id").and_then(Value::as_u64) == Some(id))
        .and_then(|message| message.get("result"))
}

fn account_is_explicitly_logged_out(messages: &[Value]) -> bool {
    result_for(messages, 1)
        .and_then(|result| result.get("account"))
        .is_some_and(Value::is_null)
}

fn response_error_code(message: &Value) -> Option<i64> {
    message.pointer("/error/code").and_then(Value::as_i64)
}

fn plan_from_limits(limits: &Value) -> Option<&str> {
    limits
        .pointer("/rateLimitsByLimitId/codex/planType")
        .or_else(|| limits.pointer("/rateLimits/planType"))
        .or_else(|| limits.get("planType"))
        .and_then(Value::as_str)
        .or_else(|| {
            limits
                .get("rateLimitsByLimitId")
                .and_then(Value::as_object)
                .and_then(|buckets| {
                    buckets
                        .values()
                        .find_map(|bucket| bucket.get("planType").and_then(Value::as_str))
                })
        })
}

fn append_bucket(quotas: &mut Vec<QuotaWindow>, fallback_id: &str, bucket: &Value) {
    let label = bucket
        .get("limitName")
        .and_then(Value::as_str)
        .or_else(|| bucket.get("limitId").and_then(Value::as_str))
        .unwrap_or(fallback_id);
    append_window(quotas, label, bucket.get("primary"));
    append_window(quotas, &format!("{label} · 次级"), bucket.get("secondary"));
}

fn append_window(quotas: &mut Vec<QuotaWindow>, label: &str, window: Option<&Value>) {
    let Some(window) = window else { return };
    let Some(used) = window.get("usedPercent").and_then(Value::as_f64) else {
        return;
    };
    let duration = window.get("windowDurationMins").and_then(Value::as_u64);
    let resets_at = window
        .get("resetsAt")
        .and_then(Value::as_i64)
        .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single());
    quotas.push(QuotaWindow::from_used_percent(
        label, used, duration, resets_at,
    ));
}
use super::{
    Detection, UsageProvider,
    discovery::{
        ResolvedExecutable, command_for, find_executable_prefer_known,
        find_executable_with_override,
    },
};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
#[cfg(windows)]
use std::path::Path;

#[derive(Debug)]
pub struct CodexProvider {
    executable_override: Option<PathBuf>,
    cached_executable: Mutex<Option<ResolvedExecutable>>,
    timeout: Duration,
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self {
            executable_override: None,
            cached_executable: Mutex::new(None),
            timeout: Duration::from_secs(15),
        }
    }
}

impl CodexProvider {
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
                format!("provider=codex path={}", executable.path().display()),
            );
            return Some(executable.clone());
        }
        let known_paths = codex_known_paths();
        let discovered = if std::env::var_os("METRA_CODEX_PATH")
            .filter(|value| !value.is_empty())
            .is_some()
        {
            find_executable_with_override("METRA_CODEX_PATH", &["codex"], &known_paths)
        } else {
            find_executable_prefer_known(&["codex"], &known_paths)
        };
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
            .ok_or_else(|| "未检测到 Codex CLI".to_string())?;
        let mut command = command_for(&executable, &["app-server"]);
        hide_window(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "无法启动 Codex App Server".to_string())?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "无法连接 Codex stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "无法连接 Codex stdout".to_string())?;
        let initialize = serde_json::json!({"method":"initialize","id":0,"params":{"clientInfo":{"name":"metra","title":"Metra","version":env!("CARGO_PKG_VERSION")}}});
        writeln!(stdin, "{}", initialize).map_err(|_| "Codex App Server 写入失败".to_string())?;
        stdin
            .flush()
            .map_err(|_| "Codex App Server 写入失败".to_string())?;

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && sender.send(value).is_err()
                {
                    break;
                }
            }
        });
        let deadline = Instant::now() + self.timeout;
        let mut messages = Vec::new();
        let mut rate_limit_attempts = 1_u8;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Codex App Server 初始化超时".into());
            }
            match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
                Ok(message) => {
                    let initialized = message.get("id").and_then(Value::as_u64) == Some(0);
                    messages.push(message);
                    if initialized {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Codex App Server 初始化失败".into());
                }
            }
        }
        let requests = [
            serde_json::json!({"method":"initialized","params":{}}),
            serde_json::json!({"method":"account/read","id":1,"params":{"refreshToken":false}}),
            serde_json::json!({"method":"account/rateLimits/read","id":2}),
            serde_json::json!({"method":"account/usage/read","id":3}),
        ];
        for request in requests {
            writeln!(stdin, "{}", request).map_err(|_| "Codex App Server 写入失败".to_string())?;
        }
        stdin
            .flush()
            .map_err(|_| "Codex App Server 写入失败".to_string())?;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
                Ok(message) => {
                    let id = message.get("id").and_then(Value::as_u64);
                    if id == Some(2) && message.get("error").is_some() && rate_limit_attempts < 2 {
                        diagnostics::warn(
                            "codex.rate_limits.retry",
                            format!(
                                "attempt={} error_code={}",
                                rate_limit_attempts + 1,
                                response_error_code(&message)
                                    .map(|code| code.to_string())
                                    .unwrap_or_else(|| "unknown".into())
                            ),
                        );
                        rate_limit_attempts += 1;
                        thread::sleep(Duration::from_millis(300));
                        let retry = serde_json::json!({
                            "method": "account/rateLimits/read",
                            "id": 2
                        });
                        writeln!(stdin, "{}", retry)
                            .map_err(|_| "Codex App Server 写入失败".to_string())?;
                        stdin
                            .flush()
                            .map_err(|_| "Codex App Server 写入失败".to_string())?;
                        continue;
                    }
                    if id == Some(2) {
                        messages.retain(|existing| {
                            existing.get("id").and_then(Value::as_u64) != Some(2)
                        });
                    }
                    messages.push(message);
                    if [1, 2, 3].iter().all(|expected| {
                        messages.iter().any(|message| {
                            message.get("id").and_then(Value::as_u64) == Some(*expected)
                        })
                    }) {
                        break;
                    }
                    if id.is_none() {
                        continue;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(stdin);
        let _ = child.kill();
        let _ = child.wait();
        if let Some(rate_limit_error) = messages.iter().find(|message| {
            message.get("id").and_then(Value::as_u64) == Some(2) && message.get("error").is_some()
        }) {
            if account_is_explicitly_logged_out(&messages) {
                return Err(CODEX_NOT_LOGGED_IN.into());
            }
            diagnostics::warn(
                "codex.rate_limits.unavailable",
                format!(
                    "attempts={rate_limit_attempts} error_code={}",
                    response_error_code(rate_limit_error)
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".into())
                ),
            );
            return Err(CODEX_NETWORK_UNAVAILABLE.into());
        }
        let mut snapshot = snapshot_from_messages(&messages)?;
        if snapshot
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.today)
            .is_none()
            && let Some(today) = local_codex_today_tokens()
        {
            snapshot
                .tokens
                .get_or_insert_with(TokenUsage::default)
                .today = Some(today);
        }
        Ok(snapshot)
    }
}
impl UsageProvider for CodexProvider {
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
                detail: Some("未检测到 Codex CLI".into()),
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
                Provider::Codex,
                ProviderStatus::NotInstalled,
                message,
            ),
            Err(message) if message == CODEX_NOT_LOGGED_IN => {
                ProviderSnapshot::unavailable(Provider::Codex, ProviderStatus::NotLoggedIn, message)
            }
            Err(message) if message == CODEX_NETWORK_UNAVAILABLE => {
                ProviderSnapshot::unavailable(
                    Provider::Codex,
                    ProviderStatus::NetworkError,
                    message,
                )
            }
            Err(message) => ProviderSnapshot::unavailable(
                Provider::Codex,
                ProviderStatus::ProtocolError,
                message,
            ),
        }
    }
}

fn codex_known_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            push_versioned_candidates(
                &mut paths,
                &local.join("OpenAI/Codex/bin"),
                Path::new("codex.exe"),
                |_| true,
            );
            paths.push(local.join("Programs/Codex/codex.exe"));
            for root in [local.join("Microsoft/WinGet/Links"), local.join("pnpm")] {
                paths.push(root.join("codex"));
            }
        }
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("scoop/shims/codex"));
            paths.push(home.join(".bun/bin/codex"));
            for extension_root in [
                home.join(".cursor/extensions"),
                home.join(".vscode/extensions"),
            ] {
                push_versioned_candidates(
                    &mut paths,
                    &extension_root,
                    Path::new("bin/windows-x86_64/codex.exe"),
                    |name| name.starts_with("openai.chatgpt-"),
                );
            }
        }
        if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
            paths.push(roaming.join("npm/codex"));
            paths.push(roaming.join("pnpm/codex"));
        }
        paths.push(PathBuf::from(r"C:\Program Files\OpenAI Codex\codex.exe"));
    }
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local/bin/codex"));
        #[cfg(target_os = "macos")]
        push_macos_app_bundle_candidates(&mut paths, &home.join("Applications"));
    }
    #[cfg(target_os = "macos")]
    {
        push_macos_app_bundle_candidates(&mut paths, std::path::Path::new("/Applications"));
        paths.push(PathBuf::from("/opt/homebrew/bin/codex"));
        paths.push(PathBuf::from("/usr/local/bin/codex"));
    }
    paths
}

#[cfg(target_os = "macos")]
fn push_macos_app_bundle_candidates(
    paths: &mut Vec<PathBuf>,
    applications: &std::path::Path,
) {
    paths.push(applications.join("Codex.app/Contents/Resources/codex"));
    paths.push(applications.join("ChatGPT.app/Contents/Resources/codex"));
}

#[cfg(windows)]
fn push_versioned_candidates(
    paths: &mut Vec<PathBuf>,
    root: &Path,
    suffix: &Path,
    include: impl Fn(&str) -> bool,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| include(&entry.file_name().to_string_lossy()))
        .map(|entry| entry.path().join(suffix))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.reverse();
    paths.extend(candidates);
}
fn local_codex_today_tokens() -> Option<u64> {
    let today = Local::now().date_naive();
    let midnight = Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0)?)
        .earliest()?;
    let codex_home = dirs::home_dir()?.join(".codex");
    let state_db = std::fs::read_dir(codex_home)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("state_") && name.ends_with(".sqlite"))
        })
        .max_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
        })?;
    let connection = Connection::open_with_flags(
        state_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let mut statement = connection
        .prepare("SELECT rollout_path FROM threads WHERE updated_at >= ?1")
        .ok()?;
    let paths = statement
        .query_map([midnight.timestamp()], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(Result::ok)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    rollout_tokens_for_local_date(&paths, today)
}

fn rollout_tokens_for_local_date(paths: &[PathBuf], date: NaiveDate) -> Option<u64> {
    let mut total = 0_u64;
    let mut saw_today = false;
    for path in paths {
        let Ok(file) = File::open(path) else { continue };
        let mut baseline = 0_u64;
        let mut maximum_today = None::<u64>;
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if event.pointer("/payload/type").and_then(Value::as_str) != Some("token_count") {
                continue;
            }
            let Some(timestamp) = event
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            else {
                continue;
            };
            let Some(tokens) = event
                .pointer("/payload/info/total_token_usage/total_tokens")
                .and_then(Value::as_u64)
            else {
                continue;
            };
            let event_date = timestamp.with_timezone(&Local).date_naive();
            if event_date < date {
                baseline = baseline.max(tokens);
            } else if event_date == date {
                maximum_today = Some(maximum_today.unwrap_or(0).max(tokens));
            }
        }
        if let Some(maximum) = maximum_today {
            saw_today = true;
            total = total.saturating_add(maximum.saturating_sub(baseline));
        }
    }
    saw_today.then_some(total)
}
fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn discovery_finds_codex_bundled_with_chatgpt_app() {
        let temp = tempfile::tempdir().unwrap();
        let applications = temp.path().join("Applications");
        let bundled_codex = applications.join("ChatGPT.app/Contents/Resources/codex");
        std::fs::create_dir_all(bundled_codex.parent().unwrap()).unwrap();
        std::fs::write(&bundled_codex, []).unwrap();
        let mut known_paths = Vec::new();
        push_macos_app_bundle_candidates(&mut known_paths, &applications);

        let discovered = find_executable_prefer_known(
            &["__metra_missing_codex_cli__"],
            &known_paths,
        );

        assert_eq!(
            discovered.map(|value| value.path().to_path_buf()),
            Some(bundled_codex)
        );
    }

    #[cfg(windows)]
    #[test]
    fn discovery_covers_windows_user_package_managers() {
        let home = dirs::home_dir().unwrap();
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        let roaming = PathBuf::from(std::env::var_os("APPDATA").unwrap());
        let paths = codex_known_paths();
        assert!(paths.contains(&roaming.join("npm/codex")));
        assert!(paths.contains(&roaming.join("pnpm/codex")));
        assert!(paths.contains(&local.join("Microsoft/WinGet/Links/codex")));
        assert!(paths.contains(&home.join("scoop/shims/codex")));
    }

    fn write_fake_server(dir: &std::path::Path, silent: bool) -> PathBuf {
        #[cfg(windows)]
        {
            let path = dir.join("fake-codex.ps1");
            let body = if silent {
                "Start-Sleep -Seconds 5\n"
            } else {
                "Start-Sleep -Milliseconds 200\nWrite-Output '{\"id\":0,\"result\":{\"userAgent\":\"mock\"}}'\nWrite-Output '{\"id\":1,\"result\":{\"account\":{\"type\":\"chatgpt\",\"planType\":\"plus\"}}}'\nWrite-Output '{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex\",\"primary\":{\"usedPercent\":25,\"windowDurationMins\":300}}}}'\nWrite-Output '{\"id\":3,\"result\":{\"summary\":{\"lifetimeTokens\":1234}}}'\n"
            };
            std::fs::write(&path, body).unwrap();
            path
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join("fake-codex");
            let body = if silent {
                "#!/bin/sh\nsleep 5\n"
            } else {
                "#!/bin/sh\nsleep 0.2\nprintf '%s\\n' '{\"id\":0,\"result\":{\"userAgent\":\"mock\"}}' '{\"id\":1,\"result\":{\"account\":{\"type\":\"chatgpt\",\"planType\":\"plus\"}}}' '{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex\",\"primary\":{\"usedPercent\":25,\"windowDurationMins\":300}}}}' '{\"id\":3,\"result\":{\"summary\":{\"lifetimeTokens\":1234}}}'\n"
            };
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
    }

    fn write_flaky_rate_server(dir: &std::path::Path, recover_on_retry: bool) -> PathBuf {
        #[cfg(windows)]
        {
            let path = dir.join("flaky-codex.ps1");
            let final_rate_response = if recover_on_retry {
                r#"Write-Output '{"id":2,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":25,"windowDurationMins":300}}}}'"#
            } else {
                r#"Write-Output '{"id":2,"error":{"code":-32603,"message":"failed to fetch codex rate limits: error sending request"}}'"#
            };
            let body = format!(
                "Start-Sleep -Milliseconds 200\nWrite-Output '{{\"id\":0,\"result\":{{\"userAgent\":\"mock\"}}}}'\nWrite-Output '{{\"id\":1,\"result\":{{\"account\":{{\"type\":\"chatgpt\",\"planType\":\"plus\"}}}}}}'\nWrite-Output '{{\"id\":2,\"error\":{{\"code\":-32603,\"message\":\"failed to fetch codex rate limits: error sending request\"}}}}'\nWrite-Output '{{\"id\":3,\"result\":{{\"summary\":{{\"lifetimeTokens\":1234}}}}}}'\nStart-Sleep -Milliseconds 400\n{final_rate_response}\n"
            );
            std::fs::write(&path, body).unwrap();
            path
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join("flaky-codex");
            let final_rate_response = if recover_on_retry {
                r#"'{"id":2,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":25,"windowDurationMins":300}}}}'"#
            } else {
                r#"'{"id":2,"error":{"code":-32603,"message":"failed to fetch codex rate limits: error sending request"}}'"#
            };
            let body = format!(
                "#!/bin/sh\nsleep 0.2\nprintf '%s\\n' '{{\"id\":0,\"result\":{{\"userAgent\":\"mock\"}}}}' '{{\"id\":1,\"result\":{{\"account\":{{\"type\":\"chatgpt\",\"planType\":\"plus\"}}}}}}' '{{\"id\":2,\"error\":{{\"code\":-32603,\"message\":\"failed to fetch codex rate limits: error sending request\"}}}}' '{{\"id\":3,\"result\":{{\"summary\":{{\"lifetimeTokens\":1234}}}}}}'\nsleep 0.4\nprintf '%s\\n' {final_rate_response}\n"
            );
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
    }

    #[test]
    fn mock_app_server_is_consumed_over_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let provider = CodexProvider {
            executable_override: Some(write_fake_server(temp.path(), false)),
            cached_executable: Mutex::new(None),
            timeout: Duration::from_secs(3),
        };
        let snapshot = provider.refresh();
        assert_eq!(snapshot.status, ProviderStatus::Available);
        assert_eq!(snapshot.quotas[0].remaining_percent, 75.0);
        assert_eq!(snapshot.tokens.unwrap().lifetime, Some(1234));
    }

    #[test]
    fn transient_rate_limit_error_is_retried_before_publishing_failure() {
        let temp = tempfile::tempdir().unwrap();
        let provider = CodexProvider {
            executable_override: Some(write_flaky_rate_server(temp.path(), true)),
            cached_executable: Mutex::new(None),
            timeout: Duration::from_secs(3),
        };

        let snapshot = provider.refresh();

        assert_eq!(snapshot.status, ProviderStatus::Available);
        assert_eq!(snapshot.quotas[0].remaining_percent, 75.0);
    }

    #[test]
    fn repeated_rate_limit_transport_error_is_not_treated_as_logout() {
        let temp = tempfile::tempdir().unwrap();
        let provider = CodexProvider {
            executable_override: Some(write_flaky_rate_server(temp.path(), false)),
            cached_executable: Mutex::new(None),
            timeout: Duration::from_secs(3),
        };

        let snapshot = provider.refresh();

        assert_eq!(snapshot.status, ProviderStatus::NetworkError);
        assert!(
            snapshot
                .message
                .as_deref()
                .is_some_and(|message| message.contains("暂时"))
        );
    }

    #[test]
    fn an_explicitly_empty_account_is_still_treated_as_logout() {
        let logged_out = [serde_json::json!({"id": 1, "result": {"account": null}})];
        let logged_in = [
            serde_json::json!({"id": 1, "result": {"account": {"type": "chatgpt"}}}),
        ];

        assert!(account_is_explicitly_logged_out(&logged_out));
        assert!(!account_is_explicitly_logged_out(&logged_in));
    }

    #[test]
    fn local_daily_tokens_use_cumulative_delta_without_double_counting() {
        let temp = tempfile::tempdir().unwrap();
        let today = Local::now().date_naive();
        let yesterday = today.pred_opt().unwrap();
        let timestamp = |date: NaiveDate, hour| {
            Local
                .from_local_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
                .single()
                .unwrap()
                .to_rfc3339()
        };
        let event = |timestamp: String, tokens| {
            serde_json::json!({
                "timestamp": timestamp,
                "payload": {
                    "type": "token_count",
                    "info": {"total_token_usage": {"total_tokens": tokens}}
                }
            })
            .to_string()
        };
        let old_thread = temp.path().join("old.jsonl");
        std::fs::write(
            &old_thread,
            [
                event(timestamp(yesterday, 23), 100),
                event(timestamp(today, 1), 100),
                event(timestamp(today, 2), 160),
            ]
            .join("\n"),
        )
        .unwrap();
        let new_thread = temp.path().join("new.jsonl");
        std::fs::write(&new_thread, event(timestamp(today, 3), 40)).unwrap();

        assert_eq!(
            rollout_tokens_for_local_date(&[old_thread, new_thread], today),
            Some(100)
        );
    }
    #[test]
    fn silent_app_server_is_killed_at_timeout() {
        let temp = tempfile::tempdir().unwrap();
        let provider = CodexProvider {
            executable_override: Some(write_fake_server(temp.path(), true)),
            cached_executable: Mutex::new(None),
            timeout: Duration::from_millis(150),
        };
        let started = Instant::now();
        let snapshot = provider.refresh();
        assert_eq!(snapshot.status, ProviderStatus::ProtocolError);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
