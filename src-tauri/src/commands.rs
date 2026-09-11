use tauri::State;

use crate::league::item_set_apply::{self, ItemBlock, ItemBlockInput, ItemSetSpec};
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
