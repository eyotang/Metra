use std::{env, fs};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use metra_lib::updater::verify_artifact;
use minisign_verify::PublicKey;

fn configured_public_key() -> PublicKey {
    let encoded = metra_lib::updater::UPDATE_PUBLIC_KEY.trim();
    let decoded = STANDARD.decode(encoded).unwrap();
    PublicKey::decode(std::str::from_utf8(&decoded).unwrap()).unwrap()
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
    let signature = fs::read_to_string(signature).unwrap();
    let mut artifact = fs::read(artifact).unwrap();
    verify_artifact(&artifact, &signature).unwrap();
    artifact[0] ^= 1;
    assert!(verify_artifact(&artifact, &signature).is_err());
}
