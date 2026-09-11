//! Parses `/lol-champ-select/v1/session` into our `ChampSelectState`.
//!
//! The field shape here (`localPlayerCellId`, `myTeam[]`/`theirTeam[]` with
//! `cellId`/`championId`/`assignedPosition`, `actions[][]` with
//! `actorCellId`/`championId`/`type`) is long-standing community-documented
//! LCU behavior, not something Riot publishes.
//!
//! Confirmed live (2026-09): `myTeam[].championId` only updates at
//! **lock-in**, not on hover/click — too late for a companion app that's
//! supposed to react as you're still deciding. The live, updates-as-you-click
//! value lives in the player's own entry in `actions[][]` (a "pick" action
//! whose `actorCellId` matches `localPlayerCellId`); the client keeps that
//! `championId` in sync with whatever you're currently hovering, well before
//! you lock in. We read `actions` first and only fall back to `myTeam` if no
//! matching action is found.

use serde_json::Value;

use super::types::ChampSelectState;

pub fn parse_session(value: &Value) -> ChampSelectState {
    let local_cell_id = value.get("localPlayerCellId").and_then(|v| v.as_i64());

    let my_champion_id = local_cell_id.and_then(|cell_id| my_champion_from_actions(value, cell_id));

    let mut my_position = None;
    let mut my_champion_fallback = None;
    if let (Some(cell_id), Some(team)) = (
        local_cell_id,
        value.get("myTeam").and_then(|v| v.as_array()),
    ) {
        if let Some(me) = team
            .iter()
            .find(|p| p.get("cellId").and_then(|v| v.as_i64()) == Some(cell_id))
        {
            my_champion_fallback = me
                .get("championId")
                .and_then(|v| v.as_i64())
                .filter(|&id| id != 0);
            my_position = me
                .get("assignedPosition")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(normalize_position)
                // Custom/bot lobbies don't run the matchmaking role-preference
                // system, so a human player's `assignedPosition` often comes
                // back empty even though bots get theirs — but a player who
                // picked Smite (spell id 11) is unambiguously jungling
                // regardless of what the lobby did or didn't assign.
                .or_else(|| has_smite(me).then(|| "jungle".to_string()));
        }
    }

    let enemy_champion_ids = value
        .get("theirTeam")
        .and_then(|v| v.as_array())
        .map(|team| {
            team.iter()
                .filter_map(|p| p.get("championId").and_then(|v| v.as_i64()))
                .filter(|&id| id != 0)
                .collect()
        })
        .unwrap_or_default();

    let my_team_champion_ids = value
        .get("myTeam")
        .and_then(|v| v.as_array())
        .map(|team| {
            team.iter()
                .filter_map(|p| p.get("championId").and_then(|v| v.as_i64()))
                .filter(|&id| id != 0)
                .collect()
        })
        .unwrap_or_default();

    let (my_action_id, my_action_type) = local_cell_id
        .and_then(|cell_id| my_active_action(value, cell_id))
        .unzip();

    let my_ban_pending = local_cell_id
        .map(|cell_id| has_pending_ban(value, cell_id))
        .unwrap_or(false);

    let banned_champion_ids = banned_champions(value);

    ChampSelectState {
        active: true,
        my_champion_id: my_champion_id.or(my_champion_fallback),
        my_champion_locked: my_champion_fallback.is_some(),
        my_position,
        enemy_champion_ids,
        queue_game_mode: None,
        my_action_id,
        my_action_type,
        my_ban_pending,
        banned_champion_ids,
        my_team_champion_ids,
    }
}

/// Finds the local player's currently-actionable turn: the one action in
/// `actions[][]` that belongs to them (`actorCellId` matches) and is still
/// `isInProgress` (not yet completed) — i.e. what they can submit a
/// ban/pick for right now.
fn my_active_action(session: &Value, cell_id: i64) -> Option<(i64, String)> {
    let rounds = session.get("actions")?.as_array()?;
    rounds
        .iter()
        .filter_map(|round| round.as_array())
        .flatten()
        .find(|action| {
            action.get("actorCellId").and_then(|v| v.as_i64()) == Some(cell_id)
                && action.get("isInProgress").and_then(|v| v.as_bool()) == Some(true)
        })
        .and_then(|action| {
            let id = action.get("id").and_then(|v| v.as_i64())?;
            let kind = action.get("type").and_then(|v| v.as_str())?.to_string();
            Some((id, kind))
        })
}

/// Every champion banned so far by either team — only `completed` bans
/// count, since an in-progress (still-being-hovered) ban isn't actually
/// locked out yet.
fn banned_champions(session: &Value) -> Vec<i64> {
    let Some(rounds) = session.get("actions").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    rounds
        .iter()
        .filter_map(|round| round.as_array())
        .flatten()
        .filter(|action| {
            action.get("type").and_then(|v| v.as_str()) == Some("ban")
                && action.get("completed").and_then(|v| v.as_bool()) == Some(true)
        })
        .filter_map(|action| action.get("championId").and_then(|v| v.as_i64()))
        .filter(|&id| id != 0)
        .collect()
}

/// Whether the local player still has a ban action they haven't completed
/// yet — `false` once they've locked in a ban, and also `false` for queues
/// that have no ban phase at all (no "ban" action for them to find).
fn has_pending_ban(session: &Value, cell_id: i64) -> bool {
    let Some(rounds) = session.get("actions").and_then(|v| v.as_array()) else {
        return false;
    };
    rounds
        .iter()
        .filter_map(|round| round.as_array())
        .flatten()
        .filter(|action| {
            action.get("actorCellId").and_then(|v| v.as_i64()) == Some(cell_id)
                && action.get("type").and_then(|v| v.as_str()) == Some("ban")
        })
        .any(|action| action.get("completed").and_then(|v| v.as_bool()) != Some(true))
}

/// Summoner spell id 11 is Smite — a reliable jungle tell independent of
/// whatever (if anything) the lobby assigned as a position.
const SMITE_SPELL_ID: i64 = 11;

fn has_smite(team_member: &Value) -> bool {
    [team_member.get("spell1Id"), team_member.get("spell2Id")]
        .into_iter()
        .flatten()
        .any(|v| v.as_i64() == Some(SMITE_SPELL_ID))
}

/// Finds the local player's own "pick" action in the (nested) `actions`
/// array and reads its live `championId` — this is the field the client
/// updates in real time as the player hovers/clicks different champions,
/// not just at lock-in.
fn my_champion_from_actions(session: &Value, cell_id: i64) -> Option<i64> {
    let rounds = session.get("actions")?.as_array()?;
    rounds
        .iter()
        .filter_map(|round| round.as_array())
        .flatten()
        .find(|action| {
            action.get("actorCellId").and_then(|v| v.as_i64()) == Some(cell_id)
                && action.get("type").and_then(|v| v.as_str()) == Some("pick")
        })
        .and_then(|action| action.get("championId"))
        .and_then(|v| v.as_i64())
        .filter(|&id| id != 0)
}

/// Riot's `gameData.queue.gameMode` (from `/lol-gameflow/v1/session`) -> the
/// mode keys our OP.GG data layer expects. **Unverified live.** "CLASSIC"
/// covers Summoner's Rift generally (ranked/normal/flex all report it) so
/// it maps to "ranked" as a reasonable default rather than trying to
/// distinguish further.
pub fn normalize_queue_mode(raw: &str) -> Option<String> {
    let mapped = match raw {
        "CLASSIC" => "ranked",
        // "KIWI" confirmed live (2026-09) as the actual `queue.gameMode`
        // value for ARAM Mayhem (both custom and presumably matchmade —
        // it's the queue/mode definition, not tied to custom vs. matched).
        // "ARAM" kept too in case an older/different queue still reports it.
        "KIWI" | "ARAM" => "aram",
        "URF" | "ARURF" => "urf",
        "NEXUSBLITZ" => "nexus_blitz",
        _ => return None,
    };
    Some(mapped.to_string())
}

/// LCU position names (top/jungle/middle/bottom/utility) -> our canonical
/// role keys, matching what the OP.GG data layer and the UI's role picker
/// use (top/jungle/mid/adc/support).
fn normalize_position(raw: &str) -> String {
    match raw {
        "middle" => "mid",
        "bottom" => "adc",
        "utility" => "support",
        other => other,
    }
    .to_string()
}
