//! Parses the LCU's end-of-game stats block into a clean post-game
//! scoreboard. **Unverified live** — the `stats` key names below (e.g.
//! `CHAMPIONS_KILLED`, `TOTAL_DAMAGE_DEALT_TO_CHAMPIONS`) match the
//! long-standing, community-documented shape of this endpoint, not
//! something officially published by Riot; confirm against a real
//! completed match before trusting this fully.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostGamePlayer {
    pub summoner_name: String,
    pub champion_id: i64,
    pub is_local_player: bool,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub damage_dealt: i64,
    pub gold_earned: i64,
    pub cs: i64,
    pub vision_score: i64,
    /// Riot doesn't expose an official "MVP" — this is our own heuristic
    /// (KDA impact, damage, gold, vision), used only to rank/highlight
    /// players in our own scoreboard.
    pub mvp_score: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostGameState {
    /// Sorted by `mvpScore`, highest first — index 0 is the MVP.
    pub players: Vec<PostGamePlayer>,
    pub local_player_win: bool,
}

pub fn parse_eog_stats(data: &Value) -> PostGameState {
    let mut players: Vec<PostGamePlayer> = Vec::new();

    let Some(teams) = data.get("teams").and_then(|v| v.as_array()) else {
        return PostGameState::default();
    };

    for team in teams {
        let team_won = team
            .get("isWinningTeam")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let Some(team_players) = team.get("players").and_then(|v| v.as_array()) else {
            continue;
        };

        for p in team_players {
            let stats = p.get("stats");
            let stat = |key: &str| -> f64 {
                stats
                    .and_then(|s| s.get(key))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
            };

            let summoner_name = p
                .get("NAME")
                .or_else(|| p.get("summonerName"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let champion_id = p
                .get("championId")
                .or_else(|| p.get("CHAMPION_ID"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let is_local_player = p
                .get("localPlayer")
                .or_else(|| p.get("IS_LOCAL_PLAYER"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let kills = stat("CHAMPIONS_KILLED");
            let deaths = stat("NUM_DEATHS");
            let assists = stat("ASSISTS");
            let damage_dealt = stat("TOTAL_DAMAGE_DEALT_TO_CHAMPIONS");
            let gold_earned = stat("GOLD_EARNED");
            let cs = stat("MINIONS_KILLED") + stat("NEUTRAL_MINIONS_KILLED");
            let vision_score = stat("VISION_SCORE");

            // Each term scaled to a roughly comparable magnitude so no
            // single stat (e.g. raw damage numbers in the tens of
            // thousands) drowns out the rest.
            let mvp_score = kills * 3.0 + assists * 1.5 - deaths
                + damage_dealt / 100.0
                + gold_earned / 200.0
                + vision_score * 0.5;

            players.push(PostGamePlayer {
                summoner_name,
                champion_id,
                is_local_player,
                win: team_won,
                kills: kills as i64,
                deaths: deaths as i64,
                assists: assists as i64,
                damage_dealt: damage_dealt as i64,
                gold_earned: gold_earned as i64,
                cs: cs as i64,
                vision_score: vision_score as i64,
                mvp_score,
            });
        }
    }

    players.sort_by(|a, b| {
        b.mvp_score
            .partial_cmp(&a.mvp_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let local_player_win = players
        .iter()
        .find(|p| p.is_local_player)
        .map(|p| p.win)
        .unwrap_or(false);

    PostGameState {
        players,
        local_player_win,
    }
}
