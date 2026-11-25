use std::{path::Path, ptr, slice};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::aead::{self, AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use serde::Deserialize;
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData},
};
use zeroize::{Zeroize, Zeroizing};

use super::CursorError;

pub(super) fn decrypt(encrypted: &str, profile: &Path) -> Result<String, CursorError> {
    let mut bytes = Zeroizing::new(
        STANDARD
            .decode(encrypted)
            .map_err(|_| CursorError::Decryption)?,
    );
    // Electron's Windows safeStorage uses Chromium's v10 envelope and no associated data.
    let version = b"v10";
    if !bytes.starts_with(version) {
        return Err(CursorError::Decryption);
    }
    let (nonce, ciphertext) = bytes[version.len()..]
        .split_at_mut_checked(aead::NONCE_LEN)
        .ok_or(CursorError::Decryption)?;
    if ciphertext.len() <= AES_256_GCM.tag_len() {
        return Err(CursorError::Decryption);
    }
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| CursorError::Decryption)?;
    let key = load_key(profile)?;
    let token = key
        .open_in_place(nonce, Aad::empty(), ciphertext)
        .map_err(|_| CursorError::Decryption)?;
    std::str::from_utf8(token)
        .map(str::to_owned)
        .map_err(|_| CursorError::Decryption)
}

fn load_key(profile: &Path) -> Result<LessSafeKey, CursorError> {
    #[derive(Deserialize)]
    struct LocalState {
        os_crypt: OsCrypt,
    }
    #[derive(Deserialize)]
    struct OsCrypt {
        encrypted_key: String,
    }

    let bytes = std::fs::read(profile.join("Local State")).map_err(|_| CursorError::Decryption)?;
    let state: LocalState = serde_json::from_slice(&bytes).map_err(|_| CursorError::Decryption)?;
    let wrapped = STANDARD
        .decode(&state.os_crypt.encrypted_key)
        .map_err(|_| CursorError::Decryption)?;
    let protected = wrapped
        .strip_prefix(b"DPAPI")
        .filter(|bytes| !bytes.is_empty())
        .ok_or(CursorError::Decryption)?;
    let key = unprotect(protected)?;
    UnboundKey::new(&AES_256_GCM, &key)
        .map(LessSafeKey::new)
        .map_err(|_| CursorError::Decryption)
}

fn unprotect(protected: &[u8]) -> Result<Zeroizing<Vec<u8>>, CursorError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(protected.len()).map_err(|_| CursorError::Decryption)?,
        pbData: protected.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // DPAPI reads the input without changing it and allocates the output with LocalAlloc.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err(CursorError::Decryption);
    }
    // On success the caller must clear and free the writable output buffer.
    let key = unsafe {
        let bytes = slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let key = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        LocalFree(output.pbData.cast());
        key
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::super::{PREFIX, decode_uncached};
    use super::*;
    use windows_sys::Win32::Security::Cryptography::CryptProtectData;

    fn wrap_fixture_key(key: &mut [u8; 32]) -> String {
        let input = CRYPT_INTEGER_BLOB {
            cbData: key.len() as u32,
            pbData: key.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let ok = unsafe {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        assert_ne!(ok, 0);
        assert!(!output.pbData.is_null());
        let mut wrapped = b"DPAPI".to_vec();
        unsafe {
            wrapped.extend_from_slice(slice::from_raw_parts(output.pbData, output.cbData as usize));
            LocalFree(output.pbData.cast());
        }
        STANDARD.encode(wrapped)
    }

    #[test]
    fn decrypts_chromium_fixture_with_current_user_dpapi() {
        // AES-256-GCM known answer using a 0x42 key and a 0x23 nonce, generated with Node crypto.
        let ciphertext =
            "djEwIyMjIyMjIyMjIyMjPKCBBHqmMaYQKm46+ORu8AVg7Qc7w5Wt8s4Z8YTHnG4cXeEauh0Q+Q==";
        let profile = tempfile::tempdir().unwrap();
        let wrapped = wrap_fixture_key(&mut [0x42; 32]);
        let local_state = profile.path().join("Local State");
        std::fs::write(
            &local_state,
            serde_json::json!({"os_crypt": {"encrypted_key": wrapped}}).to_string(),
        )
        .unwrap();

        let token = format!("{PREFIX}{ciphertext}");
        assert_eq!(
            decode_uncached(&token, Some(profile.path())).unwrap(),
            "header.payload.signature"
        );

        let mut tampered = STANDARD.decode(ciphertext).unwrap();
        *tampered.last_mut().unwrap() ^= 1;
        let tampered = format!("{PREFIX}{}", STANDARD.encode(tampered));
        assert_eq!(
            decode_uncached(&tampered, Some(profile.path())),
            Err(CursorError::Decryption)
        );

        std::fs::remove_file(local_state).unwrap();
        assert_eq!(
            decode_uncached(&token, Some(profile.path())),
            Err(CursorError::Decryption)
        );
    }
}
