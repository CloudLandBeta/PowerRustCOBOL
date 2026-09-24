// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The application's key store (spec 076 §4.2).
//!
//! A program stores a key for a model-list entry, replaces it, removes it, or
//! asks whether one is set — and can **never read one back** (R6): no CALL
//! reaches [`KeyStore::get`], which only the runtime uses to authenticate a
//! request.
//!
//! [`KeyStore`] is the seam (R8). The first store, [`FileKeyStore`], keeps the
//! keys in one file in the installation folder, shared by every user of the
//! installation (R9), encrypted with a key derived from that installation
//! (R10). The operating system's keychain can replace it through
//! [`set_key_store`] without a program, form or list changing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Where keys are kept.
pub trait KeyStore: Send + Sync {
    fn set(&self, name: &str, key: &str) -> Result<(), String>;
    fn remove(&self, name: &str) -> Result<(), String>;
    fn is_set(&self, name: &str) -> bool;
    /// The key itself — for the runtime's own requests only.
    fn get(&self, name: &str) -> Option<String>;
}

fn norm(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// Keys in memory, gone when the process ends — for tests (AC5).
#[derive(Default)]
pub struct MemoryKeyStore {
    keys: Mutex<BTreeMap<String, String>>,
}

impl KeyStore for MemoryKeyStore {
    fn set(&self, name: &str, key: &str) -> Result<(), String> {
        self.keys.lock().unwrap().insert(norm(name), key.to_string());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        self.keys.lock().unwrap().remove(&norm(name));
        Ok(())
    }
    fn is_set(&self, name: &str) -> bool {
        self.keys.lock().unwrap().contains_key(&norm(name))
    }
    fn get(&self, name: &str) -> Option<String> {
        self.keys.lock().unwrap().get(&norm(name)).cloned()
    }
}

/// The file's first bytes.
const MAGIC: &[u8; 8] = b"PRCKEYS1";

/// Keys in one encrypted file (R9, R10, R11).
///
/// The cipher key is derived from the canonical installation folder and the
/// machine's host name, so the file is unreadable at a glance and decrypts
/// nothing once copied to another folder or machine. It is **not** a secret
/// the user holds: anyone who can run the application on this machine can use
/// the keys — the guide says so, and names the keychain as the way that closes.
pub struct FileKeyStore {
    path: PathBuf,
    cipher_key: [u8; 32],
    keys: Mutex<BTreeMap<String, String>>,
    /// Why the file could not be read. While set, nothing is written, so the
    /// unreadable file is never overwritten (R11).
    problem: Option<String>,
}

impl FileKeyStore {
    /// The store for the installation at `install_dir`.
    pub fn open(install_dir: &Path) -> FileKeyStore {
        let path = install_dir.join("settings").join("model-keys.dat");
        let cipher_key = derive_key(install_dir);
        let mut store = FileKeyStore {
            path,
            cipher_key,
            keys: Mutex::new(BTreeMap::new()),
            problem: None,
        };
        match std::fs::read(&store.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => store.problem = Some(format!("the key file cannot be read: {e}")),
            Ok(bytes) => match decrypt(&store.cipher_key, &bytes) {
                Some(keys) => *store.keys.lock().unwrap() = keys,
                None => {
                    store.problem = Some(format!(
                        "the key file {} cannot be decrypted — it is damaged, or was \
                         copied from another installation or machine; move it aside \
                         and enter the keys again",
                        store.path.display()
                    ))
                }
            },
        }
        if let Some(p) = &store.problem {
            tracing::warn!(target: "keys", "{p}");
        }
        store
    }

    /// Why the file could not be used, if it could not.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    fn save(&self, keys: &BTreeMap<String, String>) -> Result<(), String> {
        if let Some(p) = &self.problem {
            return Err(p.clone());
        }
        let bytes = encrypt(&self.cipher_key, keys)?;
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("the key folder {} cannot be created: {e}", dir.display()))?;
        let tmp = self.path.with_extension("dat.tmp");
        std::fs::write(&tmp, bytes)
            .and_then(|_| std::fs::rename(&tmp, &self.path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("the key file {} cannot be written: {e}", self.path.display())
            })
    }
}

impl KeyStore for FileKeyStore {
    fn set(&self, name: &str, key: &str) -> Result<(), String> {
        let mut keys = self.keys.lock().unwrap();
        let mut next = keys.clone();
        next.insert(norm(name), key.to_string());
        self.save(&next)?;
        *keys = next;
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        let mut keys = self.keys.lock().unwrap();
        if !keys.contains_key(&norm(name)) {
            return Ok(());
        }
        let mut next = keys.clone();
        next.remove(&norm(name));
        self.save(&next)?;
        *keys = next;
        Ok(())
    }
    fn is_set(&self, name: &str) -> bool {
        self.keys.lock().unwrap().contains_key(&norm(name))
    }
    fn get(&self, name: &str) -> Option<String> {
        self.keys.lock().unwrap().get(&norm(name)).cloned()
    }
}

fn derive_key(install_dir: &Path) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let folder = std::fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());
    let host = sysinfo::System::host_name().unwrap_or_default();
    let mut h = Sha256::new();
    h.update(b"PowerRustCOBOL application key store v1\0");
    h.update(folder.to_string_lossy().as_bytes());
    h.update(b"\0");
    h.update(host.as_bytes());
    h.finalize().into()
}

fn encrypt(cipher_key: &[u8; 32], keys: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    let plain = serde_json::to_vec(keys).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| format!("no randomness for the key file: {e}"))?;
    let cipher = aes_gcm::Aes256Gcm::new(cipher_key.into());
    let sealed = cipher
        .encrypt(&nonce.into(), plain.as_slice())
        .map_err(|_| "the keys could not be encrypted".to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn decrypt(cipher_key: &[u8; 32], bytes: &[u8]) -> Option<BTreeMap<String, String>> {
    use aes_gcm::aead::{Aead, KeyInit};
    let rest = bytes.strip_prefix(MAGIC.as_slice())?;
    if rest.len() < 12 {
        return None;
    }
    let (nonce, sealed) = rest.split_at(12);
    let nonce: [u8; 12] = nonce.try_into().ok()?;
    let cipher = aes_gcm::Aes256Gcm::new(cipher_key.into());
    let plain = cipher.decrypt(&nonce.into(), sealed).ok()?;
    serde_json::from_slice(&plain).ok()
}

// ── The store in use ─────────────────────────────────────────────────────────

fn slot() -> &'static RwLock<Option<Arc<dyn KeyStore>>> {
    static SLOT: std::sync::OnceLock<RwLock<Option<Arc<dyn KeyStore>>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

/// Replace the store every form in the process uses — a test's
/// [`MemoryKeyStore`], or, later, the operating system's keychain (R8).
pub fn set_key_store(store: Arc<dyn KeyStore>) {
    *slot().write().unwrap_or_else(|p| p.into_inner()) = Some(store);
}

/// The store in use: the one set with [`set_key_store`], else the
/// installation's [`FileKeyStore`], opened on first use.
pub fn key_store() -> Arc<dyn KeyStore> {
    if let Some(s) = slot().read().unwrap_or_else(|p| p.into_inner()).clone() {
        return s;
    }
    let mut w = slot().write().unwrap_or_else(|p| p.into_inner());
    w.get_or_insert_with(|| {
        let base = cobolt_forms::assets::current_base()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        Arc::new(FileKeyStore::open(&base))
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("prc-076-{tag}-{nanos}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// AC6, AC7 — the file round-trips, holds no key in plain text, decrypts
    /// nothing in another installation, and a damaged one is reported, left
    /// as it was, and never overwritten.
    #[test]
    fn the_file_store() {
        const SECRET: &str = "sk-live-0123456789abcdef";
        let install = temp("install");
        let store = FileKeyStore::open(&install);
        assert!(store.problem().is_none(), "a missing file is an empty store");
        assert!(!store.is_set("company"));
        store.set("company", SECRET).unwrap();
        store.set("backup", "other").unwrap();
        store.remove("backup").unwrap();

        let file = install.join("settings").join("model-keys.dat");
        let bytes = std::fs::read(&file).unwrap();
        assert!(bytes.starts_with(MAGIC));
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(SECRET) && !text.contains("COMPANY"), "nothing readable at a glance");

        // A second user of the same installation: the same folder, the same key.
        let again = FileKeyStore::open(&install);
        assert_eq!(again.get("Company").as_deref(), Some(SECRET));
        assert!(!again.is_set("backup"));

        // The file copied to another installation decrypts nothing.
        let other = temp("other");
        std::fs::create_dir_all(other.join("settings")).unwrap();
        std::fs::copy(&file, other.join("settings").join("model-keys.dat")).unwrap();
        let copied = FileKeyStore::open(&other);
        assert!(copied.problem().is_some());
        assert!(!copied.is_set("company"));
        let before = std::fs::read(other.join("settings").join("model-keys.dat")).unwrap();
        assert!(copied.set("company", "new").is_err(), "an unreadable file is never overwritten");
        assert_eq!(std::fs::read(other.join("settings").join("model-keys.dat")).unwrap(), before);

        // A damaged file: reported, left as it was.
        std::fs::write(&file, b"PRCKEYS1 garbage").unwrap();
        let damaged = FileKeyStore::open(&install);
        assert!(damaged.problem().unwrap().contains("cannot be decrypted"));
        assert_eq!(std::fs::read(&file).unwrap(), b"PRCKEYS1 garbage");

        let _ = std::fs::remove_dir_all(&install);
        let _ = std::fs::remove_dir_all(&other);
        println!("key file: {} bytes for 1 key; round-trip OK; copy elsewhere → refused; damaged → reported, untouched", bytes.len());
    }

    #[test]
    fn the_memory_store() {
        let s = MemoryKeyStore::default();
        s.set("a", "k1").unwrap();
        assert!(s.is_set("A"));
        s.set("a", "k2").unwrap();
        assert_eq!(s.get("a").as_deref(), Some("k2"));
        s.remove("a").unwrap();
        assert!(!s.is_set("a"));
    }
}
