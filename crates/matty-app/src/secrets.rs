//! The session token, kept in the system's credential store: the macOS
//! Keychain, the Windows Credential Manager, or the Secret Service on Linux
//! (PLAN §6.6). Nothing under Matty's data folder holds it.
//!
//! The store can be missing, as on a Linux desktop without a Secret
//! Service; then Matty signs in each launch, as before. On the web the
//! token is kept in the page's `localStorage`.
//!
//! Builds with the `bench-session-file` feature (never a release) keep it
//! in the file `MATTY_BENCH_SESSION_FILE` names instead: bench/README.md.

use crate::settings::SavedAccount;

/// The credential store's service name for Matty's items.
const SERVICE: &str = "Matty";

/// One item per server and user.
#[cfg(not(target_family = "wasm"))]
fn entry(account: &SavedAccount) -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, &account.keychain_key())
        .inspect_err(|error| tracing::info!(%error, "no credential store"))
        .ok()
}

/// Keeps `token` for `account`, replacing any older one.
#[cfg(not(target_family = "wasm"))]
pub fn save(account: &SavedAccount, token: &str) {
    #[cfg(feature = "bench-session-file")]
    if let Some(file) = bench_file::path() {
        return bench_file::set(&file, &account.keychain_key(), Some(token));
    }
    if let Some(entry) = entry(account)
        && let Err(error) = entry.set_password(token)
    {
        tracing::warn!(%error, "could not save the session");
    }
}

/// The token saved for `account`, if there is one.
#[cfg(not(target_family = "wasm"))]
pub fn load(account: &SavedAccount) -> Option<String> {
    #[cfg(feature = "bench-session-file")]
    if let Some(file) = bench_file::path() {
        return bench_file::read(&file)
            .remove(&account.keychain_key())
            .filter(|token| !token.is_empty());
    }
    match entry(account)?.get_password() {
        Ok(token) if !token.is_empty() => Some(token),
        Ok(_) | Err(keyring::Error::NoEntry) => None,
        Err(error) => {
            tracing::warn!(%error, "could not read the saved session");
            None
        }
    }
}

/// Forgets `account`'s token, on sign-out or when it expired.
#[cfg(not(target_family = "wasm"))]
pub fn delete(account: &SavedAccount) {
    #[cfg(feature = "bench-session-file")]
    if let Some(file) = bench_file::path() {
        return bench_file::set(&file, &account.keychain_key(), None);
    }
    match entry(account).map(|entry| entry.delete_credential()) {
        None | Some(Ok(()) | Err(keyring::Error::NoEntry)) => {}
        Some(Err(error)) => tracing::warn!(%error, "could not remove the saved session"),
    }
}

/// The benchmark's stand-in for the credential store. A build made on the
/// spot is signed ad hoc, so each one is a stranger to the keychain item the
/// last one wrote, and macOS stops the run to ask for a password.
#[cfg(all(feature = "bench-session-file", not(target_family = "wasm")))]
mod bench_file {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    pub fn path() -> Option<PathBuf> {
        std::env::var_os("MATTY_BENCH_SESSION_FILE")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }

    pub fn read(file: &Path) -> BTreeMap<String, String> {
        std::fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn set(file: &Path, key: &str, token: Option<&str>) {
        let mut tokens = read(file);
        match token {
            Some(token) => tokens.insert(key.to_owned(), token.to_owned()),
            None => tokens.remove(key),
        };
        let written = serde_json::to_vec(&tokens)
            .map_err(std::io::Error::other)
            .and_then(|bytes| std::fs::write(file, bytes));
        if let Err(error) = written {
            tracing::warn!(%error, "could not write the bench session file");
        }
    }
}

/// The `localStorage` key for `account`'s token on the web.
#[cfg(target_family = "wasm")]
fn key(account: &SavedAccount) -> String {
    format!("{SERVICE} session {}", account.keychain_key())
}

/// Keeps `token` for `account`. The browser has no credential store, so on
/// the web it goes in this origin's `localStorage`.
#[cfg(target_family = "wasm")]
pub fn save(account: &SavedAccount, token: &str) {
    if let Err(error) = crate::web_storage::set(&key(account), token) {
        tracing::warn!(%error, "could not save the session");
    }
}

/// The token saved for `account`, if there is one.
#[cfg(target_family = "wasm")]
pub fn load(account: &SavedAccount) -> Option<String> {
    crate::web_storage::get(&key(account))
        .ok()
        .filter(|token| !token.is_empty())
}

/// Forgets `account`'s token, on sign-out or when it expired.
#[cfg(target_family = "wasm")]
pub fn delete(account: &SavedAccount) {
    let _ = crate::web_storage::remove(&key(account));
}

#[cfg(all(test, feature = "bench-session-file", not(target_family = "wasm")))]
mod tests {
    use super::bench_file;

    #[test]
    fn the_bench_file_keeps_and_forgets_a_token() {
        let file = std::env::temp_dir().join(format!("matty-bench-session-{}", std::process::id()));
        bench_file::set(&file, "http://localhost:8075 u1", Some("token"));
        bench_file::set(&file, "http://localhost:8075 u2", Some("other"));
        assert_eq!(
            bench_file::read(&file)
                .get("http://localhost:8075 u1")
                .map(String::as_str),
            Some("token")
        );
        bench_file::set(&file, "http://localhost:8075 u1", None);
        assert_eq!(bench_file::read(&file).len(), 1);
        std::fs::remove_file(&file).ok();
    }
}
