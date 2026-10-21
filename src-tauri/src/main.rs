#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    match std::env::args().nth(1).as_deref() {
        Some("--usage-json") => {
            if let Err(error) = metra_lib::headless::run(false) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        Some("--watch-json") => {
            if let Err(error) = metra_lib::headless::run(true) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }
    metra_lib::run();
}
