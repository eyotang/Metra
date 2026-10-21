//! Line-oriented usage transport for native Linux desktop widgets; never initializes GTK.
use std::{
    io::{self, BufRead, Write},
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::{service::RefreshService, settings::SettingsStore};

pub fn run(watch: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = dirs::config_dir().ok_or("Configuration directory is unavailable")?;
    let service = RefreshService::new(SettingsStore::new(
        config.join("dev.metra.desktop/settings.db"),
    ));
    let mut output = io::BufWriter::new(io::stdout());
    if !watch {
        service.refresh(true);
        return emit(&service, &mut output);
    }

    // Read independently of collection, so UI requests cannot block the pipe.
    // A bounded channel coalesces refresh clicks while a collection is running.
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) if line.trim() == "refresh" => {
                    let _ = sender.try_send(());
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        // Dropping the sender makes the process exit when its shell owner closes stdin.
    });

    emit(&service, &mut output)?;
    loop {
        service.refresh(true);
        emit(&service, &mut output)?;
        let interval = service
            .settings
            .lock()
            .map_err(|_| "Settings lock failed")?
            .refresh_minutes
            .clamp(1, 60)
            * 60;
        let next = Instant::now() + Duration::from_secs(interval);
        loop {
            match receiver.recv_timeout(next.saturating_duration_since(Instant::now())) {
                Ok(()) => {
                    // Bound manual refresh frequency even if stdin is written repeatedly.
                    let elapsed = next - Duration::from_secs(interval);
                    if elapsed.elapsed() >= Duration::from_secs(10) {
                        break;
                    }
                    emit(&service, &mut output)?;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
        // Reload display preferences and explicit compatibility consent between rounds.
        let settings = service.store.load();
        service.cursor_compat.store(
            settings.cursor_compat_enabled,
            std::sync::atomic::Ordering::Release,
        );
        *service
            .settings
            .lock()
            .map_err(|_| "Settings lock failed")? = settings;
    }
}

fn emit(
    service: &RefreshService,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer(&mut *output, &service.payload())?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}
