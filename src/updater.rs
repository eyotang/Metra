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
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/eyotang/Metra/releases/latest";
const RELEASES_URL: &str = "https://github.com/eyotang/Metra/releases";
const MAX_RELEASE_BYTES: u64 = 256 * 1024;
const MAX_RELEASE_NOTES_CHARS: usize = 8192;
#[cfg(windows)]
const MAX_SIGNATURE_BYTES: u64 = 4096;
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize)]
pub struct UpdateStatus {
    pub version: Option<String>,
    pub notes: Option<String>,
    pub error: Option<String>,
    pub downloading: bool,
    #[cfg(any(windows, test))]
    #[serde(skip)]
    windows_installer: bool,
}
#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    state: String,
    size: u64,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    #[cfg(any(windows, test))]
    assets: Vec<ReleaseAsset>,
}
#[derive(Default)]
pub struct Updater {
    status: Mutex<UpdateStatus>,
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
fn parse_release(bytes: &[u8], current: &str) -> Result<UpdateStatus, String> {
    let release: Release = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if release.draft || release.prerelease {
        return Ok(UpdateStatus::default());
    }
    let tag = release
        .tag_name
        .strip_prefix('v')
        .ok_or("Invalid release tag")?;
    let available = version(tag).ok_or("Invalid release version")?;
    let current = version(current).ok_or("Invalid current version")?;
    if available <= current {
        return Ok(UpdateStatus::default());
    }
    #[cfg(any(windows, test))]
    let windows_installer = {
        let installer = windows_installer_name(tag);
        let signature = format!("{installer}.sig");
        [installer, signature].iter().all(|name| {
            release
                .assets
                .iter()
                .any(|asset| asset.name == *name && asset.state == "uploaded" && asset.size > 0)
        })
    };
    Ok(UpdateStatus {
        version: Some(tag.into()),
        notes: release
            .body
            .as_deref()
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(|notes| notes.chars().take(MAX_RELEASE_NOTES_CHARS).collect()),
        #[cfg(any(windows, test))]
        windows_installer,
        ..UpdateStatus::default()
    })
}
pub fn release_page(version: &str) -> Option<String> {
    self::version(version)?;
    Some(format!("{RELEASES_URL}/tag/v{version}"))
}
#[cfg(any(windows, test))]
fn windows_installer_name(version: &str) -> String {
    format!("Metra-{version}-windows-x64-setup.exe")
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
fn download(request: reqwest::blocking::RequestBuilder, limit: u64) -> Result<Vec<u8>, String> {
    let response = request
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Update response exceeds the size limit".into());
    }
    Ok(bytes)
}

fn fetch_latest_release(current: &str) -> Result<UpdateStatus, String> {
    let request = client()?
        .get(LATEST_RELEASE_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    parse_release(&download(request, MAX_RELEASE_BYTES)?, current)
}

impl Updater {
    pub fn available_version(&self) -> Option<String> {
        self.status.lock().expect("update status").version.clone()
    }
    pub fn status(&self) -> UpdateStatus {
        self.status.lock().expect("update status").clone()
    }
    pub fn check(&self) {
        if self.checking.swap(true, Ordering::AcqRel) {
            return;
        }
        match fetch_latest_release(env!("CARGO_PKG_VERSION")) {
            Ok(mut release) => {
                crate::diagnostics::info(
                    if release.version.is_some() {
                        "updater.available"
                    } else {
                        "updater.current"
                    },
                    format!(
                        "current={} available={}",
                        env!("CARGO_PKG_VERSION"),
                        release.version.as_deref().unwrap_or("none")
                    ),
                );
                let mut status = self.status.lock().expect("update status");
                release.downloading = status.downloading;
                *status = release;
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
            self.status.lock().expect("update status").windows_installer
                && is_installed_windows_x64()
        }
        #[cfg(not(windows))]
        false
    }
    pub fn install_windows(&self) -> Result<(), String> {
        #[cfg(not(windows))]
        return Err("This distribution uses manual updates".into());
        #[cfg(windows)]
        {
            if !is_installed_windows_x64() {
                return Err("Portable installations use manual updates".into());
            }
            let version = {
                let mut status = self.status.lock().map_err(|_| "Update state unavailable")?;
                if status.downloading {
                    return Err("Update already in progress".into());
                }
                let version = status.version.clone().ok_or("No update available")?;
                if !status.windows_installer {
                    return Err("No signed installer for this release".into());
                }
                status.downloading = true;
                version
            };
            let result = (|| {
                let installer_name = windows_installer_name(&version);
                let url = format!("{RELEASES_URL}/download/v{version}/{installer_name}");
                let client = client()?;
                let signature = download(client.get(format!("{url}.sig")), MAX_SIGNATURE_BYTES)?;
                let signature =
                    std::str::from_utf8(&signature).map_err(|_| "Invalid update signature")?;
                let bytes = download(
                    client.get(url).timeout(Duration::from_secs(300)),
                    MAX_DOWNLOAD_BYTES,
                )?;
                verify_artifact(&bytes, signature)?;
                let directory = tempfile::Builder::new()
                    .prefix("metra-update-")
                    .tempdir()
                    .map_err(|error| error.to_string())?;
                let installer = directory.path().join("Metra-setup.exe");
                std::fs::write(&installer, bytes).map_err(|error| error.to_string())?;
                let mut command = std::process::Command::new(&installer);
                crate::providers::scrub_sensitive_child_environment(&mut command);
                command
                    .args(["/S", "/NS"])
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
#[cfg(windows)]
fn is_installed_windows_x64() -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let expected = std::path::PathBuf::from(local).join("Metra");
    cfg!(target_arch = "x86_64")
        && std::env::current_exe().is_ok_and(|path| path.parent() == Some(expected.as_path()))
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
    use serde_json::json;

    fn release(tag: &str) -> serde_json::Value {
        json!({
            "tag_name": tag, "draft": false, "prerelease": false,
            "body": "Changes", "assets": []
        })
    }

    #[test]
    #[ignore = "requires access to the official release endpoint using the host network configuration"]
    fn official_update_endpoint_is_reachable() {
        let status = fetch_latest_release("0.0.0").unwrap();
        assert!(status.version.is_some());
        assert!(status.windows_installer);
    }

    #[test]
    fn only_new_stable_versions_are_offered() {
        let mut stable = release("v0.5.10");
        let parse =
            |value: &serde_json::Value| parse_release(value.to_string().as_bytes(), "0.5.9");
        let status = parse(&stable).unwrap();
        assert_eq!(status.version.as_deref(), Some("0.5.10"));
        assert_eq!(status.notes.as_deref(), Some("Changes"));
        for tag in ["v0.5.8", "v0.5.9"] {
            assert!(parse(&release(tag)).unwrap().version.is_none());
        }
        stable["draft"] = json!(true);
        assert!(parse(&stable).unwrap().version.is_none());
        let mut preview = release("v0.6.0-beta");
        preview["prerelease"] = json!(true);
        assert!(parse(&preview).unwrap().version.is_none());
        for tag in ["../bad", "0.6.0", "v1.0", "v01.0.0", "v1.0.0.1"] {
            assert!(parse(&release(tag)).is_err());
        }
    }

    #[test]
    fn in_app_install_requires_both_assets_from_the_selected_release() {
        let mut release = release("v0.6.1");
        release["assets"] = json!([
            {"name": "Metra-0.6.1-windows-x64-setup.exe", "state": "uploaded", "size": 30},
            {"name": "Metra-0.6.0-windows-x64-setup.exe.sig", "state": "uploaded", "size": 10}
        ]);
        let parse = |value: &serde_json::Value| {
            parse_release(value.to_string().as_bytes(), "0.6.0").unwrap()
        };
        assert!(!parse(&release).windows_installer);
        release["assets"][1]["name"] = json!("Metra-0.6.1-windows-x64-setup.exe.sig");
        release["assets"][1]["state"] = json!("starter");
        assert!(!parse(&release).windows_installer);
        release["assets"][1]["state"] = json!("uploaded");
        assert!(parse(&release).windows_installer);
        release["assets"][1]["size"] = json!(0);
        let status = parse(&release);
        assert!(!status.windows_installer);
        assert_eq!(status.version.as_deref(), Some("0.6.1"));
    }

    #[test]
    fn rejects_invalid_signatures_and_release_links() {
        assert!(verify_artifact(b"untrusted executable", "invalid").is_err());
        assert!(release_page("../../bad").is_none());
    }
}
