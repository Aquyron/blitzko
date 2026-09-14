//! Pushes a custom Item Set to the League Client so it shows up as a
//! recommended build in the in-game shop.
//!
//! Confirmed live against a real client (2026-09): `PUT
//! /lol-item-sets/v1/item-sets/{summonerId}/sets` with a full `itemSets`
//! array replaces the player's custom sets, and one becomes usable in the
//! in-game shop immediately. This is **not** auto-purchasing — the LCU has
//! no access to a live match's shop/gold state, only the pre-game
//! recommendation list. See the project notes on why "Apply Build" for
//! items can only ever mean this.

use serde::Deserialize;
use serde_json::Value;

use super::lcu_client::LcuClient;

const TITLE_PREFIX: &str = "Blitzko";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBlockInput {
    pub block_type: String,
    pub item_ids: Vec<String>,
}

pub struct ItemBlock {
    pub block_type: String,
    pub item_ids: Vec<String>,
}

pub struct ItemSetSpec {
    pub champion_id: i64,
    pub title: String,
    pub blocks: Vec<ItemBlock>,
}

pub async fn apply_item_set(
    client: &LcuClient,
    summoner_id: i64,
    spec: ItemSetSpec,
) -> Result<(), String> {
    let current = client
        .item_sets(summoner_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut sets: Vec<Value> = current
        .get("itemSets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    sets.retain(|s| {
        let is_ours = s
            .get("title")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t.starts_with(TITLE_PREFIX));
        // The client itself seeds a blank "New Item Set" (empty blocks) the
        // first time the in-game shop's item-set editor is opened with none
        // present — confirmed live. It's useless (no items) but, being
        // untitled/unassociated, the shop was defaulting to it as the first
        // tab ahead of the real Blitzko build, so the player had to
        // manually switch every game. Drop any set with no blocks — it can
        // never contribute anything to the shop regardless of title.
        let is_blank = s
            .get("blocks")
            .and_then(|b| b.as_array())
            .is_none_or(|b| b.is_empty());
        !is_ours && !is_blank
    });

    let blocks: Vec<Value> = spec
        .blocks
        .iter()
        .filter(|b| !b.item_ids.is_empty())
        .map(|b| {
            serde_json::json!({
                "type": b.block_type,
                "items": b.item_ids.iter().map(|id| serde_json::json!({ "id": id, "count": 1 })).collect::<Vec<_>>(),
            })
        })
        .collect();

    sets.push(serde_json::json!({
        "title": spec.title,
        "type": "custom",
        "map": "any",
        "mode": "any",
        "preferredItemSlots": [],
        "associatedChampions": [spec.champion_id],
        "associatedMaps": [],
        "blocks": blocks,
    }));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let body = serde_json::json!({
        "accountId": summoner_id,
        "itemSets": sets,
        "timestamp": now,
    });

    client
        .put_item_sets(summoner_id, &body)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
