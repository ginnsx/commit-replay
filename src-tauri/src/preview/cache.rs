use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Mutex,
};

use crate::model::FileChange;

use super::MappingInput;

pub struct PreviewCache(Mutex<Option<CachedPreview>>);

struct CachedPreview {
    key: u64,
    aggregated: Vec<FileChange>,
}

pub fn preview_cache_key(
    source_id: &str,
    target_id: &str,
    source_refs: &[String],
    mappings: &[MappingInput],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    source_id.hash(&mut hasher);
    target_id.hash(&mut hasher);
    for r in source_refs {
        r.hash(&mut hasher);
    }
    for m in mappings {
        m.from.hash(&mut hasher);
        m.to.hash(&mut hasher);
    }
    hasher.finish()
}

impl PreviewCache {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn get(&self, key: u64) -> Option<Vec<FileChange>> {
        let guard = self.0.lock().ok()?;
        guard
            .as_ref()
            .filter(|entry| entry.key == key)
            .map(|entry| entry.aggregated.clone())
    }

    pub fn set(&self, key: u64, aggregated: Vec<FileChange>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(CachedPreview { key, aggregated });
        }
    }
}
