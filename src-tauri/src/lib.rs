pub mod app;
pub mod diagnostics;
#[cfg(target_os = "linux")]
pub mod headless;
pub mod model;
mod platform;
pub mod providers;
pub mod service;
pub mod settings;
pub mod updater;

pub use app::run;
