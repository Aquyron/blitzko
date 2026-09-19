mod commands;
mod data;
mod league;
mod state;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered before any other plugin — a second launch
        // (e.g. double-clicking the shortcut while the app is already
        // running, hidden in the tray) exits immediately here instead of
        // spawning a whole new process with its own tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
            commands::get_post_game_stats,
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

            // Closing the window shouldn't kill the League companion — it
            // needs to keep running in the background. Instead we hide it
            // to the system tray and let the user reopen it from there.
            let show_item = MenuItem::with_id(app, "show", "Show Blitzko", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Dropping the TrayIcon handle removes it from the system tray
            // immediately — keep it alive for the life of the app.
            app.manage(tray);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // On macOS, clicking the Dock icon while the window is hidden
            // doesn't reopen it by default — Tauri leaves that decision to
            // us. `RunEvent::Reopen` only exists on macOS (Windows/Linux
            // have no equivalent "dock reopen" concept) — confirmed by a
            // failed Windows CI build (E0599: no variant `Reopen`) after
            // this was first written without the cfg gate.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
