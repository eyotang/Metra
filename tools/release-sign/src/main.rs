use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{env, fs, path::PathBuf};
use zeroize::Zeroizing;
fn main() -> Result<()> {
    let artifact = PathBuf::from(
        env::args_os()
            .nth(1)
            .context("Usage: metra-release-sign <artifact>")?,
    );
    let encoded = Zeroizing::new(
        env::var("METRA_SIGNING_PRIVATE_KEY")
            .or_else(|_| env::var("TAURI_SIGNING_PRIVATE_KEY"))
            .context("Release signing key is not configured")?,
    );
    let password = env::var("METRA_SIGNING_PRIVATE_KEY_PASSWORD")
        .or_else(|_| env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD"))
        .unwrap_or_default();
    let signature = sign_encoded(&encoded, password, fs::File::open(&artifact)?)?;
    fs::write(format!("{}.sig", artifact.display()), signature)?;
    Ok(())
}

fn sign_encoded(encoded: &str, password: String, reader: impl std::io::Read) -> Result<String> {
    let decoded = Zeroizing::new(
        STANDARD
            .decode(encoded.trim())
            .context("Invalid encoded signing key")?,
    );
    let boxed = minisign::SecretKeyBox::from_string(std::str::from_utf8(&decoded)?)?;
    let secret = boxed.into_secret_key(Some(password))?;
    Ok(STANDARD.encode(minisign::sign(None, &secret, reader, None, None)?.to_string()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_release_keys_produce_compatible_minisign_signatures() {
        let pair = minisign::KeyPair::generate_encrypted_keypair(Some(String::new())).unwrap();
        let encoded = Zeroizing::new(STANDARD.encode(pair.sk.to_box(None).unwrap().to_string()));
        let bytes = b"release artifact";
        let encoded_signature = sign_encoded(&encoded, String::new(), bytes.as_slice()).unwrap();
        let raw = STANDARD.decode(encoded_signature).unwrap();
        let signature =
            minisign::SignatureBox::from_string(std::str::from_utf8(&raw).unwrap()).unwrap();
        minisign::verify(
            &pair.pk,
            &signature,
            std::io::Cursor::new(bytes),
            true,
            false,
            true,
        )
        .unwrap();
    }
}
