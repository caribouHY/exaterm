use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zeroize::Zeroizing;

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct PasswordCacheKey {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub host_key: String,
}

#[derive(Clone, Default)]
pub(super) struct PasswordCache(Arc<Mutex<CacheState>>);

#[derive(Default)]
struct CacheState {
    generation: u64,
    entries: HashMap<PasswordCacheKey, CacheEntry>,
}

#[derive(Default)]
struct CacheEntry {
    revision: u64,
    password: Option<Zeroizing<String>>,
}

pub(super) struct PasswordLease {
    key: PasswordCacheKey,
    generation: u64,
    revision: u64,
    pub password: Option<Zeroizing<String>>,
}

impl PasswordCache {
    pub fn generation(&self) -> u64 {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).generation
    }

    pub fn clear(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.generation += 1;
        state.entries.clear();
    }

    pub fn checkout(&self, key: PasswordCacheKey, generation: u64) -> PasswordLease {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (revision, password) = if state.generation == generation {
            let entry = state.entries.entry(key.clone()).or_default();
            (entry.revision, entry.password.clone())
        } else {
            (0, None)
        };
        PasswordLease {
            key,
            generation,
            revision,
            password,
        }
    }

    pub fn remember(&self, lease: &PasswordLease, password: Zeroizing<String>) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation != lease.generation {
            return;
        }
        if let Some(entry) = state.entries.get_mut(&lease.key) {
            if entry.revision == lease.revision {
                entry.password = Some(password);
                entry.revision += 1;
            }
        }
    }

    pub fn reject(&self, lease: &mut PasswordLease) {
        lease.password = None;
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation != lease.generation {
            return;
        }
        if let Some(entry) = state.entries.get_mut(&lease.key) {
            if entry.revision == lease.revision {
                entry.password = None;
                entry.revision += 1;
                lease.revision = entry.revision;
            }
        }
    }
}
