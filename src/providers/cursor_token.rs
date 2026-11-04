//! Decode Cursor's opt-in encrypted token storage on macOS.
use zeroize::Zeroizing;

const PREFIX: &str = "cursor-safe-storage:v1:";
const ERROR: &str = "无法解密 Cursor 本地登录令牌";

#[derive(Default)]
struct TokenCache {
    source: Zeroizing<String>,
    plaintext: Option<Zeroizing<String>>,
    rejected: bool,
}

impl TokenCache {
    fn decode_with(
        &mut self,
        source: String,
        read: impl FnOnce(&str) -> Result<String, String>,
    ) -> Result<String, String> {
        let source = Zeroizing::new(source);
        if self.source.as_str() != source.as_str() {
            *self = Self::default();
            self.source = source;
        }
        if self.rejected {
            return Err("Cursor 登录已失效".into());
        }
        if let Some(token) = &self.plaintext {
            return Ok(token.to_string());
        }
        let token = Zeroizing::new(read(&self.source)?);
        let result = token.to_string();
        self.plaintext = Some(token);
        Ok(result)
    }

    fn invalidate(&mut self, rejected: bool) {
        self.plaintext = None;
        self.rejected = rejected;
    }
}

static CACHE: std::sync::Mutex<Option<TokenCache>> = std::sync::Mutex::new(None);

pub(super) fn invalidate(rejected: bool) {
    if let Ok(mut cache) = CACHE.lock()
        && let Some(cache) = cache.as_mut()
    {
        cache.invalidate(rejected);
    }
}

pub(super) fn decode(token: String) -> Result<String, String> {
    CACHE
        .lock()
        .map_err(|_| ERROR.to_string())?
        .get_or_insert_with(TokenCache::default)
        .decode_with(token, decode_uncached)
}

fn decode_uncached(token: &str) -> Result<String, String> {
    if !token.starts_with(PREFIX) {
        return Ok(token.to_owned());
    }
    #[cfg(target_os = "macos")]
    {
        let password = keychain_password()?;
        decrypt(token, &password)
    }
    #[cfg(not(target_os = "macos"))]
    Err(ERROR.into())
}

#[cfg(target_os = "macos")]
fn keychain_password() -> Result<Zeroizing<Vec<u8>>, String> {
    use std::ptr;
    let service = b"Cursor Safe Storage";
    let mut length = 0_u32;
    let mut data: *mut std::ffi::c_void = ptr::null_mut();
    // Query only Cursor's existing generic password. macOS handles user consent
    // for the signed Metra process; never delegate this access to a shell tool.
    let status = unsafe {
        SecKeychainFindGenericPassword(
            ptr::null(),
            service.len() as u32,
            service.as_ptr(),
            0,
            ptr::null(),
            &mut length,
            &mut data,
            ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(ERROR.into());
    }
    if data.is_null() {
        return Err(ERROR.into());
    }
    let password = unsafe {
        let password =
            Zeroizing::new(std::slice::from_raw_parts(data.cast::<u8>(), length as usize).to_vec());
        SecKeychainItemFreeContent(ptr::null_mut(), data);
        password
    };
    Ok(password)
}

#[cfg(target_os = "macos")]
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainFindGenericPassword(
        keychain: *const std::ffi::c_void,
        service_len: u32,
        service: *const u8,
        account_len: u32,
        account: *const u8,
        password_len: *mut u32,
        password: *mut *mut std::ffi::c_void,
        item: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn SecKeychainItemFreeContent(
        attributes: *mut std::ffi::c_void,
        data: *mut std::ffi::c_void,
    ) -> i32;
}

#[cfg(target_os = "macos")]
fn decrypt(token: &str, password: &[u8]) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let encrypted = token.strip_prefix(PREFIX).ok_or(ERROR)?;
    let bytes = STANDARD.decode(encrypted).map_err(|_| ERROR)?;
    let ciphertext = bytes.strip_prefix(b"v10").ok_or(ERROR)?;
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(ERROR.into());
    }
    let mut key = Zeroizing::new([0_u8; 16]);
    let mut plaintext = Zeroizing::new(vec![0_u8; ciphertext.len()]);
    let mut written = 0;
    // Chromium's macOS OSCrypt format: PBKDF2-SHA1, AES-128-CBC, PKCS#7.
    // All buffers remain alive for these synchronous calls and lengths match them.
    unsafe {
        if CCKeyDerivationPBKDF(
            2,
            password.as_ptr(),
            password.len(),
            b"saltysalt".as_ptr(),
            9,
            1,
            1003,
            key.as_mut_ptr(),
            key.len(),
        ) != 0
        {
            return Err(ERROR.into());
        }
        if CCCrypt(
            1,
            0,
            1,
            key.as_ptr(),
            key.len(),
            [b' '; 16].as_ptr(),
            ciphertext.as_ptr(),
            ciphertext.len(),
            plaintext.as_mut_ptr(),
            plaintext.len(),
            &mut written,
        ) != 0
        {
            return Err(ERROR.into());
        }
    }
    let decoded = std::str::from_utf8(&plaintext[..written]).map_err(|_| ERROR)?;
    if decoded.is_empty() {
        return Err(ERROR.into());
    }
    Ok(decoded.to_owned())
}

#[cfg(target_os = "macos")]
#[link(name = "System")]
unsafe extern "C" {
    fn CCKeyDerivationPBKDF(
        algorithm: u32,
        password: *const u8,
        password_len: usize,
        salt: *const u8,
        salt_len: usize,
        prf: u32,
        rounds: u32,
        key: *mut u8,
        key_len: usize,
    ) -> i32;
    fn CCCrypt(
        operation: u32,
        algorithm: u32,
        options: u32,
        key: *const u8,
        key_len: usize,
        iv: *const u8,
        input: *const u8,
        input_len: usize,
        output: *mut u8,
        output_len: usize,
        written: *mut usize,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_reuses_rotates_and_invalidates_without_repeated_prompts() {
        let mut cache = TokenCache::default();
        assert_eq!(
            cache
                .decode_with("source-a".into(), |_| Ok("token-a".into()))
                .unwrap(),
            "token-a"
        );
        assert_eq!(
            cache
                .decode_with("source-a".into(), |_| panic!(
                    "cache hit must not access Keychain"
                ))
                .unwrap(),
            "token-a"
        );
        assert_eq!(
            cache
                .decode_with("source-b".into(), |_| Ok("token-b".into()))
                .unwrap(),
            "token-b"
        );
        cache.invalidate(false);
        assert_eq!(
            cache
                .decode_with("source-b".into(), |_| Ok("token-b-new".into()))
                .unwrap(),
            "token-b-new"
        );
        cache.invalidate(true);
        assert!(
            cache
                .decode_with("source-b".into(), |_| panic!(
                    "rejected source must not prompt again"
                ))
                .is_err()
        );
        assert_eq!(
            cache
                .decode_with("source-c".into(), |_| Ok("token-c".into()))
                .unwrap(),
            "token-c"
        );
        cache.invalidate(false);
        assert!(
            cache
                .decode_with("source-c".into(), |_| Err("denied".into()))
                .is_err()
        );
        assert_eq!(
            cache
                .decode_with("source-c".into(), |_| Ok("retry".into()))
                .unwrap(),
            "retry"
        );
    }

    #[test]
    fn plaintext_tokens_are_unchanged() {
        assert_eq!(
            decode("header.payload.signature".into()).unwrap(),
            "header.payload.signature"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn chromium_encrypted_token_is_decrypted() {
        // Fixed synthetic Chromium fixture; contains no real account data.
        let token = concat!(
            "cursor-safe-storage:v1:",
            "djEwaTKn8QSpJ4aIH7pIsFHxMWdeVpGH4Q7X7hMfDsr+cpE="
        );
        assert_eq!(
            decrypt(token, b"test-password").unwrap(),
            "header.payload.signature"
        );
        assert!(decrypt(token, b"wrong-password").is_err());
        assert!(decrypt("cursor-safe-storage:v1:invalid", b"test-password").is_err());
    }
}
