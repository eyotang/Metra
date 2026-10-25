//! Release discovery and signed Windows installation, independent of the UI framework.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const UPDATE_PUBLIC_KEY: &str = include_str!("../assets/update-public-key.txt");
const MANIFEST_URL: &str = "https://github.com/eyotang/Metra/releases/latest/download/latest.json";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize)]
pub struct UpdateStatus {
    pub version: Option<String>,
    pub notes: Option<String>,
    pub error: Option<String>,
    pub downloading: bool,
}
#[cfg(windows)]
#[derive(Clone, Debug, Deserialize)]
struct Artifact {
    url: String,
    signature: String,
}
#[derive(Clone, Debug, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[cfg(windows)]
    #[serde(default)]
    platforms: std::collections::BTreeMap<String, Artifact>,
}
#[derive(Default)]
pub struct Updater {
    status: Mutex<UpdateStatus>,
    manifest: Mutex<Option<Manifest>>,
    checking: AtomicBool,
}

fn version(value: &str) -> Option<[u64; 3]> {
    let parts = value
        .split('.')
        .map(|part| {
            if part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                None
            } else {
                part.parse().ok()
            }
        })
        .collect::<Option<Vec<_>>>()?;
    parts.try_into().ok()
}
fn parse_manifest(bytes: &[u8], current: &str) -> Result<Option<Manifest>, String> {
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Update manifest exceeds the size limit".into());
    }
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let available = version(&manifest.version).ok_or("Invalid release version")?;
    let current = version(current).ok_or("Invalid current version")?;
    Ok((available > current).then_some(manifest))
}
pub fn release_page(version: &str) -> Option<String> {
    self::version(version)?;
    Some(format!(
        "https://github.com/eyotang/Metra/releases/tag/v{version}"
    ))
}
fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Metra/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || attempt.url().scheme() != "https" {
                attempt.error("Invalid update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| error.to_string())
}
impl Updater {
    pub fn status(&self) -> UpdateStatus {
        self.status.lock().expect("update status").clone()
    }
    pub fn check(&self) {
        if self.checking.swap(true, Ordering::AcqRel) {
            return;
        }
        let result = (|| {
            let response = client()?
                .get(MANIFEST_URL)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map_err(|error| error.to_string())?;
            let mut bytes = Vec::new();
            response
                .take(MAX_MANIFEST_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            parse_manifest(&bytes, env!("CARGO_PKG_VERSION"))
        })();
        match result {
            Ok(manifest) => {
                let mut status = self.status.lock().expect("update status");
                status.version = manifest.as_ref().map(|manifest| manifest.version.clone());
                status.notes = manifest
                    .as_ref()
                    .and_then(|manifest| manifest.notes.clone());
                status.error = None;
                *self.manifest.lock().expect("update manifest") = manifest;
            }
            Err(error) => {
                crate::diagnostics::warn("updater.check_failed", &error);
                self.status.lock().expect("update status").error = Some(error);
            }
        }
        self.checking.store(false, Ordering::Release);
    }
    pub fn supports_install(&self) -> bool {
        #[cfg(windows)]
        {
            let Some(local) = std::env::var_os("LOCALAPPDATA") else {
                return false;
            };
            let expected = std::path::PathBuf::from(local).join("Metra");
            std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent == expected))
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        false
    }
    pub fn install_windows(&self) -> Result<(), String> {
        #[cfg(not(windows))]
        return Err("This distribution uses manual updates".into());
        #[cfg(windows)]
        {
            if !self.supports_install() {
                return Err("Portable installations use manual updates".into());
            }
            let manifest = self
                .manifest
                .lock()
                .map_err(|_| "Update state unavailable")?
                .clone()
                .ok_or("No update available")?;
            let target = if cfg!(target_arch = "aarch64") {
                "windows-aarch64"
            } else {
                "windows-x86_64"
            };
            let artifact = manifest
                .platforms
                .get(target)
                .ok_or("No installer for this target")?;
            validate_artifact_url(&artifact.url, &manifest.version)?;
            {
                let mut status = self.status.lock().map_err(|_| "Update state unavailable")?;
                if status.downloading {
                    return Err("Update already in progress".into());
                }
                status.downloading = true;
            }
            let result = (|| {
                let response = client()?
                    .get(&artifact.url)
                    .timeout(Duration::from_secs(300))
                    .send()
                    .and_then(reqwest::blocking::Response::error_for_status)
                    .map_err(|error| error.to_string())?;
                let mut bytes = Vec::new();
                response
                    .take(MAX_DOWNLOAD_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                verify_artifact(&bytes, &artifact.signature)?;
                let directory = tempfile::Builder::new()
                    .prefix("metra-update-")
                    .tempdir()
                    .map_err(|error| error.to_string())?;
                let installer = directory.path().join("Metra-setup.exe");
                std::fs::write(&installer, bytes).map_err(|error| error.to_string())?;
                let mut command = std::process::Command::new(&installer);
                crate::providers::scrub_sensitive_child_environment(&mut command);
                command
                    .args(["/S", "/R", "/UPDATE"])
                    .spawn()
                    .map_err(|error| error.to_string())?;
                // The installer owns its staged files until it finishes after this process exits.
                let _ = directory.keep();
                Ok(())
            })();
            let mut status = self.status.lock().map_err(|_| "Update state unavailable")?;
            status.downloading = false;
            status.error = result.as_ref().err().cloned();
            result
        }
    }
}
#[cfg(any(windows, test))]
fn validate_artifact_url(value: &str, version: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid update URL")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url
            .path()
            .starts_with(&format!("/eyotang/Metra/releases/download/v{version}/"))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Update artifact must belong to the selected official release".into());
    }
    Ok(())
}
pub fn verify_artifact(bytes: &[u8], encoded_signature: &str) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err("Invalid update download size".into());
    }
    let public_key = STANDARD
        .decode(UPDATE_PUBLIC_KEY.trim())
        .map_err(|_| "Invalid update public key")?;
    let public_key = minisign_verify::PublicKey::decode(
        std::str::from_utf8(&public_key).map_err(|_| "Invalid update public key")?,
    )
    .map_err(|error| error.to_string())?;
    let signature = STANDARD
        .decode(encoded_signature.trim())
        .map_err(|_| "Invalid update signature")?;
    let signature = minisign_verify::Signature::decode(
        std::str::from_utf8(&signature).map_err(|_| "Invalid update signature")?,
    )
    .map_err(|error| error.to_string())?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|error| error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_new_stable_versions_are_offered() {
        assert!(
            parse_manifest(br#"{"version":"0.3.0"}"#, "0.2.2")
                .unwrap()
                .is_some()
        );
        assert!(
            parse_manifest(br#"{"version":"0.2.2"}"#, "0.2.2")
                .unwrap()
                .is_none()
        );
        for value in ["../bad", "1.0", "01.0.0", "1.0.0-beta", "1.0.0.1"] {
            assert!(version(value).is_none());
        }
    }
    #[test]
    fn installation_requires_the_exact_official_release_and_a_valid_signature() {
        assert!(
            validate_artifact_url(
                "https://github.com/eyotang/Metra/releases/download/v0.3.0/setup.exe",
                "0.3.0"
            )
            .is_ok()
        );
        for url in [
            "http://github.com/eyotang/Metra/releases/download/v0.3.0/setup.exe",
            "https://evil.example/setup.exe",
            "https://github.com/other/Metra/releases/download/v0.3.0/setup.exe",
            "https://github.com/eyotang/Metra/releases/download/v0.4.0/setup.exe",
        ] {
            assert!(validate_artifact_url(url, "0.3.0").is_err());
        }
        assert!(verify_artifact(b"untrusted executable", "invalid").is_err());
        assert!(release_page("../../bad").is_none());
    }
}
