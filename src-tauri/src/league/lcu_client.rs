use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use super::types::LcuConnectionInfo;

const WS_RETRY_DELAY: Duration = Duration::from_secs(3);
const POLL_FALLBACK_INTERVAL: Duration = Duration::from_secs(10);

/// One LCU push event: the API path it came from and its JSON payload.
#[derive(Debug, Clone)]
pub struct LcuWsEvent {
    pub uri: String,
    pub data: Value,
}

/// Abort handles for the two background loops `watch_events` starts, so a
/// superseded `LcuClient` can be fully torn down instead of leaking retry
/// loops that spin forever against a dead port.
pub struct EventWatchHandles {
    ws: tokio::task::AbortHandle,
    poll: tokio::task::AbortHandle,
}

impl EventWatchHandles {
    pub fn abort(&self) {
        self.ws.abort();
        self.poll.abort();
    }
}

#[derive(Clone)]
pub struct LcuClient {
    http: reqwest::Client,
    base_url: String,
    auth_header: String,
    ws_url: String,
}

impl LcuClient {
    pub fn new(info: LcuConnectionInfo) -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build LCU http client");
        let creds = general_purpose::STANDARD.encode(format!("riot:{}", info.password));
        Self {
            http,
            base_url: format!("{}://127.0.0.1:{}", info.protocol, info.port),
            auth_header: format!("Basic {creds}"),
            ws_url: format!("wss://127.0.0.1:{}/", info.port),
        }
    }

    pub async fn get(&self, path: &str) -> Result<Value, reqwest::Error> {
        self.http
            .get(format!("{}{}", self.base_url, path))
            .header("Authorization", &self.auth_header)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, reqwest::Error> {
        self.http
            .post(format!("{}{}", self.base_url, path))
            .header("Authorization", &self.auth_header)
            .json(body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    /// Not all LCU PUT endpoints return a JSON body (item-sets returns an
    /// empty one, confirmed live — trying to decode it broke `apply_item_set`
    /// with a real "error decoding response body" failure) — only the
    /// status is checked, the body (if any) is ignored.
    pub async fn put(&self, path: &str, body: &Value) -> Result<(), reqwest::Error> {
        self.http
            .put(format!("{}{}", self.base_url, path))
            .header("Authorization", &self.auth_header)
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn patch(&self, path: &str, body: &Value) -> Result<(), reqwest::Error> {
        self.http
            .patch(format!("{}{}", self.base_url, path))
            .header("Authorization", &self.auth_header)
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn current_summoner(&self) -> Result<Value, reqwest::Error> {
        self.get("/lol-summoner/v1/current-summoner").await
    }

    pub async fn game_version(&self) -> Option<String> {
        self.get("/lol-patch/v1/game-version")
            .await
            .ok()?
            .as_str()
            .map(|s| s.to_string())
    }

    pub async fn gameflow_phase(&self) -> Result<String, reqwest::Error> {
        let value = self.get("/lol-gameflow/v1/gameflow-phase").await?;
        Ok(value.as_str().unwrap_or("Unknown").to_string())
    }

    /// Full gameflow session, used to read the queue's game mode
    /// (ARAM/URF/etc.) once champ select starts. **Unverified live** — no
    /// non-Ranked/Flex champ select was available to test against this
    /// session; confirm `gameData.queue.gameMode` against a real client.
    pub async fn gameflow_session(&self) -> Result<Value, reqwest::Error> {
        self.get("/lol-gameflow/v1/session").await
    }

    pub async fn rune_pages(&self) -> Result<Value, reqwest::Error> {
        self.get("/lol-perks/v1/pages").await
    }

    pub async fn rune_inventory(&self) -> Result<Value, reqwest::Error> {
        self.get("/lol-perks/v1/inventory").await
    }

    pub async fn item_sets(&self, summoner_id: i64) -> Result<Value, reqwest::Error> {
        self.get(&format!("/lol-item-sets/v1/item-sets/{summoner_id}/sets"))
            .await
    }

    pub async fn put_item_sets(&self, summoner_id: i64, body: &Value) -> Result<(), reqwest::Error> {
        self.put(&format!("/lol-item-sets/v1/item-sets/{summoner_id}/sets"), body)
            .await
    }

    /// Sets summoner spells during champ select. **Unverified live** — no
    /// champ select was available to test against this session. Community
    /// knowledge only; confirm the path/body shape against a real client
    /// before trusting this in a real pick/ban.
    pub async fn set_champ_select_spells(
        &self,
        spell1_id: i64,
        spell2_id: i64,
    ) -> Result<(), reqwest::Error> {
        self.patch(
            "/lol-champ-select/v1/session/my-selection",
            &serde_json::json!({ "spell1Id": spell1_id, "spell2Id": spell2_id }),
        )
        .await
    }

    /// Submits a ban or pick for one champ-select action — this is what
    /// drives one-click "auto ban"/"auto pick" from the suggestions panel.
    /// `completed: true` both selects the champion and locks it in
    /// immediately, matching how the LCU's own grid behaves when you
    /// double-click a champion portrait. **Unverified live** — no real ban
    /// or pick phase was available to test against yet; confirm the
    /// path/body shape (community-documented, not Riot-published) against
    /// a real champ select before trusting this in a real game.
    pub async fn set_champ_select_action(
        &self,
        action_id: i64,
        champion_id: i64,
    ) -> Result<(), reqwest::Error> {
        self.patch(
            &format!("/lol-champ-select/v1/session/actions/{action_id}"),
            &serde_json::json!({ "championId": champion_id, "completed": true }),
        )
        .await
    }

    /// Full champ-select session snapshot. Unverified field shape (see
    /// `league::champ_select` for the parsing caveats) — fetched fresh
    /// whenever we detect entry into the ChampSelect phase, since the push
    /// stream alone might start after the session already exists.
    pub async fn champ_select_session(&self) -> Result<Value, reqwest::Error> {
        self.get("/lol-champ-select/v1/session").await
    }

    /// Streams every LCU push event over the WAMP-style event WebSocket, plus
    /// a slow gameflow-phase poll as a safety net in case the socket framing
    /// doesn't match on a given client version (community-documented, not
    /// officially published by Riot).
    ///
    /// Returns abort handles for both background loops — this client tears
    /// down and gets replaced whenever the lockfile changes (client
    /// restart, reconnect), and without aborting the old loops they'd keep
    /// retrying against a dead port forever.
    pub fn watch_events(self: Arc<Self>, tx: mpsc::UnboundedSender<LcuWsEvent>) -> EventWatchHandles {
        let ws_handle = {
            let client = self.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                loop {
                    if let Err(err) = client.subscribe_events_once(&tx).await {
                        eprintln!("[blitzko] LCU event socket dropped: {err}");
                    }
                    tokio::time::sleep(WS_RETRY_DELAY).await;
                }
            })
            .abort_handle()
        };

        let poll_handle = tokio::spawn(async move {
            loop {
                if let Ok(phase) = self.gameflow_phase().await {
                    let _ = tx.send(LcuWsEvent {
                        uri: "/lol-gameflow/v1/gameflow-phase".to_string(),
                        data: Value::String(phase),
                    });
                }
                tokio::time::sleep(POLL_FALLBACK_INTERVAL).await;
            }
        })
        .abort_handle();

        EventWatchHandles {
            ws: ws_handle,
            poll: poll_handle,
        }
    }

    async fn subscribe_events_once(
        &self,
        tx: &mpsc::UnboundedSender<LcuWsEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut request = self.ws_url.as_str().into_client_request()?;
        request
            .headers_mut()
            .insert("Authorization", HeaderValue::from_str(&self.auth_header)?);

        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        let (ws_stream, _) = tokio_tungstenite::connect_async_tls_with_config(
            request,
            None,
            false,
            Some(Connector::NativeTls(connector)),
        )
        .await?;

        let (mut write, mut read) = ws_stream.split();
        // WAMP CALL frame: subscribe to every JSON-API event.
        write
            .send(Message::Text(r#"[5,"OnJsonApiEvent"]"#.to_string()))
            .await?;

        while let Some(msg) = read.next().await {
            if let Message::Text(text) = msg? {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    if let Some(event) = extract_ws_event(&value) {
                        let _ = tx.send(event);
                    }
                }
            }
        }
        Ok(())
    }
}

/// WAMP event frames look like `[8, "OnJsonApiEvent", {uri, eventType, data}]`.
fn extract_ws_event(value: &Value) -> Option<LcuWsEvent> {
    let arr = value.as_array()?;
    let payload = arr.get(2)?;
    let uri = payload.get("uri")?.as_str()?.to_string();
    let data = payload.get("data")?.clone();
    Some(LcuWsEvent { uri, data })
}
