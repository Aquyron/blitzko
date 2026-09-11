use serde::Serialize;

#[derive(Debug, Clone)]
pub struct LcuConnectionInfo {
    pub pid: u32,
    pub port: u16,
    pub password: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum LeagueStatus {
    #[default]
    NotRunning,
    Connecting,
    #[serde(rename_all = "camelCase")]
    Connected {
        summoner_name: String,
        game_version: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GameflowState {
    pub phase: String,
}

impl Default for GameflowState {
    fn default() -> Self {
        Self {
            phase: "None".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectState {
    pub active: bool,
    pub my_champion_id: Option<i64>,
    /// True only once the local player's pick is actually locked in — unlike
    /// `my_champion_id` (which updates live as you hover, including during
    /// the pre-ban "pick intent" window), this stays false through every
    /// hover and only flips once the choice is final.
    pub my_champion_locked: bool,
    pub my_position: Option<String>,
    pub enemy_champion_ids: Vec<i64>,
    pub queue_game_mode: Option<String>,
    /// The local player's currently in-progress action (the one turn they
    /// can actually act on right now) — `None` when it isn't their turn, so
    /// the UI knows when a suggested ban/pick button may be submitted.
    pub my_action_id: Option<i64>,
    /// "ban" or "pick", mirroring the action's own `type` field.
    pub my_action_type: Option<String>,
    /// Whether the local player still has an uncompleted ban action —
    /// `false` once they've banned (or if this queue has no ban phase at
    /// all), so the UI can drop the ban suggestions entirely instead of
    /// showing "waiting for your ban" forever after they're done.
    pub my_ban_pending: bool,
    /// Every champion banned so far by either team (completed bans only),
    /// so ban/pick suggestions can skip champions that are no longer
    /// available.
    pub banned_champion_ids: Vec<i64>,
    /// Champions already locked in by the local player's own team — a
    /// second teammate can't also lock the same one in, so suggested picks
    /// should skip these too.
    pub my_team_champion_ids: Vec<i64>,
}
