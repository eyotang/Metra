use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use chrono::{SecondsFormat, Utc};

const LOG_FILE_NAME: &str = "metra.log";
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn init() -> Option<PathBuf> {
    let directory = log_directory()?;
    if fs::create_dir_all(&directory).is_err() {
        return None;
    }
    let path = directory.join(LOG_FILE_NAME);
    rotate_if_needed(&path);
    let _ = LOG_PATH.set(path.clone());
    info(
        "app.start",
        format!(
            "version={} os={} arch={} pid={}",
            env!("CARGO_PKG_VERSION"),
            env::consts::OS,
            env::consts::ARCH,
            std::process::id()
        ),
    );
    Some(path)
}

pub fn info(event: &str, detail: impl AsRef<str>) {
    write("INFO", event, detail.as_ref());
}

pub fn warn(event: &str, detail: impl AsRef<str>) {
    write("WARN", event, detail.as_ref());
}

fn write(level: &str, event: &str, detail: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let Ok(_guard) = LOG_LOCK.lock() else {
        return;
    };
    let clean_event = clean(event, 80);
    let clean_detail = clean(detail, 2_000);
    let line = format!(
        "{} [{}] {} {}\n",
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        level,
        clean_event,
        clean_detail
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn log_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let root = dirs::config_dir();
    root.map(|path| path.join("metra"))
}

fn rotate_if_needed(path: &Path) {
    if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) <= MAX_LOG_BYTES {
        return;
    }
    let archive = path.with_extension("log.1");
    if archive.is_file() {
        let _ = fs::remove_file(&archive);
    }
    let _ = fs::rename(path, archive);
}

fn clean(value: &str, max_chars: usize) -> String {
    let mut cleaned: String = value
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            _ => character,
        })
        .take(max_chars)
        .collect();
    if value.chars().count() > max_chars {
        cleaned.push('…');
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::clean;
    #[cfg(windows)]
    use super::log_directory;

    #[test]
    fn log_fields_are_single_line_and_bounded() {
        assert_eq!(clean("one\ntwo\tthree", 80), "one two three");
        assert_eq!(clean("abcdef", 3), "abc…");
    }
    #[cfg(windows)]
    #[test]
    fn windows_logs_live_under_roaming_appdata_metra() {
        let appdata = std::env::var_os("APPDATA").unwrap();
        assert_eq!(
            log_directory().unwrap(),
            std::path::PathBuf::from(appdata).join("metra")
        );
    }
}
