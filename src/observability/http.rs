use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde_json::json;
use tokio::sync::RwLock;

use crate::events::OrchestratorState;
use crate::status::StatusSnapshot;
use crate::{Result, SymphonyError};

#[derive(Clone)]
pub struct HttpState {
    pub orchestrator_state: Arc<RwLock<OrchestratorState>>,
}

pub async fn serve(bind: &str, state: Arc<RwLock<OrchestratorState>>) -> Result<()> {
    let addr: SocketAddr = bind.parse().map_err(|error| {
        SymphonyError::Http(format!("invalid http bind address {bind:?}: {error}"))
    })?;
    let app_state = HttpState {
        orchestrator_state: state,
    };
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/health", get(health))
        .route("/status", get(api_state))
        .route("/api/v1/state", get(api_state))
        .route("/api/v1/refresh", post(refresh))
        .route("/api/v1/{*identifier}", get(issue_detail))
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "http status server listening");
    axum::serve(listener, app)
        .await
        .map_err(|error| SymphonyError::Http(error.to_string()))
}

async fn dashboard(State(state): State<HttpState>) -> Html<String> {
    let guard = state.orchestrator_state.read().await;
    let snapshot = StatusSnapshot::from_state(&guard);
    Html(format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Symphony</title></head>
<body>
<h1>Symphony</h1>
<p>Running: {} | Retrying: {} | Claimed: {} | Completed: {}</p>
<pre>{}</pre>
</body></html>"#,
        snapshot.counts.running,
        snapshot.counts.retrying,
        snapshot.counts.claimed,
        snapshot.counts.completed,
        serde_json::to_string_pretty(&snapshot).unwrap_or_default()
    ))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true, "service": "symphony" }))
}

async fn api_state(State(state): State<HttpState>) -> Json<StatusSnapshot> {
    let guard = state.orchestrator_state.read().await;
    Json(StatusSnapshot::from_state(&guard))
}

async fn refresh() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "queued": true,
            "coalesced": false,
            "requested_at": Utc::now(),
            "operations": ["poll", "reconcile"]
        })),
    )
}

async fn issue_detail(
    Path(identifier): Path<String>,
    State(state): State<HttpState>,
) -> impl IntoResponse {
    let decoded = urlencoding::decode(&identifier)
        .map(|value| value.to_string())
        .unwrap_or(identifier);
    let guard = state.orchestrator_state.read().await;
    if let Some((issue_id, live)) = guard
        .running
        .iter()
        .find(|(_, live)| live.identifier == decoded || live.issue.id == decoded)
    {
        return (
            StatusCode::OK,
            Json(json!({
                "issue_identifier": live.identifier,
                "issue_id": issue_id,
                "status": "running",
                "attempts": {
                    "current_retry_attempt": live.retry_attempt
                },
                "running": {
                    "session_id": live.session_id,
                    "turn_count": live.turn_count,
                    "state": live.issue.state,
                    "started_at": live.started_at,
                    "last_event": live.last_codex_event,
                    "last_event_at": live.last_codex_timestamp,
                    "tokens": {
                        "input_tokens": live.codex_input_tokens,
                        "output_tokens": live.codex_output_tokens,
                        "total_tokens": live.codex_total_tokens
                    }
                },
                "retry": null,
                "recent_events": [],
                "last_error": null,
                "tracked": {}
            })),
        );
    }
    if let Some(retry) = guard
        .retry_attempts
        .values()
        .find(|retry| retry.identifier == decoded || retry.issue_id == decoded)
    {
        return (
            StatusCode::OK,
            Json(json!({
                "issue_identifier": retry.identifier,
                "issue_id": retry.issue_id,
                "status": "retrying",
                "running": null,
                "retry": retry,
                "recent_events": [],
                "last_error": retry.error,
                "tracked": {}
            })),
        );
    }
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "issue_not_found",
                "message": format!("issue {decoded:?} is not present in current runtime state")
            }
        })),
    )
}
