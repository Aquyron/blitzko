use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;

/// Small in-memory TTL cache shared by the data services. Keeps the app
/// from re-fetching the same champion/build/matchup data on every
/// champ-select re-render — only a cache miss or expiry triggers a network
/// call.
pub struct Cache {
    entries: Mutex<HashMap<String, (Value, Instant)>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Value> {
        let map = self.entries.lock().await;
        map.get(key).and_then(|(value, expires_at)| {
            if Instant::now() < *expires_at {
                Some(value.clone())
            } else {
                None
            }
        })
    }

    pub async fn set(&self, key: String, value: Value, ttl: Duration) {
        let mut map = self.entries.lock().await;
        map.insert(key, (value, Instant::now() + ttl));
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}
