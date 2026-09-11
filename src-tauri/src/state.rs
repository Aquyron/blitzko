use std::sync::{Arc, Mutex};

use crate::data::{DdragonService, OpggDataService};
use crate::league::lcu_client::LcuClient;
use crate::league::types::{ChampSelectState, GameflowState, LeagueStatus};

pub struct AppState {
    pub status: Mutex<LeagueStatus>,
    pub gameflow: Mutex<GameflowState>,
    pub champ_select: Mutex<ChampSelectState>,
    pub current_client: Mutex<Option<Arc<LcuClient>>>,
    pub opgg: OpggDataService,
    pub ddragon: DdragonService,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: Mutex::new(LeagueStatus::default()),
            gameflow: Mutex::new(GameflowState::default()),
            champ_select: Mutex::new(ChampSelectState::default()),
            current_client: Mutex::new(None),
            opgg: OpggDataService::new(),
            ddragon: DdragonService::new(),
        }
    }
}
