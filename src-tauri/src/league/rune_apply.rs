//! Applies a rune page to the League Client's rune inventory.
//!
//! Confirmed live against a real client (2026-09): creating a page with
//! `POST /lol-perks/v1/pages` and `"current": true` in the body activates
//! it immediately (no separate call needed); updating one via
//! `PUT /lol-perks/v1/pages/{id}` with `"current": true` does the same.
//! We reuse one page across calls instead of creating a new one every time
//! (avoids cluttering the player's rune page list and staying within
//! `canAddCustomPage` limits) — identified by the `PAGE_PREFIX` prefix
//! rather than an exact name, since the name itself changes per champion
//! (e.g. "Blitzko: Ahri Mid").

use serde_json::Value;

use super::lcu_client::LcuClient;

const PAGE_PREFIX: &str = "Blitzko";

pub struct RuneSpec {
    pub page_name: String,
    pub primary_style_id: i64,
    pub sub_style_id: i64,
    pub selected_perk_ids: Vec<i64>,
}

pub async fn apply_runes(client: &LcuClient, spec: RuneSpec) -> Result<(), String> {
    let pages = client.rune_pages().await.map_err(|e| e.to_string())?;
    let existing_id = pages
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|p| {
                p.get("name")
                    .and_then(|n| n.as_str())
                    .is_some_and(|n| n.starts_with(PAGE_PREFIX))
            })
        })
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_i64());

    let mut body = serde_json::json!({
        "name": spec.page_name,
        "primaryStyleId": spec.primary_style_id,
        "subStyleId": spec.sub_style_id,
        "selectedPerkIds": spec.selected_perk_ids,
        "current": true,
    });

    if let Some(id) = existing_id {
        body["id"] = Value::from(id);
        client
            .put(&format!("/lol-perks/v1/pages/{id}"), &body)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let inventory = client.rune_inventory().await.map_err(|e| e.to_string())?;
        let can_add = inventory
            .get("canAddCustomPage")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !can_add {
            return Err(
                "No free rune page slot — delete a custom page in the client and try again."
                    .to_string(),
            );
        }
        client
            .post("/lol-perks/v1/pages", &body)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
