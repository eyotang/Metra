#[cfg(feature = "desktop")]
pub mod app;
pub mod desktop_actions;
pub mod diagnostics;
pub mod headless;
pub mod i18n;
pub mod model;
#[cfg(feature = "desktop")]
mod platform;
pub mod providers;
pub mod service;
pub mod settings;
pub mod settings_change;
pub mod status_bar;
pub mod updater;

#[cfg(feature = "desktop")]
mod tray;

pub mod geometry;
