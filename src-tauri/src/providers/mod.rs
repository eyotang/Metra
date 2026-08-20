pub mod codex;
pub mod claude;
pub mod cursor;
pub mod discovery;

use std::{path::PathBuf, process::Command};

use crate::model::{ProviderSnapshot, ProviderStatus};

pub(crate) const ANTHROPIC_ADMIN_KEY_ENV: &str = "ANTHROPIC_ADMIN_KEY";
pub(crate) const CLAUDE_API_KEY_NAME_ENV: &str = "METRA_CLAUDE_API_KEY_NAME";

pub(crate) fn scrub_sensitive_child_environment(command: &mut Command) {
    command
        .env_remove(ANTHROPIC_ADMIN_KEY_ENV)
        .env_remove(CLAUDE_API_KEY_NAME_ENV);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub executable: Option<PathBuf>,
    pub status: ProviderStatus,
    pub detail: Option<String>,
}

pub trait UsageProvider: Send + Sync {
    fn detect(&self) -> Detection;
    fn account_status(&self) -> ProviderSnapshot;
    fn refresh(&self) -> ProviderSnapshot;
}
