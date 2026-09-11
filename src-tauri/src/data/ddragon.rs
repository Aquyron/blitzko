use std::time::Duration;

use serde_json::Value;

use super::cache::Cache;

const DDRAGON_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const BASE: &str = "https://ddragon.leagueoflegends.com";

/// Static reference data (champions, items, summoner spells, runes) from
/// Riot's own Data Dragon CDN — official, unrestricted, no ToS ambiguity.
/// Kept separate from `OpggDataService`: OP.GG supplies stats/build
/// recommendations, Data Dragon supplies the id/name/icon metadata to
/// render them, per the "don't mix data layers" rule.
pub struct DdragonService {
    http: reqwest::Client,
    cache: Cache,
}

impl DdragonService {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("failed to build Data Dragon http client"),
            cache: Cache::new(),
        }
    }

    pub async fn latest_version(&self) -> Result<String, reqwest::Error> {
        const KEY: &str = "ddragon:versions";
        if let Some(cached) = self.cache.get(KEY).await {
            if let Some(v) = cached.as_str() {
                return Ok(v.to_string());
            }
        }
        let versions: Vec<String> = self
            .http
            .get(format!("{BASE}/api/versions.json"))
            .send()
            .await?
            .json()
            .await?;
        let latest = versions.into_iter().next().unwrap_or_default();
        self.cache
            .set(KEY.to_string(), Value::String(latest.clone()), DDRAGON_TTL)
            .await;
        Ok(latest)
    }

    async fn fetch_cached(&self, key: &str, url: String) -> Result<Value, reqwest::Error> {
        if let Some(cached) = self.cache.get(key).await {
            return Ok(cached);
        }
        let value: Value = self.http.get(url).send().await?.json().await?;
        self.cache.set(key.to_string(), value.clone(), DDRAGON_TTL).await;
        Ok(value)
    }

    pub async fn champions(&self) -> Result<Value, reqwest::Error> {
        let version = self.latest_version().await?;
        let key = format!("ddragon:champions:{version}");
        let url = format!("{BASE}/cdn/{version}/data/en_US/champion.json");
        self.fetch_cached(&key, url).await
    }

    pub async fn items(&self) -> Result<Value, reqwest::Error> {
        let version = self.latest_version().await?;
        let key = format!("ddragon:items:{version}");
        let url = format!("{BASE}/cdn/{version}/data/en_US/item.json");
        self.fetch_cached(&key, url).await
    }

    pub async fn summoner_spells(&self) -> Result<Value, reqwest::Error> {
        let version = self.latest_version().await?;
        let key = format!("ddragon:summoner:{version}");
        let url = format!("{BASE}/cdn/{version}/data/en_US/summoner.json");
        self.fetch_cached(&key, url).await
    }

    pub async fn runes(&self) -> Result<Value, reqwest::Error> {
        let version = self.latest_version().await?;
        let key = format!("ddragon:runes:{version}");
        let url = format!("{BASE}/cdn/{version}/data/en_US/runesReforged.json");
        self.fetch_cached(&key, url).await
    }
}

impl Default for DdragonService {
    fn default() -> Self {
        Self::new()
    }
}
