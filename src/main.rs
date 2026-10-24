#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mode = std::env::args().nth(1);
    if matches!(mode.as_deref(), Some("--usage-json" | "--watch-json")) {
        if let Err(error) = metra_lib::headless::run(mode.as_deref() == Some("--watch-json")) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(feature = "desktop")]
    metra_lib::app::run();
    #[cfg(not(feature = "desktop"))]
    eprintln!("This collector-only build supports --usage-json and --watch-json.");
}
