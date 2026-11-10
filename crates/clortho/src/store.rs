//! Key-store integration. Secrets live in the OS store; clortho keeps only
//! an index of which key names have one, so that checking never touches (and
//! on macOS never prompts for) the keychain.

use crate::config::{Config, StoreBackend};
use clortho_core::Secret;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

pub trait Store: Send {
    fn has(&self, key: &str) -> bool;
    fn get(&mut self, key: &str) -> Result<Option<Secret>, String>;
    fn set(&mut self, key: &str, secret: &Secret) -> Result<(), String>;
    fn delete(&mut self, key: &str);
}

pub fn open(cfg: &Config) -> Box<dyn Store> {
    match cfg.store_backend {
        StoreBackend::Memory => Box::new(MemoryStore::default()),
        StoreBackend::Os => Box::new(OsStore::open()),
    }
}

#[derive(Default)]
pub struct MemoryStore(HashMap<String, Secret>);

impl Store for MemoryStore {
    fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    fn get(&mut self, key: &str) -> Result<Option<Secret>, String> {
        Ok(self.0.get(key).cloned())
    }
    fn set(&mut self, key: &str, secret: &Secret) -> Result<(), String> {
        self.0.insert(key.to_string(), secret.clone());
        Ok(())
    }
    fn delete(&mut self, key: &str) {
        self.0.remove(key);
    }
}

const SERVICE: &str = "clortho";

pub struct OsStore {
    index: BTreeSet<String>,
    index_path: PathBuf,
}

impl OsStore {
    pub fn open() -> Self {
        let index_path = crate::config::state_dir().join("stored-keys.json");
        let index = std::fs::read(&index_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        OsStore { index, index_path }
    }

    fn save_index(&self) {
        if let Some(dir) = self.index_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(&self.index) {
            let _ = std::fs::write(&self.index_path, json);
        }
    }
}

impl Store for OsStore {
    fn has(&self, key: &str) -> bool {
        self.index.contains(key)
    }

    fn get(&mut self, key: &str) -> Result<Option<Secret>, String> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(p) => Ok(Some(Secret::new(p))),
            Err(keyring::Error::NoEntry) => {
                // Removed behind our back (e.g. in Keychain Access).
                if self.index.remove(key) {
                    self.save_index();
                }
                Ok(None)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&mut self, key: &str, secret: &Secret) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(|e| e.to_string())?;
        entry
            .set_password(secret.expose())
            .map_err(|e| e.to_string())?;
        if self.index.insert(key.to_string()) {
            self.save_index();
        }
        Ok(())
    }

    fn delete(&mut self, key: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, key) {
            let _ = entry.delete_credential();
        }
        if self.index.remove(key) {
            self.save_index();
        }
    }
}
