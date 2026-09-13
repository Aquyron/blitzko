mod commands;
mod data;
mod league;
mod state;

use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_league_status,
            commands::get_gameflow_phase,
            commands::get_champ_select_state,
            commands::apply_runes,
            commands::apply_item_set,
            commands::apply_summoner_spells,
            commands::submit_champ_select_action,
            commands::get_live_game_enemies,
            commands::get_live_game_allies,
            commands::get_enemy_rank,
            commands::get_enemy_lane_stats,
            commands::get_teammate_rank,
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

            // A background League companion is only useful if it's
            // actually running when League is — launch it at login so the
            // user never has to remember to open it themselves.
            let autostart = app.autolaunch();
            if !autostart.is_enabled().unwrap_or(false) {
                let _ = autostart.enable();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
