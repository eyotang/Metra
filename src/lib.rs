#[cfg(feature = "desktop")]
pub mod app;
pub mod diagnostics;
pub mod headless;
pub mod i18n;
pub mod model;
#[cfg(feature = "desktop")]
mod platform;
pub mod providers;
pub mod service;
pub mod settings;
#[cfg(any(feature = "desktop", test))]
pub(crate) mod status_bar;
pub mod updater;

#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
mod geometry;
