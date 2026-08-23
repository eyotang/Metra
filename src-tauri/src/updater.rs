//! Signed application updates backed by the latest public GitHub Release.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, utils::config::BundleType};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::diagnostics;

const UPDATE_EVENT: &str = "app-update-status";
const UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(20);
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const UPDATE_SCHEDULER_TICK: Duration = Duration::from_secs(5);
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

    fn finish_check_available(&self, mut update: Update) -> Option<AppUpdateStatus> {
        let mut state = self.lock_inner();
        if state.operation != UpdateOperation::Checking {
            return None;
        }
        update.timeout = Some(UPDATE_DOWNLOAD_TIMEOUT);
        state.status.revision = state.status.revision.saturating_add(1);
        state.status.phase = AppUpdatePhase::Available;
        state.status.version = Some(update.version.clone());
        state.status.notes = update.body.clone();
        state.status.downloaded_bytes = 0;
        state.status.total_bytes = None;
        state.pending = should_retain_pending_update(state.status.mode).then_some(update);
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
            // final rename fails (upstream issue #3505). Keep signed update
            // discovery, but direct macOS users to the notarized DMG for now.
            AppUpdateMode::ManualDownload
        }
        _ => AppUpdateMode::Disabled,
    }
}

fn should_retain_pending_update(mode: AppUpdateMode) -> bool {
    mode == AppUpdateMode::InApp
}

fn manual_download_page(version: &str) -> Option<String> {
    let mut segments = version.split('.');
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment.bytes().all(|byte| byte.is_ascii_digit())
            && (segment == "0" || !segment.starts_with('0'))
    };
    let major = segments.next()?;
    let minor = segments.next()?;
    let patch = segments.next()?;
    if segments.next().is_some()
        || !valid_segment(major)
        || !valid_segment(minor)
        || !valid_segment(patch)
    {
        return None;
    }
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
                if let Some(status) = updater.finish_check_available(update) {
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
        AppUpdateMode, AppUpdaterState, DesktopPlatform, UPDATE_CHECK_INTERVAL,
        UPDATE_STARTUP_DELAY, UpdateSchedule, macos_app_bundle_from_executable,
        manual_download_page, should_retain_pending_update, update_mode,
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
        assert!(updater.begin_download().is_none());
        assert!(!should_retain_pending_update(AppUpdateMode::ManualDownload));
        assert!(should_retain_pending_update(AppUpdateMode::InApp));
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
