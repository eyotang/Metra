pub mod codex;
pub mod claude;
pub mod cursor;
pub mod discovery;

use std::path::PathBuf;

use crate::model::{ProviderSnapshot, ProviderStatus};

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
