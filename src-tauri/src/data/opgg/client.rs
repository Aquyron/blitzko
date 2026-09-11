use std::fmt;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;

use super::text_format::{self, ParseError};

const MCP_URL: &str = "https://mcp-api.op.gg/mcp";

#[derive(Debug)]
pub enum OpggError {
    Http(reqwest::Error),
    NoSession,
    BadResponse(String),
    Parse(ParseError),
    ToolError(String),
}

impl fmt::Display for OpggError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpggError::Http(e) => write!(f, "OP.GG request failed: {e}"),
            OpggError::NoSession => write!(f, "OP.GG MCP session was not established"),
            OpggError::BadResponse(s) => write!(f, "unexpected OP.GG response shape: {s}"),
            OpggError::Parse(e) => write!(f, "failed to parse OP.GG response: {e}"),
            OpggError::ToolError(s) => write!(f, "OP.GG tool call failed: {s}"),
        }
    }
}

impl std::error::Error for OpggError {}

impl From<reqwest::Error> for OpggError {
    fn from(e: reqwest::Error) -> Self {
        OpggError::Http(e)
    }
}

/// Thin client for OP.GG's officially maintained MCP server
/// (https://github.com/opgginc/opgg-mcp), used as our stats/build data
/// provider instead of scraping u.gg (see project notes on why: u.gg's
/// robots.txt blocks AI crawlers and its ToS page has active bot
/// protection, while this is OP.GG's own sanctioned integration channel).
pub struct OpggClient {
    http: reqwest::Client,
    session_id: Mutex<Option<String>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl OpggClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("failed to build OP.GG http client"),
            session_id: Mutex::new(None),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    fn next_request_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    async fn open_session(&self) -> Result<String, OpggError> {
        let resp = self
            .http
            .post(MCP_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": self.next_request_id(),
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "blitzko", "version": "0.1.0" }
                }
            }))
            .send()
            .await?
            .error_for_status()?;

        let session_id = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or(OpggError::NoSession)?;

        Ok(session_id)
    }

    async fn session(&self) -> Result<String, OpggError> {
        {
            let guard = self.session_id.lock().await;
            if let Some(id) = guard.as_ref() {
                return Ok(id.clone());
            }
        }
        let id = self.open_session().await?;
        *self.session_id.lock().await = Some(id.clone());
        Ok(id)
    }

    async fn call_once(
        &self,
        session_id: &str,
        name: &str,
        arguments: &Value,
    ) -> Result<Value, OpggError> {
        let resp: Value = self
            .http
            .post(MCP_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Session-Id", session_id)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": self.next_request_id(),
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments }
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(err) = resp.get("error") {
            return Err(OpggError::ToolError(err.to_string()));
        }

        let text = resp
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpggError::BadResponse(resp.to_string()))?;

        text_format::parse_response(text).map_err(OpggError::Parse)
    }

    /// Calls an MCP tool by name. Retries once with a fresh session if the
    /// first attempt fails — the session id isn't documented as durable, so
    /// treat any failure as a possible expiry rather than a hard error.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, OpggError> {
        let session_id = self.session().await?;
        match self.call_once(&session_id, name, &arguments).await {
            Ok(v) => Ok(v),
            Err(_) => {
                *self.session_id.lock().await = None;
                let fresh_session = self.session().await?;
                self.call_once(&fresh_session, name, &arguments).await
            }
        }
    }
}
