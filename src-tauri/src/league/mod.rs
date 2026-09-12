pub mod champ_select;
pub mod item_set_apply;
pub mod lcu_client;
pub mod live_client;
pub mod process_detector;
pub mod rune_apply;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::FutureExt;
use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::state::AppState;
use lcu_client::{EventWatchHandles, LcuClient, LcuWsEvent};
use types::{ChampSelectState, GameflowState, LeagueStatus};

const DISCOVERY_INTERVAL: Duration = Duration::from_secs(15);
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(500);
const CONNECT_RETRY_ATTEMPTS: u32 = 10;

const URI_GAMEFLOW_PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
const URI_CHAMP_SELECT_SESSION: &str = "/lol-champ-select/v1/session";

/// Drives League detection end-to-end: watches every candidate lockfile
/// location with the OS's native file-watcher (no polling once at least one
/// candidate dir exists) and connects to the LCU whenever a lockfile shows
/// up. Runs for the lifetime of the app as a single background task.
pub async fn run(app: AppHandle) {
    let candidates = process_detector::candidate_lockfile_paths();
    if candidates.is_empty() {
        set_status(
            &app,
            LeagueStatus::Error {
                message: "League detection is not supported on this platform".into(),
            },
        );
        return;
    }

    // Tracks the background loops for whichever LcuClient is currently
    // active, so a superseded one (client restart, reconnect) gets fully
    // aborted instead of leaking a retry loop that spins forever against a
    // dead port.
    let mut event_handles: Option<EventWatchHandles> = None;

    loop {
        let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<notify::Event>();
        let mut watched_dirs: Vec<PathBuf> = Vec::new();
        let mut watchers = Vec::new();

        for candidate in &candidates {
            if let Some(parent) = candidate.parent() {
                if parent.exists() && !watched_dirs.contains(&parent.to_path_buf()) {
                    if let Ok(w) = spawn_watcher(parent.to_path_buf(), fs_tx.clone()) {
                        watchers.push(w);
                        watched_dirs.push(parent.to_path_buf());
                    }
                }
            }
        }

        // The lockfile may already exist before the watchers above armed.
        if let Some(found) = candidates.iter().find(|p| p.exists()) {
            handle_lockfile_present(&app, found, &mut event_handles).await;
        } else {
            set_status(&app, LeagueStatus::NotRunning);
        }

        if watched_dirs.is_empty() {
            // None of the candidate parent dirs exist yet (League likely not
            // installed) — cheap coarse discovery poll until one appears.
            tokio::time::sleep(DISCOVERY_INTERVAL).await;
            continue;
        }

        loop {
            tokio::select! {
                event = fs_rx.recv() => {
                    let Some(event) = event else { break };
                    let Some(matched) = candidates.iter().find(|p| event.paths.contains(p)) else {
                        continue;
                    };
                    if matched.exists() {
                        handle_lockfile_present(&app, matched, &mut event_handles).await;
                    } else {
                        if let Some(handles) = event_handles.take() {
                            handles.abort();
                        }
                        set_status(&app, LeagueStatus::NotRunning);
                        set_gameflow(&app, GameflowState::default());
                        set_champ_select(&app, ChampSelectState::default());
                        set_current_client(&app, None);
                    }
                }
                _ = tokio::time::sleep(DISCOVERY_INTERVAL) => {
                    let newly_available = candidates.iter().any(|p| {
                        p.parent()
                            .map(|d| d.exists() && !watched_dirs.contains(&d.to_path_buf()))
                            .unwrap_or(false)
                    });
                    if newly_available {
                        break;
                    }
                }
            }
        }
        drop(watchers);
    }
}

fn spawn_watcher(
    dir: PathBuf,
    tx: mpsc::UnboundedSender<notify::Event>,
) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

async fn handle_lockfile_present(
    app: &AppHandle,
    lockfile: &PathBuf,
    event_handles: &mut Option<EventWatchHandles>,
) {
    let Some(info) = process_detector::parse_lockfile(lockfile) else {
        set_status(
            app,
            LeagueStatus::Error {
                message: "Could not parse League lockfile".into(),
            },
        );
        return;
    };

    // A new lockfile always means a new (or restarted) client — any loops
    // still chasing the previous port are now talking to nothing.
    if let Some(handles) = event_handles.take() {
        handles.abort();
    }

    set_status(app, LeagueStatus::Connecting);
    let client = Arc::new(LcuClient::new(info));

    let mut last_err = String::new();
    for _ in 0..CONNECT_RETRY_ATTEMPTS {
        match client.current_summoner().await {
            Ok(summoner) => {
                let summoner_name = summoner
                    .get("gameName")
                    .or_else(|| summoner.get("displayName"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Summoner")
                    .to_string();
                let game_version = client
                    .game_version()
                    .await
                    .unwrap_or_else(|| "unknown".into());

                set_status(
                    app,
                    LeagueStatus::Connected {
                        summoner_name,
                        game_version,
                    },
                );
                set_current_client(app, Some(client.clone()));

                // Sync immediately rather than waiting for the next push
                // event or poll tick — otherwise reconnecting (app restart)
                // while already mid-champ-select shows nothing until the
                // next change, up to POLL_FALLBACK_INTERVAL later.
                let mut last_phase = String::new();
                let mut queue_game_mode: Option<String> = None;
                if let Ok(phase) = client.gameflow_phase().await {
                    last_phase = phase.clone();
                    sync_gameflow_phase(app, &client, &phase, &mut queue_game_mode).await;
                }

                let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<LcuWsEvent>();
                *event_handles = Some(client.clone().watch_events(ev_tx));
                let app_clone = app.clone();
                let client_for_events = client.clone();
                tokio::spawn(async move {
                    let mut last_phase = last_phase;
                    let mut queue_game_mode = queue_game_mode;
                    while let Some(event) = ev_rx.recv().await {
                        // A panic while handling one event must not kill this
                        // whole loop — that would silently freeze every
                        // future LCU update (status/gameflow/champ-select)
                        // until the app is restarted, with no crash to
                        // signal it. `AssertUnwindSafe` is fine here: on a
                        // caught panic the worst case is `last_phase` /
                        // `queue_game_mode` reflecting a half-applied
                        // update, which just gets corrected by the next
                        // event.
                        let result = std::panic::AssertUnwindSafe(handle_lcu_event(
                            &app_clone,
                            &client_for_events,
                            event,
                            &mut last_phase,
                            &mut queue_game_mode,
                        ))
                        .catch_unwind()
                        .await;
                        if let Err(panic) = result {
                            let msg = panic
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| panic.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "<non-string panic payload>".to_string());
                            eprintln!("[blitzko] LCU event handler panicked (recovered): {msg}");
                        }
                    }
                });
                return;
            }
            Err(err) => {
                last_err = err.to_string();
                tokio::time::sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    set_status(
        app,
        LeagueStatus::Error {
            message: format!("LCU connection failed: {last_err}"),
        },
    );
}

/// Applies a (possibly just-changed, possibly just-read-fresh) gameflow
/// phase: updates gameflow state, and when the phase is ChampSelect, reads
/// the queue's game mode and the current champ-select session snapshot.
/// Shared by the live event handler and the immediate post-connect sync so
/// a reconnect mid-champ-select doesn't wait for the next event/poll tick.
async fn sync_gameflow_phase(
    app: &AppHandle,
    client: &Arc<LcuClient>,
    phase: &str,
    queue_game_mode: &mut Option<String>,
) {
    set_gameflow(
        app,
        GameflowState {
            phase: phase.to_string(),
        },
    );

    if phase == "ChampSelect" {
        // Queue mode (ARAM/URF/...) isn't in the champ-select session
        // itself — read it once from the gameflow session so ARAM
        // picks pull ARAM-appropriate data instead of defaulting to
        // Ranked. Unverified field shape, see `gameflow_session`.
        *queue_game_mode = client
            .gameflow_session()
            .await
            .ok()
            .and_then(|s| {
                s.pointer("/gameData/queue/gameMode")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .and_then(|raw| champ_select::normalize_queue_mode(&raw));

        // The push stream may only start delivering session updates
        // after this point, so fetch the current snapshot directly
        // rather than waiting for the next diff.
        if let Ok(session) = client.champ_select_session().await {
            let mut state = champ_select::parse_session(&session);
            state.queue_game_mode = queue_game_mode.clone();
            set_champ_select(app, state);
        }
    } else {
        *queue_game_mode = None;
        set_champ_select(app, ChampSelectState::default());
    }
}

async fn handle_lcu_event(
    app: &AppHandle,
    client: &Arc<LcuClient>,
    event: LcuWsEvent,
    last_phase: &mut String,
    queue_game_mode: &mut Option<String>,
) {
    match event.uri.as_str() {
        URI_GAMEFLOW_PHASE => {
            let Some(phase) = event.data.as_str() else {
                return;
            };
            if phase == last_phase {
                return;
            }
            *last_phase = phase.to_string();
            sync_gameflow_phase(app, client, phase, queue_game_mode).await;
        }
        URI_CHAMP_SELECT_SESSION => {
            // `parse_session` always reports `active: true` for whatever it's
            // handed — it has no way to tell a real session from a stray
            // leftover payload. Confirmed live (2026-09): right as champ
            // select ends, the LCU fires one more session event (near-empty
            // body) *after* the gameflow-phase event has already moved on —
            // trusting it unconditionally flipped our state back to
            // "active" for one frame with no data, right after it had just
            // been correctly reset. Only trust session events while we
            // still believe we're actually in ChampSelect.
            if last_phase != "ChampSelect" {
                return;
            }
            let mut state = champ_select::parse_session(&event.data);
            state.queue_game_mode = queue_game_mode.clone();
            set_champ_select(app, state);
        }
        _ => {}
    }
}

fn set_status(app: &AppHandle, status: LeagueStatus) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.status.lock().unwrap() = status.clone();
    }
    let _ = app.emit("league-status", status);
}

fn set_gameflow(app: &AppHandle, gameflow: GameflowState) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.gameflow.lock().unwrap() = gameflow.clone();
    }
    let _ = app.emit("gameflow-phase", gameflow);
}

fn set_champ_select(app: &AppHandle, champ_select: ChampSelectState) {
    // Temporary: `my_action_id`/`my_action_type` (drives the auto ban/pick
    // suggestions) is still unverified live — logging every change here so
    // a real ban/pick phase's actual values can be checked against the dev
    // server output instead of guessing at the LCU's `actions[]` shape.
    eprintln!(
        "[blitzko] champ-select: active={} action_id={:?} action_type={:?} position={:?} locked={} banned={:?} myTeam={:?}",
        champ_select.active,
        champ_select.my_action_id,
        champ_select.my_action_type,
        champ_select.my_position,
        champ_select.my_champion_locked,
        champ_select.banned_champion_ids,
        champ_select.my_team_champion_ids
    );
    if let Some(state) = app.try_state::<AppState>() {
        *state.champ_select.lock().unwrap() = champ_select.clone();
    }
    let _ = app.emit("champ-select-state", champ_select);
}

fn set_current_client(app: &AppHandle, client: Option<Arc<LcuClient>>) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.current_client.lock().unwrap() = client;
    }
}
