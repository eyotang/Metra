use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::diagnostics;

#[cfg(not(windows))]
const SHELL_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
#[cfg(not(windows))]
const SHELL_MISS_CACHE_TTL: Duration = Duration::from_secs(2);
#[cfg(target_os = "macos")]
const SHELL_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedExecutable {
    path: PathBuf,
    execution_path: OsString,
}

impl fmt::Debug for ResolvedExecutable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedExecutable")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl ResolvedExecutable {
    pub fn from_path(path: PathBuf) -> Self {
        Self::with_path_entries(path, &current_path_entries())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn is_valid(&self) -> bool {
        self.path.is_file()
    }

    fn with_path_entries(path: PathBuf, entries: &[PathBuf]) -> Self {
        let mut execution_entries = Vec::new();
        if let Some(parent) = path.parent().filter(|parent| parent.is_absolute()) {
            execution_entries.push(parent.to_path_buf());
        }
        for entry in entries.iter().filter(|entry| entry.is_absolute()) {
            if !execution_entries.contains(entry) {
                execution_entries.push(entry.clone());
            }
        }
        let execution_path = env::join_paths(&execution_entries)
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| env::var_os("PATH"))
            .unwrap_or_else(default_execution_path);
        Self {
            path,
            execution_path,
        }
    }
}

fn default_execution_path() -> OsString {
    #[cfg(windows)]
    {
        OsString::new()
    }
    #[cfg(not(windows))]
    {
        OsString::from("/usr/bin:/bin:/usr/sbin:/sbin")
    }
}

fn current_path_entries() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths)
                .filter(|entry| entry.is_absolute())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(windows))]
struct ShellCache {
    environment_key: ShellEnvironmentKey,
    resolved_at: Instant,
    paths: HashMap<String, ResolvedExecutable>,
}

#[cfg(not(windows))]
impl ShellCache {
    fn find(&self, names: &[&str]) -> Option<ResolvedExecutable> {
        names.iter().find_map(|name| {
            self.paths
                .get(*name)
                .filter(|path| path.is_valid())
                .cloned()
        })
    }

    fn needs_refresh(&self, environment_key: &ShellEnvironmentKey, names: &[&str]) -> bool {
        if &self.environment_key != environment_key {
            return true;
        }
        let ttl = if self.find(names).is_some() {
            SHELL_CACHE_TTL
        } else {
            SHELL_MISS_CACHE_TTL
        };
        self.resolved_at.elapsed() >= ttl
    }
}

#[cfg(not(windows))]
#[derive(Clone, PartialEq, Eq)]
struct ShellEnvironmentKey {
    path: Option<OsString>,
    shell: Option<OsString>,
    home: Option<OsString>,
    zdotdir: Option<OsString>,
}

#[cfg(not(windows))]
impl ShellEnvironmentKey {
    fn current() -> Self {
        Self {
            path: env::var_os("PATH"),
            shell: env::var_os("SHELL"),
            home: env::var_os("HOME"),
            zdotdir: env::var_os("ZDOTDIR"),
        }
    }
}

#[cfg(not(windows))]
static SHELL_CACHE: OnceLock<Mutex<Option<ShellCache>>> = OnceLock::new();

fn executable_candidates(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.ps1"),
            name.into(),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![name.into()]
    }
}

pub fn find_executable(
    names: &[&str],
    known_paths: &[PathBuf],
) -> Option<ResolvedExecutable> {
    find_executable_ordered(names, known_paths, false)
}

pub fn find_executable_prefer_known(
    names: &[&str],
    known_paths: &[PathBuf],
) -> Option<ResolvedExecutable> {
    find_executable_ordered(names, known_paths, true)
}

fn find_executable_ordered(
    names: &[&str],
    known_paths: &[PathBuf],
    prefer_known_paths: bool,
) -> Option<ResolvedExecutable> {
    let path_entries = current_path_entries();
    diagnostics::info(
        "cli.discovery.start",
        format!(
            "names={} path_entries={} known_paths={} order={}",
            names.join(","),
            path_entries.len(),
            known_paths.len(),
            if prefer_known_paths {
                "known_paths_first"
            } else {
                "path_first"
            }
        ),
    );

    let from_path = || {
        find_in_path_entries(names, &path_entries).map(|path| {
            diagnostics::info(
                "cli.discovery.found",
                format!("source=PATH path={}", path.display()),
            );
            ResolvedExecutable::with_path_entries(path, &path_entries)
        })
    };
    let from_known_paths = || {
        for path in known_paths {
            let resolved = resolve_known_path(path);
            diagnostics::info(
                "cli.discovery.candidate",
                format!("path={} found={}", path.display(), resolved.is_some()),
            );
            if let Some(resolved) = resolved {
                diagnostics::info(
                    "cli.discovery.found",
                    format!("source=known_path path={}", resolved.display()),
                );
                return Some(ResolvedExecutable::with_path_entries(
                    resolved,
                    &path_entries,
                ));
            }
        }
        None
    };

    let discovered = if prefer_known_paths {
        from_known_paths().or_else(from_path)
    } else {
        from_path().or_else(from_known_paths)
    };
    if discovered.is_some() {
        return discovered;
    }
    if let Some(executable) = find_via_login_shell(names) {
        diagnostics::info(
            "cli.discovery.found",
            format!("source=login_shell path={}", executable.path().display()),
        );
        return Some(executable);
    }
    diagnostics::warn(
        "cli.discovery.not_found",
        format!("names={}", names.join(",")),
    );
    None
}

pub fn find_executable_with_override(
    override_env: &str,
    names: &[&str],
    known_paths: &[PathBuf],
) -> Option<ResolvedExecutable> {
    if let Some(value) = env::var_os(override_env).filter(|value| !value.is_empty()) {
        let configured = PathBuf::from(value);
        let resolved = resolve_known_path(&configured);
        diagnostics::info(
            "cli.discovery.override",
            format!(
                "env={} path={} found={}",
                override_env,
                configured.display(),
                resolved.is_some()
            ),
        );
        return resolved.map(ResolvedExecutable::from_path);
    }
    find_executable(names, known_paths)
}

pub fn invalidate_shell_cache() {
    #[cfg(not(windows))]
    if let Some(cache) = SHELL_CACHE.get()
        && let Ok(mut cache) = cache.lock()
    {
        *cache = None;
    }
}

fn find_in_path_entries(names: &[&str], path_entries: &[PathBuf]) -> Option<PathBuf> {
    for dir in path_entries.iter().filter(|dir| dir.is_absolute()) {
        for name in names {
            for candidate in executable_candidates(name) {
                let path = dir.join(candidate);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn resolve_known_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if path.extension().is_some() {
        return None;
    }
    let parent = path.parent()?;
    let name = path.file_name()?.to_str()?;
    executable_candidates(name)
        .into_iter()
        .map(|candidate| parent.join(candidate))
        .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn find_via_login_shell(_names: &[&str]) -> Option<ResolvedExecutable> {
    None
}

#[cfg(not(windows))]
fn find_via_login_shell(names: &[&str]) -> Option<ResolvedExecutable> {
    let environment_key = ShellEnvironmentKey::current();
    let cache = SHELL_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock().ok()?;
    let needs_refresh = cache
        .as_ref()
        .is_none_or(|cached| cached.needs_refresh(&environment_key, names));
    if needs_refresh {
        let refreshed = resolve_standard_commands_via_login_shell();
        let resolved_at = Instant::now();
        match (cache.as_mut(), refreshed) {
            (_, Some(paths)) => {
                *cache = Some(ShellCache {
                    environment_key,
                    resolved_at,
                    paths,
                });
            }
            (Some(cached), None) if cached.environment_key == environment_key => {
                cached.resolved_at = resolved_at;
            }
            _ => {
                *cache = Some(ShellCache {
                    environment_key,
                    resolved_at,
                    paths: HashMap::new(),
                });
            }
        }
    }
    let cached = cache.as_ref()?;
    cached.find(names)
}

#[cfg(target_os = "macos")]
fn resolve_standard_commands_via_login_shell() -> Option<HashMap<String, ResolvedExecutable>> {
    let shell = env::var_os("SHELL")
        .filter(|value| Path::new(value).is_absolute() && Path::new(value).is_file())
        .unwrap_or_else(|| "/bin/zsh".into());
    resolve_standard_commands_via_shell(&shell, &[], SHELL_PROBE_TIMEOUT)
}

#[cfg(target_os = "macos")]
fn resolve_standard_commands_via_shell(
    shell: &OsStr,
    environment: &[(&str, &OsStr)],
    timeout: Duration,
) -> Option<HashMap<String, ResolvedExecutable>> {
    use std::{
        io::Read,
        os::unix::ffi::OsStringExt,
        process::Stdio,
        sync::mpsc,
        thread,
    };

    const OUTPUT_LIMIT: usize = 64 * 1024;
    const PATH_MARKER: &[u8] = b"__METRA_SHELL_PATH__=";
    const SCRIPT: &str =
        r#"exec /bin/sh -c 'printf "__METRA_SHELL_PATH__=%s\n" "$PATH"'"#;
    let started = Instant::now();
    let mut child = match std::process::Command::new(shell)
        .args(["-l", "-i", "-c", SCRIPT])
        .envs(environment.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            diagnostics::warn(
                "cli.discovery.shell_failed",
                format!("stage=spawn kind={:?}", error.kind()),
            );
            return None;
        }
    };
    let stdout = child.stdout.take()?;
    let (output_sender, output_receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = stdout;
        let mut tail = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    tail.extend_from_slice(&buffer[..read]);
                    if tail.len() > OUTPUT_LIMIT {
                        let excess = tail.len() - OUTPUT_LIMIT;
                        tail.drain(..excess);
                    }
                    if marked_path_bytes(&tail, PATH_MARKER).is_some() {
                        let _ = output_sender.send(tail);
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = output_sender.send(tail);
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                diagnostics::warn(
                    "cli.discovery.shell_failed",
                    format!(
                        "stage=timeout elapsed_ms={}",
                        started.elapsed().as_millis()
                    ),
                );
                return None;
            }
        }
    };
    let output = output_receiver
        .recv_timeout(Duration::from_millis(500))
        .unwrap_or_default();
    if !status.success() {
        diagnostics::warn(
            "cli.discovery.shell_failed",
            format!(
                "stage=exit code={} elapsed_ms={}",
                status.code().unwrap_or(-1),
                started.elapsed().as_millis()
            ),
        );
        return None;
    }
    let raw_path = OsString::from_vec(marked_path_bytes(&output, PATH_MARKER)?.to_vec());
    let path_entries = env::split_paths(&raw_path)
        .filter(|entry| entry.is_absolute())
        .collect::<Vec<_>>();
    let paths = resolve_standard_commands_in_paths(&path_entries);
    diagnostics::info(
        "cli.discovery.shell_complete",
        format!(
            "elapsed_ms={} path_entries={} resolved={}",
            started.elapsed().as_millis(),
            path_entries.len(),
            {
                let mut names = paths.keys().cloned().collect::<Vec<_>>();
                names.sort();
                names.join(",")
            }
        ),
    );
    Some(paths)
}

#[cfg(target_os = "macos")]
fn marked_path_bytes<'a>(output: &'a [u8], marker: &[u8]) -> Option<&'a [u8]> {
    let start = output
        .windows(marker.len())
        .rposition(|window| window == marker)?
        + marker.len();
    let end = output[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| start + offset)?;
    Some(&output[start..end])
}

#[cfg(target_os = "macos")]
fn resolve_standard_commands_in_paths(
    path_entries: &[PathBuf],
) -> HashMap<String, ResolvedExecutable> {
    ["cursor-agent", "agent", "codex", "claude"]
        .into_iter()
        .filter_map(|name| {
            find_in_path_entries(&[name], path_entries).map(|path| {
                (
                    name.to_owned(),
                    ResolvedExecutable::with_path_entries(path, path_entries),
                )
            })
        })
        .collect()
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn resolve_standard_commands_via_login_shell() -> Option<HashMap<String, ResolvedExecutable>> {
    use std::{process::Stdio, thread};

    let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    let script = r#"for n in cursor-agent agent codex claude; do p=$(command -v "$n" 2>/dev/null) && printf '%s\t%s\n' "$n" "$p"; done"#;
    let mut child = match std::process::Command::new(shell)
        .args(["-lc", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return None,
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output.stdout,
        Err(_) => return None,
    };
    Some(
        String::from_utf8_lossy(&output)
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(name, path)| {
            let path = PathBuf::from(path.trim());
            path.is_file()
                .then(|| (name.to_owned(), ResolvedExecutable::from_path(path)))
        })
        .collect::<HashMap<_, _>>(),
    )
}

pub fn command_for(
    executable: &ResolvedExecutable,
    args: &[&str],
) -> std::process::Command {
    let path = executable.path();
    #[cfg(windows)]
    {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat") {
            let mut command = std::process::Command::new("cmd.exe");
            command
                .arg("/D")
                .arg("/S")
                .arg("/C")
                .arg(path)
                .args(args);
            return command;
        }
        if extension.eq_ignore_ascii_case("ps1") {
            let mut command = std::process::Command::new("powershell.exe");
            command
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(path)
                .args(args);
            return command;
        }
    }
    let mut command = std::process::Command::new(path);
    command.args(args);
    #[cfg(target_os = "macos")]
    command.env("PATH", &executable.execution_path);
    command
}

#[cfg(all(test, windows))]
mod tests {
    use super::{find_executable, find_executable_prefer_known};

    #[test]
    fn extensionless_known_path_finds_windows_cmd_wrapper() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("cursor-agent");
        let wrapper = base.with_extension("cmd");
        std::fs::write(&wrapper, "@echo off\r\n").unwrap();

        assert_eq!(
            find_executable(&["__metra_missing_cursor_cli__"], &[base])
                .map(|value| value.path().to_path_buf()),
            Some(wrapper)
        );
    }

    #[test]
    fn preferred_known_path_wins_over_a_path_command() {
        let temp = tempfile::tempdir().unwrap();
        let known = temp.path().join("cmd.exe");
        std::fs::write(&known, []).unwrap();

        assert_eq!(
            find_executable_prefer_known(&["cmd"], std::slice::from_ref(&known))
                .map(|value| value.path().to_path_buf()),
            Some(known)
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::{
        ResolvedExecutable, SHELL_MISS_CACHE_TTL, ShellCache, ShellEnvironmentKey, command_for,
        resolve_standard_commands_via_shell,
    };
    use std::{
        collections::HashMap,
        ffi::OsStr,
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };

    #[test]
    fn gui_discovery_uses_the_real_cli_paths_initialized_by_fnm_in_zshrc() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("fnm multishell/bin");
        std::fs::create_dir_all(&bin).unwrap();
        for name in ["codex", "claude"] {
            let executable = bin.join(name);
            std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            temp.path().join(".zprofile"),
            "export PATH=/usr/bin:/bin:/usr/sbin:/sbin\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join(".zshrc"),
            "echo startup-noise\ncodex() { return 0; }\nalias claude=false\nexport PATH=\"$ZDOTDIR/fnm multishell/bin:$PATH\"\n",
        )
        .unwrap();

        let environment = [
            ("ZDOTDIR", temp.path().as_os_str()),
            ("HOME", temp.path().as_os_str()),
            ("PATH", OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin")),
        ];
        let paths = resolve_standard_commands_via_shell(
            OsStr::new("/bin/zsh"),
            &environment,
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(
            paths.get("codex").map(|value| value.path()),
            Some(bin.join("codex").as_path())
        );
        assert_eq!(
            paths.get("claude").map(|value| value.path()),
            Some(bin.join("claude").as_path())
        );
    }

    #[test]
    fn discovered_shell_path_is_used_to_start_node_based_cli_wrappers() {
        let temp = tempfile::tempdir().unwrap();
        let cli_bin = temp.path().join("cli bin");
        let runtime_bin = temp.path().join("runtime bin");
        std::fs::create_dir_all(&cli_bin).unwrap();
        std::fs::create_dir_all(&runtime_bin).unwrap();
        let wrapper = cli_bin.join("codex");
        let node = runtime_bin.join("node");
        std::fs::write(&wrapper, "#!/usr/bin/env node\n").unwrap();
        std::fs::write(&node, "#!/bin/sh\nprintf 'fnm-runtime-ok\\n'\n").unwrap();
        for executable in [&wrapper, &node] {
            std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let executable = ResolvedExecutable::with_path_entries(
            wrapper,
            &[cli_bin, runtime_bin],
        );

        let output = command_for(&executable, &[]).output().unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, b"fnm-runtime-ok\n");
    }

    #[test]
    fn shell_probe_does_not_wait_for_background_processes_that_inherit_stdout() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("fnm/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let executable = bin.join("codex");
        std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            temp.path().join(".zshrc"),
            "sleep 2 &\nexport PATH=\"$ZDOTDIR/fnm/bin:/usr/bin:/bin\"\n",
        )
        .unwrap();
        let environment = [
            ("ZDOTDIR", temp.path().as_os_str()),
            ("HOME", temp.path().as_os_str()),
            ("PATH", OsStr::new("/usr/bin:/bin")),
        ];
        let started = Instant::now();

        let paths = resolve_standard_commands_via_shell(
            OsStr::new("/bin/zsh"),
            &environment,
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            paths.get("codex").map(|value| value.path()),
            Some(executable.as_path())
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn missing_cli_results_expire_quickly_instead_of_poisoning_refresh_for_thirty_minutes() {
        let environment_key = ShellEnvironmentKey {
            path: None,
            shell: None,
            home: None,
            zdotdir: None,
        };
        let recent_miss = ShellCache {
            environment_key: environment_key.clone(),
            resolved_at: Instant::now(),
            paths: HashMap::new(),
        };
        let expired_miss = ShellCache {
            environment_key: environment_key.clone(),
            resolved_at: Instant::now() - SHELL_MISS_CACHE_TTL - Duration::from_millis(1),
            paths: HashMap::new(),
        };

        assert!(!recent_miss.needs_refresh(&environment_key, &["codex"]));
        assert!(expired_miss.needs_refresh(&environment_key, &["codex"]));
    }
}
