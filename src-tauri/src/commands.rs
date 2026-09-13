use serde::Serialize;
use tauri::State;

use crate::league::item_set_apply::{self, ItemBlock, ItemBlockInput, ItemSetSpec};
use crate::league::live_client::{self, LiveClient};
use crate::league::rune_apply::{self, RuneSpec};
use crate::league::types::{ChampSelectState, GameflowState, LeagueStatus};
use crate::state::AppState;

#[tauri::command]
pub fn get_league_status(state: State<AppState>) -> LeagueStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_gameflow_phase(state: State<AppState>) -> GameflowState {
    state.gameflow.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_champ_select_state(state: State<AppState>) -> ChampSelectState {
    state.champ_select.lock().unwrap().clone()
}

#[tauri::command]
pub async fn apply_runes(
    state: State<'_, AppState>,
    page_name: String,
    primary_style_id: i64,
    sub_style_id: i64,
    selected_perk_ids: Vec<i64>,
) -> Result<(), String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    rune_apply::apply_runes(
        &client,
        RuneSpec {
            page_name,
            primary_style_id,
            sub_style_id,
            selected_perk_ids,
        },
    )
    .await
}

async fn current_summoner_id(client: &crate::league::lcu_client::LcuClient) -> Result<i64, String> {
    let summoner = client.current_summoner().await.map_err(|e| e.to_string())?;
    summoner
        .get("summonerId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Could not read summoner id".to_string())
}

#[tauri::command]
pub async fn apply_item_set(
    state: State<'_, AppState>,
    title: String,
    champion_id: i64,
    blocks: Vec<ItemBlockInput>,
) -> Result<(), String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    let summoner_id = current_summoner_id(&client).await?;

    item_set_apply::apply_item_set(
        &client,
        summoner_id,
        ItemSetSpec {
            champion_id,
            title,
            blocks: blocks
                .into_iter()
                .map(|b| ItemBlock {
                    block_type: b.block_type,
                    item_ids: b.item_ids,
                })
                .collect(),
        },
    )
    .await
}

#[tauri::command]
pub async fn apply_summoner_spells(
    state: State<'_, AppState>,
    spell1_id: i64,
    spell2_id: i64,
) -> Result<(), String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    client
        .set_champ_select_spells(spell1_id, spell2_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_champ_select_action(
    state: State<'_, AppState>,
    action_id: i64,
    champion_id: i64,
) -> Result<(), String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    client
        .set_champ_select_action(action_id, champion_id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnemyInfo {
    pub champion_name: String,
    pub game_name: String,
    pub tag_line: String,
    pub position: String,
    pub spell1_name: String,
    pub spell2_name: String,
    pub keystone_name: String,
    pub primary_tree_name: String,
    pub secondary_tree_name: String,
}

fn to_enemy_info(e: live_client::LiveEnemy) -> EnemyInfo {
    EnemyInfo {
        champion_name: e.champion_name,
        game_name: e.game_name,
        tag_line: e.tag_line,
        position: e.position,
        spell1_name: e.spell1_name,
        spell2_name: e.spell2_name,
        keystone_name: e.keystone_name,
        primary_tree_name: e.primary_tree_name,
        secondary_tree_name: e.secondary_tree_name,
    }
}

/// Reads the live game (only reachable once a match has actually loaded,
/// per Riot's own design — never during champ select) and returns the
/// enemy team's champions, Riot IDs, spells and runes.
#[tauri::command]
pub async fn get_live_game_enemies() -> Result<Vec<EnemyInfo>, String> {
    let client = LiveClient::new();
    let data = client.all_game_data().await.map_err(|e| e.to_string())?;
    Ok(live_client::parse_enemies(&data)
        .into_iter()
        .map(to_enemy_info)
        .collect())
}

/// Same as `get_live_game_enemies` but for the local player's own team —
/// unlike champ select (where teammate identity comes from the LCU's
/// `summonerId`, since the game hasn't launched yet), once the game has
/// actually loaded this is simpler: read it from the same live-game feed
/// used for the enemy team.
#[tauri::command]
pub async fn get_live_game_allies() -> Result<Vec<EnemyInfo>, String> {
    let client = LiveClient::new();
    let data = client.all_game_data().await.map_err(|e| e.to_string())?;
    Ok(live_client::parse_allies(&data)
        .into_iter()
        .map(to_enemy_info)
        .collect())
}

/// The LCU client's platform region, mapped to OP.GG's shorter region code
/// — shared by every command that needs to look a summoner up on OP.GG.
async fn detect_region(
    client: &crate::league::lcu_client::LcuClient,
) -> Result<String, String> {
    let region_raw = client.region_locale().await.map_err(|e| e.to_string())?;
    let platform = region_raw
        .get("region")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Could not read region".to_string())?;
    Ok(live_client::normalize_region(platform))
}

#[tauri::command]
pub async fn get_enemy_rank(
    state: State<'_, AppState>,
    game_name: String,
    tag_line: String,
) -> Result<serde_json::Value, String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    let region = detect_region(&client).await?;

    state
        .opgg
        .summoner_profile(&game_name, &tag_line, &region)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaneStatsInfo {
    pub main_position: Option<String>,
    pub is_main_role: bool,
    pub lane_games: i64,
    pub lane_wins: i64,
}

/// Finds this summoner's most-played position across their recent match
/// history, and their win rate specifically in `current_position` — so the
/// UI can flag "Correct lane" vs. a likely autofill.
/// **Unverified live** — `stats.result`'s exact win/loss string value
/// hasn't been confirmed against a real response yet, so both "WIN" and
/// "true"-ish equivalents are checked defensively.
fn analyze_lane_stats(
    matches: &serde_json::Value,
    game_name: &str,
    tag_line: &str,
    current_position: &str,
) -> LaneStatsInfo {
    let mut position_counts: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    let mut lane_games = 0i64;
    let mut lane_wins = 0i64;

    if let Some(history) = matches.pointer("/data/game_history").and_then(|v| v.as_array()) {
        for game in history {
            let Some(participants) = game.get("participants").and_then(|v| v.as_array()) else {
                continue;
            };
            let me = participants.iter().find(|p| {
                let pgn = p
                    .pointer("/summoner/game_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ptl = p
                    .pointer("/summoner/tagline")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                pgn.eq_ignore_ascii_case(game_name) && ptl.eq_ignore_ascii_case(tag_line)
            });
            let Some(me) = me else { continue };

            let position = me
                .get("position")
                .and_then(|v| v.as_str())
                .unwrap_or("NONE")
                .to_string();
            if position != "NONE" && !position.is_empty() {
                *position_counts.entry(position.clone()).or_insert(0) += 1;
            }

            if position == current_position {
                let won = me
                    .pointer("/stats/result")
                    .and_then(|v| v.as_str())
                    .map(|s| s.eq_ignore_ascii_case("WIN") || s.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                lane_games += 1;
                if won {
                    lane_wins += 1;
                }
            }
        }
    }

    let main_position = position_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(pos, _)| pos);

    LaneStatsInfo {
        is_main_role: main_position.as_deref() == Some(current_position),
        main_position,
        lane_games,
        lane_wins,
    }
}

#[tauri::command]
pub async fn get_enemy_lane_stats(
    state: State<'_, AppState>,
    game_name: String,
    tag_line: String,
    current_position: String,
) -> Result<LaneStatsInfo, String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    let region = detect_region(&client).await?;

    let matches = state
        .opgg
        .summoner_matches(&game_name, &tag_line, &region)
        .await
        .map_err(|e| e.to_string())?;

    Ok(analyze_lane_stats(&matches, &game_name, &tag_line, &current_position))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeammateRankInfo {
    pub game_name: String,
    pub tag_line: String,
    pub rank: Option<serde_json::Value>,
    pub lane: LaneStatsInfo,
}

/// Rank + lane-fit for one teammate during champ select — identity here
/// comes from the LCU's own champ-select session (`summonerId`), which,
/// unlike the enemy team, Riot doesn't hide from us.
#[tauri::command]
pub async fn get_teammate_rank(
    state: State<'_, AppState>,
    summoner_id: i64,
    position: String,
) -> Result<TeammateRankInfo, String> {
    let client = state
        .current_client
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "League Client is not connected".to_string())?;

    let summoner = client
        .summoner_by_id(summoner_id)
        .await
        .map_err(|e| e.to_string())?;
    let game_name = summoner
        .get("gameName")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Could not read summoner gameName".to_string())?
        .to_string();
    let tag_line = summoner
        .get("tagLine")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let region = detect_region(&client).await?;

    let rank = state
        .opgg
        .summoner_profile(&game_name, &tag_line, &region)
        .await
        .ok();

    let lane = if position != "NONE" {
        state
            .opgg
            .summoner_matches(&game_name, &tag_line, &region)
            .await
            .ok()
            .map(|matches| analyze_lane_stats(&matches, &game_name, &tag_line, &position))
            .unwrap_or(LaneStatsInfo {
                main_position: None,
                is_main_role: false,
                lane_games: 0,
                lane_wins: 0,
            })
    } else {
        LaneStatsInfo {
            main_position: None,
            is_main_role: false,
            lane_games: 0,
            lane_wins: 0,
        }
    };

    Ok(TeammateRankInfo {
        game_name,
        tag_line,
        rank,
        lane,
    })
}

#[tauri::command]
pub async fn get_champion_build(
    state: State<'_, AppState>,
    champion: String,
    position: String,
    tier: String,
    game_mode: String,
) -> Result<serde_json::Value, String> {
    state
        .opgg
        .champion_analysis(&champion, &position, &tier, &game_mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_champion_list(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state.ddragon.champions().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_aram_augments(
    state: State<'_, AppState>,
    champion_id: i64,
) -> Result<serde_json::Value, String> {
    state
        .opgg
        .aram_augments(champion_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_lane_tier_list(
    state: State<'_, AppState>,
    position: String,
) -> Result<serde_json::Value, String> {
    state
        .opgg
        .lane_meta_champions(&position)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ddragon_version(state: State<'_, AppState>) -> Result<String, String> {
    state.ddragon.latest_version().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_item_list(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state.ddragon.items().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_summoner_spell_list(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    state
        .ddragon
        .summoner_spells()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_rune_list(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state.ddragon.runes().await.map_err(|e| e.to_string())
}
