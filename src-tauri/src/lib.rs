mod commands;
mod data;
mod league;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_league_status,
            commands::get_gameflow_phase,
            commands::get_champ_select_state,
            commands::apply_runes,
            commands::apply_item_set,
            commands::apply_summoner_spells,
            commands::submit_champ_select_action,
            commands::get_champion_build,
            commands::get_champion_list,
            commands::get_lane_tier_list,
            commands::get_aram_augments,
            commands::get_ddragon_version,
            commands::get_item_list,
            commands::get_summoner_spell_list,
            commands::get_rune_list,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(league::run(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
