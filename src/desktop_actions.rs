//! Shared native desktop operations; caller owns async scheduling and progress.
use std::time::{Duration, Instant};

pub fn open_url(url: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer.exe"
    } else {
        "xdg-open"
    });
    command.arg(url);
    crate::providers::scrub_sensitive_child_environment(&mut command);
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn login_cursor(compat_enabled: bool) -> Result<bool, String> {
    let executable = crate::providers::cursor::cursor_login_executable();
    if !compat_enabled && let Some(executable) = executable {
        let mut command = crate::providers::discovery::command_for(&executable, &["login"]);
        crate::providers::scrub_sensitive_child_environment(&mut command);
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) => return Err("Cursor sign-in was not completed".into()),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Cursor sign-in timed out".into());
                }
            }
        }
        return Ok(true);
    } else {
        open_url("cursor://anysphere.cursor-deeplink/settings/plan-usage")?;
    }

    Ok(false)
}
