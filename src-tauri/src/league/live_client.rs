//! Client for Riot's official, publicly documented Live Client Data API —
//! exposed by the League *game* client itself (not the LCU) on a fixed
//! local port once a match is actually loading/in-progress. No auth, no
//! lockfile needed; it simply isn't reachable outside of a live game.
//!
//! **Unverified live** — the shape below (`riotIdGameName`/`riotIdTagLine`/
//! `team` per entry in `allPlayers[]`) matches Riot's long-standing public
//! documentation for this API, but hasn't been confirmed against a real
//! match from this app yet.

use std::time::Duration;

use serde_json::Value;

const LIVE_CLIENT_BASE: &str = "https://127.0.0.1:2999";

pub struct LiveClient {
    http: reqwest::Client,
}

impl LiveClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(3))
            .build()
            .expect("failed to build live-client-data http client");
        Self { http }
    }

    /// Fails fast (short timeout) since this port is only listening while a
    /// match is actually loaded — during champ select or the post-game
    /// lobby it's simply not there yet.
    pub async fn all_game_data(&self) -> Result<Value, reqwest::Error> {
        self.http
            .get(format!("{LIVE_CLIENT_BASE}/liveclientdata/allgamedata"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

impl Default for LiveClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Maps the LCU's platform-id-style region (as read from
/// `/riotclient/region-locale`, e.g. "NA1", "EUN1") to the shorter region
/// codes OP.GG's API expects (e.g. "NA", "EUNE"). **Unverified live** —
/// standard, long-stable Riot platform ids, but the exact set OP.GG accepts
/// hasn't been confirmed beyond the "KR, BR, EUNE" examples in their own
/// tool schema.
pub fn normalize_region(platform: &str) -> String {
    match platform.to_uppercase().as_str() {
        "NA1" => "NA",
        "EUW1" => "EUW",
        "EUN1" => "EUNE",
        "KR" => "KR",
        "BR1" => "BR",
        "LA1" => "LAN",
        "LA2" => "LAS",
        "OC1" => "OCE",
        "TR1" => "TR",
        "RU" => "RU",
        "JP1" => "JP",
        "PH2" => "PH",
        "SG2" => "SG",
        "TH2" => "TH",
        "TW2" => "TW",
        "VN2" => "VN",
        other => other,
    }
    .to_string()
}

pub struct LiveEnemy {
    pub champion_name: String,
    pub game_name: String,
    pub tag_line: String,
    /// "TOP"/"JUNGLE"/"MIDDLE"/"BOTTOM"/"UTILITY", or "NONE" when the client
    /// hasn't assigned one (common in customs/blind pick) — matches the
    /// same position strings OP.GG's match history reports, so no
    /// normalization is needed to compare them.
    pub position: String,
    pub spell1_name: String,
    pub spell2_name: String,
    pub keystone_name: String,
    pub primary_tree_name: String,
    pub secondary_tree_name: String,
}

/// Finds the local player's team (by matching `activePlayer`'s Riot ID
/// against `allPlayers[]`) and returns everyone on the other team.
pub fn parse_enemies(data: &Value) -> Vec<LiveEnemy> {
    parse_team(data, false)
}

/// Same as `parse_enemies` but for the local player's own team (bots
/// included — filtered out downstream by an empty `riotIdGameName`, same
/// as enemies).
pub fn parse_allies(data: &Value) -> Vec<LiveEnemy> {
    parse_team(data, true)
}

fn parse_team(data: &Value, want_own_team: bool) -> Vec<LiveEnemy> {
    let Some(players) = data.get("allPlayers").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let active_game_name = data.pointer("/activePlayer/riotIdGameName").and_then(|v| v.as_str());
    let active_tag_line = data.pointer("/activePlayer/riotIdTagLine").and_then(|v| v.as_str());

    let my_team = players
        .iter()
        .find(|p| {
            p.get("riotIdGameName").and_then(|v| v.as_str()) == active_game_name
                && p.get("riotIdTagLine").and_then(|v| v.as_str()) == active_tag_line
        })
        .and_then(|p| p.get("team"))
        .and_then(|v| v.as_str());

    let Some(my_team) = my_team else {
        return Vec::new();
    };

    players
        .iter()
        .filter(|p| {
            let same_team = p.get("team").and_then(|v| v.as_str()) == Some(my_team);
            same_team == want_own_team
        })
        .filter_map(|p| {
            let champion_name = p.get("championName")?.as_str()?.to_string();
            let game_name = p.get("riotIdGameName")?.as_str()?.to_string();
            let tag_line = p.get("riotIdTagLine")?.as_str()?.to_string();
            if game_name.is_empty() {
                return None;
            }
            let position = p
                .get("position")
                .and_then(|v| v.as_str())
                .unwrap_or("NONE")
                .to_string();
            let spell1_name = p
                .pointer("/summonerSpells/summonerSpellOne/displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let spell2_name = p
                .pointer("/summonerSpells/summonerSpellTwo/displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let keystone_name = p
                .pointer("/runes/keystone/displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let primary_tree_name = p
                .pointer("/runes/primaryRuneTree/displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let secondary_tree_name = p
                .pointer("/runes/secondaryRuneTree/displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(LiveEnemy {
                champion_name,
                game_name,
                tag_line,
                position,
                spell1_name,
                spell2_name,
                keystone_name,
                primary_tree_name,
                secondary_tree_name,
            })
        })
        .collect()
}
