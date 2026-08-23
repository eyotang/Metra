//! Signed application updates backed by the latest public GitHub Release.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, utils::config::BundleType};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::diagnostics;

const UPDATE_EVENT: &str = "app-update-status";
const UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(20);
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const UPDATE_SCHEDULER_TICK: Duration = Duration::from_secs(5);
const UPDATE_MANIFEST_MAX_BYTES: u64 = 64 * 1024;
const UPDATE_MANIFEST_URL: &str =
    "https://github.com/eyotang/Metra/releases/latest/download/latest.json";
const UPDATE_RELEASE_TAG_PAGE_PREFIX: &str = "https://github.com/eyotang/Metra/releases/tag/v";

/// Installation policy for the current application distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppUpdateMode {
    Disabled,
    ManualDownload,
    InApp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// User-visible phase of the application update flow.
pub enum AppUpdatePhase {
    Idle,
    Available,
    Downloading,
    Installing,
    Deferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Serializable snapshot shared with the details panel.
pub struct AppUpdateStatus {
    pub revision: u64,
    pub supported: bool,
    pub mode: AppUpdateMode,
    pub phase: AppUpdatePhase,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum UpdateOperation {
    #[default]
    Idle,
    Checking,
    Installing,
}

struct UpdateState {
    status: AppUpdateStatus,
    pending: Option<Update>,
    operation: UpdateOperation,
}

#[derive(Debug, Deserialize)]
struct ManualUpdateManifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ManualUpdate {
    version: String,
    notes: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StableVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

/// Thread-safe update state shared by the scheduler and Tauri commands.
pub struct AppUpdaterState {
    inner: Mutex<UpdateState>,
}

impl AppUpdaterState {
    /// Creates update state for the current application distribution.
    pub fn new(mode: AppUpdateMode) -> Self {
        Self {
            inner: Mutex::new(UpdateState {
                status: AppUpdateStatus {
                    revision: 0,
                    supported: mode != AppUpdateMode::Disabled,
                    mode,
                    phase: AppUpdatePhase::Idle,
                    current_version: env!("CARGO_PKG_VERSION").to_string(),
                    version: None,
                    notes: None,
                    downloaded_bytes: 0,
                    total_bytes: None,
                },
                pending: None,
                operation: UpdateOperation::Idle,
            }),
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, UpdateState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn status(&self) -> AppUpdateStatus {
        self.lock_inner().status.clone()
    }

    fn begin_check(&self) -> bool {
        let mut state = self.lock_inner();
        if !state.status.supported || state.operation != UpdateOperation::Idle {
            return false;
        }
        state.operation = UpdateOperation::Checking;
        true
    }

    fn finish_in_app_check_available(&self, mut update: Update) -> Option<AppUpdateStatus> {
        let mut state = self.lock_inner();
        if state.operation != UpdateOperation::Checking || state.status.mode != AppUpdateMode::InApp
        {
            return None;
        }
        update.timeout = Some(UPDATE_DOWNLOAD_TIMEOUT);
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Available;
        state.status.version = Some(update.version.clone());
        state.status.notes = update.body.clone();
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        state.pending = Some(update);
        state.operation = UpdateOperation::Idle;
        Some(state.status.clone())
    }

    fn finish_manual_check_available(&self, update: ManualUpdate) -> Option<AppUpdateStatus> {
        let mut state = self.lock_inner();
        if state.operation != UpdateOperation::Checking
            || state.status.mode != AppUpdateMode::ManualDownload
        {
            return None;
        }
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Available;
        state.status.version = Some(update.version);
        state.status.notes = update.notes;
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        state.pending = None;
        state.operation = UpdateOperation::Idle;
        Some(state.status.clone())
    }

    fn finish_check_idle(&self) -> Option<AppUpdateStatus> {
        let mut state = self.lock_inner();
        if state.operation != UpdateOperation::Checking {
            return None;
        }
        state.operation = UpdateOperation::Idle;
        if state.status.phase == AppUpdatePhase::Idle && state.pending.is_none() {
            return None;
        }
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Idle;
        state.status.version = None;
        state.status.notes = None;
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        state.pending = None;
        Some(state.status.clone())
    }

    fn finish_check_deferred(&self) {
        let mut state = self.lock_inner();
        if state.operation == UpdateOperation::Checking {
            state.operation = UpdateOperation::Idle;
        }
    }

    fn begin_download(&self) -> Option<(Update, AppUpdateStatus)> {
        let mut state = self.lock_inner();
        if state.status.mode != AppUpdateMode::InApp || state.operation != UpdateOperation::Idle {
            return None;
        }
        let update = state.pending.take()?;
        state.operation = UpdateOperation::Installing;
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Downloading;
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        Some((update, state.status.clone()))
    }

    fn add_download_progress(
        &self,
        chunk_length: usize,
        content_length: Option<u64>,
    ) -> AppUpdateStatus {
        let mut state = self.lock_inner();
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.downloaded_bytes = state
            .status
            .downloaded_bytes
            .saturating_add(chunk_length as u64);
        if content_length.is_some() {
            state.status.total_bytes = content_length;
        }
        state.status.clone()
    }

    fn set_installing(&self) -> AppUpdateStatus {
        let mut state = self.lock_inner();
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Installing;
        state.status.clone()
    }

    fn set_deferred(&self) -> AppUpdateStatus {
        let mut state = self.lock_inner();
        state.operation = UpdateOperation::Idle;
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Deferred;
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        state.pending = None;
        state.status.clone()
    }
}

#[derive(Debug)]
struct UpdateSchedule {
    next_due: Instant,
}

impl UpdateSchedule {
    fn new(now: Instant) -> Self {
        Self {
            next_due: now + UPDATE_STARTUP_DELAY,
        }
    }

    fn claim_due(&mut self, now: Instant) -> bool {
        if now < self.next_due {
            return false;
        }
        self.next_due = now + UPDATE_CHECK_INTERVAL;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Keeping all variants available makes the policy matrix deterministic in
// cross-platform unit tests even though one production target uses only one.
#[allow(dead_code)]
enum DesktopPlatform {
    Macos,
    Windows,
    Other,
}

fn macos_app_bundle_from_executable(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension()? != "app" || bundle.file_stem()?.is_empty() {
        return None;
    }
    Some(bundle.to_path_buf())
}

fn update_mode(
    debug: bool,
    platform: DesktopPlatform,
    bundle_type: Option<BundleType>,
    executable: Option<&Path>,
) -> AppUpdateMode {
    if debug {
        return AppUpdateMode::Disabled;
    }
    match (platform, bundle_type) {
        (DesktopPlatform::Windows, Some(BundleType::Msi | BundleType::Nsis)) => {
            AppUpdateMode::InApp
        }
        (DesktopPlatform::Windows, None) => AppUpdateMode::ManualDownload,
        (DesktopPlatform::Macos, Some(BundleType::App))
            if executable
                .and_then(macos_app_bundle_from_executable)
                .is_some() =>
        {
            // tauri-plugin-updater 2.10.1 can destroy the existing .app if its
            // final rename fails (upstream issue #3505). Keep version discovery,
            // but direct macOS users to the manual DMG while Apple signing is unavailable.
            AppUpdateMode::ManualDownload
        }
        _ => AppUpdateMode::Disabled,
    }
}

fn parse_stable_version(version: &str) -> Option<StableVersion> {
    let mut segments = version.split('.');
    let parse_segment = |segment: &str| {
        if segment.is_empty()
            || !segment.bytes().all(|byte| byte.is_ascii_digit())
            || (segment.len() > 1 && segment.starts_with('0'))
        {
            return None;
        }
        segment.parse::<u64>().ok()
    };

    let version = StableVersion {
        major: parse_segment(segments.next()?)?,
        minor: parse_segment(segments.next()?)?,
        patch: parse_segment(segments.next()?)?,
    };
    segments.next().is_none().then_some(version)
}

fn parse_manual_update_manifest(
    bytes: &[u8],
    current_version: &str,
) -> Result<Option<ManualUpdate>, String> {
    if bytes.len() as u64 > UPDATE_MANIFEST_MAX_BYTES {
        return Err("update manifest exceeds the size limit".to_string());
    }

    let manifest = serde_json::from_slice::<ManualUpdateManifest>(bytes)
        .map_err(|error| format!("invalid update manifest: {error}"))?;
    let available = parse_stable_version(&manifest.version)
        .ok_or_else(|| "update manifest version is not a stable semantic version".to_string())?;
    let current = parse_stable_version(current_version)
        .ok_or_else(|| "current version is not a stable semantic version".to_string())?;

    Ok((available > current).then_some(ManualUpdate {
        version: manifest.version,
        notes: manifest.notes,
    }))
}

fn fetch_manual_update(current_version: &str) -> Result<Option<ManualUpdate>, String> {
    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 {
            attempt.error("too many update manifest redirects")
        } else if attempt.url().scheme() != "https" {
            attempt.error("update manifest redirect must use HTTPS")
        } else {
            attempt.follow()
        }
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .redirect(redirect_policy)
        .user_agent(concat!("Metra/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client
        .get(UPDATE_MANIFEST_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| error.to_string())?;

    if response.url().scheme() != "https" {
        return Err("update manifest response must use HTTPS".to_string());
    }
    if response
        .content_length()
        .is_some_and(|length| length > UPDATE_MANIFEST_MAX_BYTES)
    {
        return Err("update manifest exceeds the size limit".to_string());
    }

    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(UPDATE_MANIFEST_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    parse_manual_update_manifest(&bytes, current_version)
}

fn manual_download_page(version: &str) -> Option<String> {
    parse_stable_version(version)?;
    Some(format!("{UPDATE_RELEASE_TAG_PAGE_PREFIX}{version}"))
}

/// Returns the safe update policy for the running application distribution.
pub fn current_distribution_update_mode() -> AppUpdateMode {
    #[cfg(target_os = "macos")]
    let platform = DesktopPlatform::Macos;
    #[cfg(target_os = "windows")]
    let platform = DesktopPlatform::Windows;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let platform = DesktopPlatform::Other;

    let executable = tauri::utils::platform::current_exe().ok();
    update_mode(
        cfg!(debug_assertions),
        platform,
        tauri::utils::platform::bundle_type(),
        executable.as_deref(),
    )
}

fn emit_status(app: &AppHandle, status: AppUpdateStatus) {
    let _ = app.emit(UPDATE_EVENT, status);
}

fn queue_update_check(app: AppHandle, updater: Arc<AppUpdaterState>) {
    if !updater.begin_check() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        match updater.status().mode {
            AppUpdateMode::InApp => {
                let result = match app.updater_builder().timeout(UPDATE_CHECK_TIMEOUT).build() {
                    Ok(client) => client.check().await,
                    Err(error) => Err(error),
                };

                match result {
                    Ok(Some(update)) => {
                        diagnostics::info(
                            "updater.available",
                            format!(
                                "current={} available={}",
                                update.current_version, update.version
                            ),
                        );
                        if let Some(status) = updater.finish_in_app_check_available(update) {
                            emit_status(&app, status);
                        }
                    }
                    Ok(None) => {
                        diagnostics::info("updater.current", "no_update=true");
                        if let Some(status) = updater.finish_check_idle() {
                            emit_status(&app, status);
                        }
                    }
                    Err(error) => {
                        // A blocked or unavailable GitHub endpoint is not actionable for the user.
                        // Keep the current version and wait for the next scheduled daily check.
                        diagnostics::warn("updater.check_deferred", error.to_string());
                        updater.finish_check_deferred();
                    }
                }
            }
            AppUpdateMode::ManualDownload => {
                let current_version = updater.status().current_version;
                let result = tauri::async_runtime::spawn_blocking(move || {
                    fetch_manual_update(&current_version)
                })
                .await;

                match result {
                    Ok(Ok(Some(update))) => {
                        diagnostics::info(
                            "updater.available",
                            format!(
                                "current={} available={}",
                                updater.status().current_version,
                                update.version
                            ),
                        );
                        if let Some(status) = updater.finish_manual_check_available(update) {
                            emit_status(&app, status);
                        }
                    }
                    Ok(Ok(None)) => {
                        diagnostics::info("updater.current", "no_update=true");
                        if let Some(status) = updater.finish_check_idle() {
                            emit_status(&app, status);
                        }
                    }
                    Ok(Err(error)) => {
                        diagnostics::warn("updater.check_deferred", error);
                        updater.finish_check_deferred();
                    }
                    Err(error) => {
                        diagnostics::warn("updater.check_deferred", error.to_string());
                        updater.finish_check_deferred();
                    }
                }
            }
            AppUpdateMode::Disabled => {
                updater.finish_check_deferred();
            }
        }
    });
}

/// Starts one update check shortly after launch and another every 24 hours.
pub fn start_update_scheduler(app: AppHandle, updater: Arc<AppUpdaterState>) {
    if !updater.status().supported {
        diagnostics::info("updater.skipped", "reason=unsupported_distribution");
        return;
    }

    std::thread::spawn(move || {
        let mut schedule = UpdateSchedule::new(Instant::now());
        loop {
            std::thread::sleep(UPDATE_SCHEDULER_TICK);
            if schedule.claim_due(Instant::now()) {
                queue_update_check(app.clone(), updater.clone());
            }
        }
    });
}

#[tauri::command]
/// Returns the latest update state without starting a network request.
pub fn get_app_update_status(updater: State<'_, Arc<AppUpdaterState>>) -> AppUpdateStatus {
    updater.status()
}

#[tauri::command]
/// Downloads, verifies, and installs the update previously found by the scheduler.
pub async fn install_app_update(app: AppHandle) -> AppUpdateStatus {
    let updater = app.state::<Arc<AppUpdaterState>>().inner().clone();
    if updater.status().mode != AppUpdateMode::InApp {
        return updater.status();
    }

    let Some((update, status)) = updater.begin_download() else {
        return updater.status();
    };
    emit_status(&app, status);

    let progress_app = app.clone();
    let progress_state = updater.clone();
    let bytes = match update
        .download(
            move |chunk_length, content_length| {
                emit_status(
                    &progress_app,
                    progress_state.add_download_progress(chunk_length, content_length),
                );
            },
            || {},
        )
        .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            diagnostics::warn("updater.download_deferred", error.to_string());
            let status = updater.set_deferred();
            emit_status(&app, status.clone());
            return status;
        }
    };

    emit_status(&app, updater.set_installing());
    #[cfg(not(target_os = "windows"))]
    {
        let _ = bytes;
        diagnostics::warn(
            "updater.install_deferred",
            "in-app installation is disabled on this platform",
        );
        let status = updater.set_deferred();
        emit_status(&app, status.clone());
        status
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(error) = update.install(bytes) {
            diagnostics::warn("updater.install_deferred", error.to_string());
            let status = updater.set_deferred();
            emit_status(&app, status.clone());
            return status;
        }
        diagnostics::info("updater.installed", format!("version={}", update.version));
        updater.status()
    }
}

#[tauri::command]
/// Opens the fixed GitHub Release page for a discovered manual update.
pub fn open_app_update_download_page(
    updater: State<'_, Arc<AppUpdaterState>>,
) -> Result<(), String> {
    let status = updater.status();
    if status.mode != AppUpdateMode::ManualDownload || status.phase != AppUpdatePhase::Available {
        return Ok(());
    }
    let download_page = status
        .version
        .as_deref()
        .and_then(manual_download_page)
        .ok_or_else(|| "update version is not a stable release".to_string())?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(download_page)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::open_url(&download_page)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = download_page;
        Err("manual update downloads are not enabled on this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppUpdateMode, AppUpdatePhase, AppUpdaterState, DesktopPlatform, ManualUpdate,
        UPDATE_CHECK_INTERVAL, UPDATE_MANIFEST_MAX_BYTES, UPDATE_STARTUP_DELAY, UpdateSchedule,
        macos_app_bundle_from_executable, manual_download_page, parse_manual_update_manifest,
        parse_stable_version, update_mode,
    };
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tauri::utils::config::BundleType;

    #[test]
    fn startup_and_daily_schedule_never_retries_early() {
        let start = Instant::now();
        let mut schedule = UpdateSchedule::new(start);

        assert!(!schedule.claim_due(start));
        assert!(!schedule.claim_due(start + UPDATE_STARTUP_DELAY - Duration::from_millis(1)));
        assert!(schedule.claim_due(start + UPDATE_STARTUP_DELAY));
        assert!(!schedule.claim_due(
            start + UPDATE_STARTUP_DELAY + UPDATE_CHECK_INTERVAL - Duration::from_millis(1)
        ));
        assert!(schedule.claim_due(start + UPDATE_STARTUP_DELAY + UPDATE_CHECK_INTERVAL));
    }

    #[test]
    fn failed_check_waits_for_the_next_daily_window() {
        let start = Instant::now();
        let first_due = start + UPDATE_STARTUP_DELAY;
        let mut schedule = UpdateSchedule::new(start);

        assert!(schedule.claim_due(first_due));
        assert!(!schedule.claim_due(first_due + Duration::from_secs(15)));
    }

    #[test]
    fn missed_intervals_are_not_replayed_as_a_retry_burst() {
        let start = Instant::now();
        let first_due = start + UPDATE_STARTUP_DELAY;
        let wake = first_due + UPDATE_CHECK_INTERVAL * 4;
        let mut schedule = UpdateSchedule::new(start);

        assert!(schedule.claim_due(wake));
        assert!(!schedule.claim_due(wake));
        assert!(!schedule.claim_due(wake + UPDATE_CHECK_INTERVAL - Duration::from_millis(1)));
        assert!(schedule.claim_due(wake + UPDATE_CHECK_INTERVAL));
    }

    #[test]
    fn update_policy_distinguishes_installed_and_portable_builds() {
        let app_executable = Path::new("/Applications/Metra.app/Contents/MacOS/metra");
        let external_app = Path::new("/Volumes/Apps/Metra.app/Contents/MacOS/metra");

        assert_eq!(
            update_mode(
                false,
                DesktopPlatform::Windows,
                Some(BundleType::Nsis),
                None,
            ),
            AppUpdateMode::InApp
        );
        assert_eq!(
            update_mode(false, DesktopPlatform::Windows, Some(BundleType::Msi), None,),
            AppUpdateMode::InApp
        );
        assert_eq!(
            update_mode(false, DesktopPlatform::Windows, None, None),
            AppUpdateMode::ManualDownload
        );
        assert_eq!(
            update_mode(
                false,
                DesktopPlatform::Macos,
                Some(BundleType::App),
                Some(app_executable),
            ),
            AppUpdateMode::ManualDownload
        );
        assert_eq!(
            update_mode(
                false,
                DesktopPlatform::Macos,
                Some(BundleType::App),
                Some(external_app),
            ),
            AppUpdateMode::ManualDownload
        );
        assert_eq!(
            update_mode(
                false,
                DesktopPlatform::Macos,
                Some(BundleType::App),
                Some(Path::new("target/release/metra")),
            ),
            AppUpdateMode::Disabled
        );
        assert_eq!(
            update_mode(true, DesktopPlatform::Windows, Some(BundleType::Nsis), None,),
            AppUpdateMode::Disabled
        );
        assert_eq!(
            update_mode(true, DesktopPlatform::Windows, None, None),
            AppUpdateMode::Disabled
        );
        assert_eq!(
            update_mode(false, DesktopPlatform::Other, None, None),
            AppUpdateMode::Disabled
        );
    }

    #[test]
    fn macos_bundle_path_requires_exact_app_contents_macos_layers() {
        assert_eq!(
            macos_app_bundle_from_executable(Path::new(
                "/Applications/Metra.app/Contents/MacOS/metra"
            )),
            Some(Path::new("/Applications/Metra.app").to_path_buf())
        );
        assert!(macos_app_bundle_from_executable(Path::new("/tmp/Contents/MacOS/metra")).is_none());
        assert!(
            macos_app_bundle_from_executable(Path::new("/tmp/not-Contents/MacOS/metra")).is_none()
        );
        assert!(
            macos_app_bundle_from_executable(Path::new("/tmp/Metra.app/Contents/not-MacOS/metra"))
                .is_none()
        );
    }

    #[test]
    fn check_and_install_operations_share_one_exclusive_state() {
        let updater = Arc::new(AppUpdaterState::new(AppUpdateMode::InApp));
        let attempts = (0..8)
            .map(|_| {
                let updater = updater.clone();
                std::thread::spawn(move || updater.begin_check())
            })
            .collect::<Vec<_>>();
        let accepted = attempts
            .into_iter()
            .map(|attempt| attempt.join().expect("check attempt panicked"))
            .filter(|accepted| *accepted)
            .count();

        assert_eq!(accepted, 1);
        assert!(updater.begin_download().is_none());
        updater.finish_check_deferred();
        assert!(updater.begin_check());
    }

    #[test]
    fn manual_download_mode_checks_but_never_keeps_an_install_payload() {
        let updater = AppUpdaterState::new(AppUpdateMode::ManualDownload);

        assert!(updater.status().supported);
        assert!(updater.begin_check());
        let status = updater
            .finish_manual_check_available(ManualUpdate {
                version: "0.1.40".to_string(),
                notes: Some("Manual download".to_string()),
            })
            .expect("manual update should become available");

        assert_eq!(status.phase, AppUpdatePhase::Available);
        assert_eq!(status.version.as_deref(), Some("0.1.40"));
        assert_eq!(status.notes.as_deref(), Some("Manual download"));
        assert!(updater.lock_inner().pending.is_none());
        assert!(updater.begin_download().is_none());
    }

    #[test]
    fn manual_manifest_reads_only_release_metadata_and_compares_semver() {
        let manifest = br#"{
            "version": "0.10.0",
            "notes": "A newer stable release",
            "platforms": {
                "windows-x86_64": {
                    "signature": "unused by manual checks",
                    "url": "https://example.invalid/unused.exe"
                }
            }
        }"#;

        assert_eq!(
            parse_manual_update_manifest(manifest, "0.9.99"),
            Ok(Some(ManualUpdate {
                version: "0.10.0".to_string(),
                notes: Some("A newer stable release".to_string()),
            }))
        );
        assert_eq!(parse_manual_update_manifest(manifest, "0.10.0"), Ok(None));
        assert_eq!(parse_manual_update_manifest(manifest, "1.0.0"), Ok(None));
    }

    #[test]
    fn manual_manifest_rejects_non_stable_or_oversized_input() {
        assert!(parse_stable_version("0.1.40").is_some());
        assert!(parse_stable_version("0.1.40-beta.1").is_none());
        assert!(parse_stable_version("0.1.40+build.1").is_none());
        assert!(parse_stable_version("00.1.40").is_none());
        assert!(parse_stable_version("18446744073709551616.1.0").is_none());

        assert!(parse_manual_update_manifest(br#"{"version":"0.1.40-beta.1"}"#, "0.1.39").is_err());
        assert!(
            parse_manual_update_manifest(
                &vec![b' '; UPDATE_MANIFEST_MAX_BYTES as usize + 1],
                "0.1.39"
            )
            .is_err()
        );
    }

    #[test]
    fn failed_manual_check_is_silent_and_a_later_check_can_run() {
        let updater = AppUpdaterState::new(AppUpdateMode::ManualDownload);

        assert!(updater.begin_check());
        updater.finish_check_deferred();

        let status = updater.status();
        assert_eq!(status.phase, AppUpdatePhase::Idle);
        assert_eq!(status.revision, 0);
        assert!(status.version.is_none());
        assert!(updater.begin_check());
    }

    #[test]
    fn successful_manual_no_update_clears_stale_availability() {
        let updater = AppUpdaterState::new(AppUpdateMode::ManualDownload);
        assert!(updater.begin_check());
        updater
            .finish_manual_check_available(ManualUpdate {
                version: "0.1.40".to_string(),
                notes: None,
            })
            .expect("manual update should become available");

        assert!(updater.begin_check());
        let status = updater
            .finish_check_idle()
            .expect("stale availability should be cleared");

        assert_eq!(status.phase, AppUpdatePhase::Idle);
        assert!(status.version.is_none());
        assert!(status.notes.is_none());
    }

    #[test]
    fn manual_download_page_is_pinned_to_the_detected_stable_tag() {
        assert_eq!(
            manual_download_page("0.1.39").as_deref(),
            Some("https://github.com/eyotang/Metra/releases/tag/v0.1.39")
        );
        assert!(manual_download_page("0.1.39-beta.1").is_none());
        assert!(manual_download_page("0.1.39/../../settings").is_none());
        assert!(manual_download_page("01.1.39").is_none());
    }
}
