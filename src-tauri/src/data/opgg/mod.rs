mod client;
pub mod text_format;

use std::time::Duration;

use serde_json::Value;

pub use client::OpggError;
use client::OpggClient;

use super::cache::Cache;

const CHAMPION_ANALYSIS_TTL: Duration = Duration::from_secs(20 * 60);
const LANE_META_TTL: Duration = Duration::from_secs(20 * 60);
const ARAM_AUGMENTS_TTL: Duration = Duration::from_secs(20 * 60);
const SUMMONER_PROFILE_TTL: Duration = Duration::from_secs(20 * 60);
const SUMMONER_MATCHES_TTL: Duration = Duration::from_secs(20 * 60);

/// Fields requested from `lol_get_champion_analysis`, matching the "closed
/// set" the tool documents via `tools/list`. Keep this in sync with that
/// list if OP.GG adds/renames fields — an unmatched field is silently
/// skipped by the server (returned in `_field_diagnostics`, which we don't
/// currently surface), not an error.
fn champion_analysis_fields() -> Vec<&'static str> {
    vec![
        "data.summary.average_stats.play",
        "data.summary.average_stats.win_rate",
        "data.summary.average_stats.pick_rate",
        "data.summary.average_stats.ban_rate",
        "data.summary.average_stats.tier_data.tier",
        "data.damage_type",
        "data.starter_items.ids_names",
        "data.starter_items.play",
        "data.starter_items.win",
        "data.starter_items.pick_rate",
        "data.core_items.ids_names",
        "data.core_items.play",
        "data.core_items.win",
        "data.core_items.pick_rate",
        "data.boots.ids_names",
        "data.boots.play",
        "data.boots.win",
        "data.boots.pick_rate",
        "data.fourth_items[].ids_names",
        "data.fourth_items[].play",
        "data.fourth_items[].win",
        "data.fourth_items[].pick_rate",
        "data.fifth_items[].ids_names",
        "data.fifth_items[].play",
        "data.fifth_items[].win",
        "data.fifth_items[].pick_rate",
        "data.sixth_items[].ids_names",
        "data.sixth_items[].play",
        "data.sixth_items[].win",
        "data.sixth_items[].pick_rate",
        "data.runes.primary_page_id",
        "data.runes.primary_page_name",
        "data.runes.primary_rune_ids",
        "data.runes.primary_rune_names",
        "data.runes.secondary_page_id",
        "data.runes.secondary_page_name",
        "data.runes.secondary_rune_ids",
        "data.runes.secondary_rune_names",
        "data.runes.stat_mod_names",
        "data.runes.play",
        "data.runes.win",
        "data.runes.pick_rate",
        "data.summoner_spells.ids",
        "data.summoner_spells.play",
        "data.summoner_spells.win",
        "data.summoner_spells.pick_rate",
        "data.skills.order",
        "data.skills.play",
        "data.skills.win",
        "data.skills.pick_rate",
        "data.strong_counters[].champion_id",
        "data.strong_counters[].champion_name",
        "data.strong_counters[].play",
        "data.strong_counters[].my_win_rate",
        "data.strong_counters[].counter_win_rate",
        "data.weak_counters[].champion_id",
        "data.weak_counters[].champion_name",
        "data.weak_counters[].play",
        "data.weak_counters[].my_win_rate",
        "data.weak_counters[].counter_win_rate",
    ]
}

/// Data layer for champion build/stats/counter recommendations. Backed by
/// OP.GG's officially maintained MCP server rather than scraping u.gg (see
/// project notes). UI code should never call `OpggClient` directly — go
/// through this service so caching stays centralized and the provider stays
/// swappable, per the UI -> RecommendationService -> DataService -> Cache ->
/// Source layering.
pub struct OpggDataService {
    client: OpggClient,
    cache: Cache,
}

impl OpggDataService {
    pub fn new() -> Self {
        Self {
            client: OpggClient::new(),
            cache: Cache::new(),
        }
    }

    pub async fn champion_analysis(
        &self,
        champion: &str,
        position: &str,
        tier: &str,
        game_mode: &str,
    ) -> Result<Value, OpggError> {
        let key = format!("champion_analysis:{champion}:{position}:{tier}:{game_mode}");
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let args = serde_json::json!({
            "champion": champion.to_uppercase(),
            "position": position,
            "tier": tier,
            "game_mode": game_mode,
            "desired_output_fields": champion_analysis_fields(),
        });
        let value = self
            .client
            .call_tool("lol_get_champion_analysis", args)
            .await?;
        self.cache
            .set(key, value.clone(), CHAMPION_ANALYSIS_TTL)
            .await;
        Ok(value)
    }

    /// Lane tier list from `lol_list_lane_meta_champions`. Note: unlike
    /// `lol_get_champion_analysis`, this tool's schema has no rank/tier
    /// input at all — confirmed live (passing one is silently ignored, same
    /// response either way) — so it always reflects OP.GG's own aggregate
    /// rank bucket, not a caller-selectable one.
    pub async fn lane_meta_champions(&self, position: &str) -> Result<Value, OpggError> {
        let key = format!("lane_meta:{position}");
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let fields = vec![
            format!("data.positions.{position}[].champion"),
            format!("data.positions.{position}[].play"),
            format!("data.positions.{position}[].win_rate"),
            format!("data.positions.{position}[].pick_rate"),
            format!("data.positions.{position}[].ban_rate"),
            format!("data.positions.{position}[].tier"),
        ];
        let args = serde_json::json!({
            "position": position,
            "desired_output_fields": fields,
        });
        let value = self
            .client
            .call_tool("lol_list_lane_meta_champions", args)
            .await?;
        self.cache.set(key, value.clone(), LANE_META_TTL).await;
        Ok(value)
    }

    pub async fn aram_augments(&self, champion_id: i64) -> Result<Value, OpggError> {
        let key = format!("aram_augments:{champion_id}");
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let args = serde_json::json!({
            "champion_id": champion_id,
            "desired_output_fields": [
                "data.augments[].id",
                "data.augments[].name",
                "data.augments[].desc",
                "data.augments[].tier",
                "data.augments[].performance",
                "data.augments[].popular",
            ],
        });
        let value = self
            .client
            .call_tool("lol_list_aram_augments", args)
            .await?;
        self.cache.set(key, value.clone(), ARAM_AUGMENTS_TTL).await;
        Ok(value)
    }

    /// Ranked tier/division/LP for one summoner — used to show an enemy's
    /// rank once a live game has revealed their Riot ID (never available
    /// during champ select, which is deliberate on Riot's part).
    pub async fn summoner_profile(
        &self,
        game_name: &str,
        tag_line: &str,
        region: &str,
    ) -> Result<Value, OpggError> {
        let key = format!("summoner_profile:{region}:{game_name}#{tag_line}");
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let args = serde_json::json!({
            "game_name": game_name,
            "tag_line": tag_line,
            "region": region,
            "desired_output_fields": [
                "data.summoner.league_stats[].game_type",
                "data.summoner.league_stats[].tier_info.tier",
                "data.summoner.league_stats[].tier_info.division",
                "data.summoner.league_stats[].tier_info.lp",
                "data.summoner.league_stats[].win",
                "data.summoner.league_stats[].lose",
            ],
        });
        let value = self
            .client
            .call_tool("lol_get_summoner_profile", args)
            .await?;
        self.cache.set(key, value.clone(), SUMMONER_PROFILE_TTL).await;
        Ok(value)
    }

    /// Recent match history for one summoner — used to work out their main
    /// role and their win rate specifically in whatever position they're
    /// playing in the current live game.
    pub async fn summoner_matches(
        &self,
        game_name: &str,
        tag_line: &str,
        region: &str,
    ) -> Result<Value, OpggError> {
        let key = format!("summoner_matches:{region}:{game_name}#{tag_line}");
        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

        let args = serde_json::json!({
            "game_name": game_name,
            "tag_line": tag_line,
            "region": region,
            "limit": 20,
            "desired_output_fields": [
                "data.game_history[].game_type",
                "data.game_history[].participants[].position",
                "data.game_history[].participants[].stats.result",
                "data.game_history[].participants[].summoner.game_name",
                "data.game_history[].participants[].summoner.tagline",
            ],
        });
        let value = self
            .client
            .call_tool("lol_list_summoner_matches", args)
            .await?;
        self.cache.set(key, value.clone(), SUMMONER_MATCHES_TTL).await;
        Ok(value)
    }
}

impl Default for OpggDataService {
    fn default() -> Self {
        Self::new()
    }
}
