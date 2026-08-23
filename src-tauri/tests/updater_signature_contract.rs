use std::{env, fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;

fn configured_public_key() -> PublicKey {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    let encoded = config["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let decoded = STANDARD.decode(encoded).unwrap();
    PublicKey::decode(std::str::from_utf8(&decoded).unwrap()).unwrap()
}

fn release_signature(path: &Path) -> Signature {
    let encoded = fs::read_to_string(path).unwrap();
    let decoded = STANDARD.decode(encoded.trim()).unwrap();
    Signature::decode(std::str::from_utf8(&decoded).unwrap()).unwrap()
}

#[test]
fn embedded_updater_public_key_is_well_formed() {
    assert!(configured_public_key().untrusted_comment().is_some());
}

#[test]
#[ignore = "release workflow supplies a platform updater artifact and signature"]
fn release_artifact_matches_embedded_public_key() {
    let artifact = env::var_os("METRA_UPDATER_ARTIFACT").unwrap();
    let signature = env::var_os("METRA_UPDATER_SIGNATURE").unwrap();
    let public_key = configured_public_key();
    let signature = release_signature(Path::new(&signature));
    let artifact = fs::read(artifact).unwrap();

    public_key.verify(&artifact, &signature, true).unwrap();
}
