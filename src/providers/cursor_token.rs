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

static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

static CACHE: std::sync::Mutex<Option<TokenCache>> = std::sync::Mutex::new(None);
#[cfg(target_os = "macos")]
type CliProbe = (std::time::Instant, Option<Zeroizing<String>>);
#[cfg(target_os = "macos")]
static CLI: std::sync::Mutex<Option<CliProbe>> = std::sync::Mutex::new(None);

pub(super) fn invalidate(rejected: bool) {
    if let Ok(mut cache) = CACHE.lock()
        && let Some(cache) = cache.as_mut()
    {
        cache.invalidate(rejected);
        #[cfg(all(target_os = "macos", not(test)))]
        if keychain::Store.delete().is_err() {
            crate::diagnostics::warn(
                "cursor.token_cache.delete_failed",
                "Keychain cache removal failed",
            );
        }
    }
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, std::sync::atomic::Ordering::Release);
    if !enabled {
        clear();
        #[cfg(target_os = "macos")]
        if let Ok(mut cli) = CLI.lock() {
            *cli = None;
        }
    }
}

pub(super) fn clear() {
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
        #[cfg(all(target_os = "macos", not(test)))]
        if keychain::Store.delete().is_err() {
            crate::diagnostics::warn(
                "cursor.token_cache.delete_failed",
                "Keychain cache removal failed",
            );
        }
    }
}

pub(super) fn decode(token: String) -> Result<String, String> {
    let mut cache = CACHE.lock().map_err(|_| ERROR.to_string())?;
    if !ENABLED.load(std::sync::atomic::Ordering::Acquire) {
        return Err(ERROR.into());
    }
    cache
        .get_or_insert_with(TokenCache::default)
        .decode_with(token, |source| {
            #[cfg(target_os = "macos")]
            {
                persisted_token(source, &mut keychain::Store, decode_uncached)
            }
            #[cfg(not(target_os = "macos"))]
            {
                decode_uncached(source)
            }
        })
}

#[cfg(target_os = "macos")]
trait TokenStore {
    fn read(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, String>;
    fn write(&mut self, data: &[u8]) -> Result<(), String>;
    fn delete(&mut self) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
fn source_hash(source: &str) -> [u8; 32] {
    let mut hash = [0; 32];
    // Source tokens are bounded SQLite values, well below u32::MAX.
    unsafe {
        CC_SHA256(source.as_ptr(), source.len() as u32, hash.as_mut_ptr());
    }
    hash
}

#[cfg(target_os = "macos")]
fn persisted_token(
    source: &str,
    store: &mut impl TokenStore,
    import: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    let hash = source_hash(source);
    if let Some(record) = store.read()? {
        if record.starts_with(&hash)
            && record.len() > hash.len()
            && let Ok(token) = std::str::from_utf8(&record[hash.len()..])
        {
            crate::diagnostics::info("cursor.token_cache.hit", "source fingerprint matched");
            return Ok(token.to_owned());
        }
        store.delete()?;
    }
    let token = Zeroizing::new(import(source)?);
    let mut record = Zeroizing::new(hash.to_vec());
    record.extend_from_slice(token.as_bytes());
    store.write(&record)?;
    crate::diagnostics::info(
        "cursor.token_cache.imported",
        "saved to Metra Keychain item",
    );
    Ok(token.to_string())
}

#[cfg(target_os = "macos")]
mod keychain {
    use super::*;
    use std::{ffi::c_void, ptr};
    const SERVICE: &[u8] = b"dev.metra.desktop.cursor-token";
    const ACCOUNT: &[u8] = b"cursor";
    const NOT_FOUND: i32 = -25300;
    pub(super) struct Store;
    impl TokenStore for Store {
        fn read(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
            let mut length = 0;
            let mut data: *mut c_void = ptr::null_mut();
            let status = unsafe {
                SecKeychainFindGenericPassword(
                    ptr::null(),
                    SERVICE.len() as u32,
                    SERVICE.as_ptr(),
                    ACCOUNT.len() as u32,
                    ACCOUNT.as_ptr(),
                    &mut length,
                    &mut data,
                    ptr::null_mut(),
                )
            };
            if status == NOT_FOUND {
                return Ok(None);
            }
            if status != 0 || data.is_null() {
                return Err(ERROR.into());
            }
            let result = unsafe {
                let bytes = Zeroizing::new(
                    std::slice::from_raw_parts(data.cast::<u8>(), length as usize).to_vec(),
                );
                SecKeychainItemFreeContent(ptr::null_mut(), data);
                bytes
            };
            Ok(Some(result))
        }
        fn write(&mut self, data: &[u8]) -> Result<(), String> {
            // A missing item is created for the calling signed Metra application.
            let status = unsafe {
                SecKeychainAddGenericPassword(
                    ptr::null(),
                    SERVICE.len() as u32,
                    SERVICE.as_ptr(),
                    ACCOUNT.len() as u32,
                    ACCOUNT.as_ptr(),
                    data.len() as u32,
                    data.as_ptr().cast(),
                    ptr::null_mut(),
                )
            };
            if status == 0 {
                Ok(())
            } else {
                Err(ERROR.into())
            }
        }
        fn delete(&mut self) -> Result<(), String> {
            let mut item: *mut c_void = ptr::null_mut();
            let status = unsafe {
                SecKeychainFindGenericPassword(
                    ptr::null(),
                    SERVICE.len() as u32,
                    SERVICE.as_ptr(),
                    ACCOUNT.len() as u32,
                    ACCOUNT.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut item,
                )
            };
            if status == NOT_FOUND {
                return Ok(());
            }
            if status != 0 || item.is_null() {
                return Err(ERROR.into());
            }
            let status = unsafe {
                let status = SecKeychainItemDelete(item);
                CFRelease(item);
                status
            };
            if status == 0 {
                Ok(())
            } else {
                Err(ERROR.into())
            }
        }
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecKeychainAddGenericPassword(
            keychain: *const c_void,
            service_len: u32,
            service: *const u8,
            account_len: u32,
            account: *const u8,
            password_len: u32,
            password: *const c_void,
            item: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainItemDelete(item: *mut c_void) -> i32;
    }
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

/// Read CLI credentials independently of the desktop token encryption format.
pub(super) fn cli_token() -> Option<Zeroizing<String>> {
    #[cfg(target_os = "macos")]
    {
        // Cache refusals too: background refreshes must not repeatedly prompt.
        let mut probe = CLI.lock().ok()?;
        if let Some((at, token)) = probe.as_ref() {
            let ttl = if token.is_some() { 10 } else { 600 };
            if at.elapsed().as_secs() < ttl {
                return token
                    .clone()
                    .or_else(|| cli_auth_file().and_then(|path| cli_token_from_file(&path)));
            }
        }
        let token = keychain_secret(b"cursor-access-token", b"cursor-user")
            .ok()
            .and_then(|bytes| {
                std::str::from_utf8(&bytes)
                    .ok()
                    .map(|v| Zeroizing::new(v.trim().to_owned()))
            })
            .filter(|v| !v.is_empty());
        *probe = Some((std::time::Instant::now(), token.clone()));
        if token.is_some() {
            return token;
        }
    }
    cli_auth_file().and_then(|path| cli_token_from_file(&path))
}

fn cli_auth_file() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(dirs::home_dir()?.join(".cursor/auth.json"))
    }
    #[cfg(windows)]
    {
        Some(std::path::PathBuf::from(std::env::var_os("APPDATA")?).join("Cursor/auth.json"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        Some(dirs::config_dir()?.join("cursor/auth.json"))
    }
}

fn cli_token_from_file(path: &std::path::Path) -> Option<Zeroizing<String>> {
    // Deserialize into a zeroized string rather than retaining a JSON Value
    // containing access/refresh tokens after parsing.
    #[derive(serde::Deserialize)]
    struct Auth {
        #[serde(rename = "accessToken")]
        access_token: String,
    }
    let bytes = Zeroizing::new(std::fs::read(path).ok()?);
    let auth: Auth = serde_json::from_slice(&bytes).ok()?;
    let token = Zeroizing::new(auth.access_token);
    (!token.trim().is_empty()).then_some(token)
}

pub(super) fn token_expired(token: &str) -> bool {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let Some(payload) = token.split('.').nth(1) else {
        return false;
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')) else {
        return false;
    };
    let bytes = Zeroizing::new(bytes);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    value
        .get("exp")
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|exp| exp <= chrono::Utc::now().timestamp())
}

#[cfg(target_os = "macos")]
fn keychain_password() -> Result<Zeroizing<Vec<u8>>, String> {
    keychain_secret(b"Cursor Safe Storage", b"")
}

#[cfg(target_os = "macos")]
fn keychain_secret(service: &[u8], account: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    use std::ptr;
    let mut length = 0_u32;
    let mut data: *mut std::ffi::c_void = ptr::null_mut();
    // Query only Cursor's existing generic password. macOS handles user consent
    // for the signed Metra process; never delegate this access to a shell tool.
    let status = unsafe {
        SecKeychainFindGenericPassword(
            ptr::null(),
            service.len() as u32,
            service.as_ptr(),
            account.len() as u32,
            if account.is_empty() {
                ptr::null()
            } else {
                account.as_ptr()
            },
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
    fn CC_SHA256(data: *const u8, length: u32, digest: *mut u8) -> *mut u8;
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

    #[cfg(target_os = "macos")]
    #[test]
    fn persistent_cache_survives_memory_reset_and_rejects_changed_sources() {
        #[derive(Default)]
        struct Store {
            record: Option<Zeroizing<Vec<u8>>>,
            deletes: usize,
        }
        impl TokenStore for Store {
            fn read(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
                Ok(self.record.clone())
            }
            fn write(&mut self, data: &[u8]) -> Result<(), String> {
                self.record = Some(Zeroizing::new(data.to_vec()));
                Ok(())
            }
            fn delete(&mut self) -> Result<(), String> {
                self.record = None;
                self.deletes += 1;
                Ok(())
            }
        }
        let mut store = Store::default();
        let mut memory = TokenCache::default();
        let first = memory
            .decode_with("encrypted-a".into(), |source| {
                persisted_token(source, &mut store, |_| Ok("token-a".into()))
            })
            .unwrap();
        assert_eq!(first, "token-a");
        drop(memory);
        let mut restarted = TokenCache::default();
        assert_eq!(
            restarted
                .decode_with("encrypted-a".into(), |source| persisted_token(
                    source,
                    &mut store,
                    |_| panic!("restart must use Metra's item")
                ))
                .unwrap(),
            "token-a"
        );
        assert_eq!(
            restarted
                .decode_with("encrypted-b".into(), |source| persisted_token(
                    source,
                    &mut store,
                    |_| Ok("token-b".into())
                ))
                .unwrap(),
            "token-b"
        );
        assert_eq!(store.deletes, 1);
        assert!(
            !store
                .record
                .as_ref()
                .unwrap()
                .windows(7)
                .any(|v| v == b"token-a")
        );
        store.delete().unwrap();
        restarted.invalidate(false);
        assert_eq!(
            restarted
                .decode_with("encrypted-b".into(), |source| persisted_token(
                    source,
                    &mut store,
                    |_| Ok("fresh".into())
                ))
                .unwrap(),
            "fresh"
        );
        store.record = Some(Zeroizing::new(b"corrupt".to_vec()));
        assert!(persisted_token("encrypted-b", &mut store, |_| Err("denied".into())).is_err());
        assert!(store.record.is_none());
    }

    #[test]
    fn cli_file_reads_exact_access_token_and_checks_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(
            &path,
            r#"{"accessToken":"cli-token","refreshToken":"never-use"}"#,
        )
        .unwrap();
        assert_eq!(cli_token_from_file(&path).unwrap().as_str(), "cli-token");
        std::fs::write(&path, r#"{"refreshToken":"never-use"}"#).unwrap();
        assert!(cli_token_from_file(&path).is_none());
        std::fs::write(&path, "invalid").unwrap();
        assert!(cli_token_from_file(&path).is_none());
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload = URL_SAFE_NO_PAD.encode(br#"{"exp":1}"#);
        assert!(token_expired(&format!("header.{payload}.signature")));
        assert!(!token_expired("opaque-token"));
    }

    #[test]
    fn plaintext_tokens_are_unchanged() {
        assert_eq!(
            decode_uncached("header.payload.signature").unwrap(),
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
