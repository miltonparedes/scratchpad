use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::{
    Json,
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use tokio::sync::RwLock;

use crate::AppState;
use crate::models::{GetOpsQuery, Op, PushOpsRequest, PushOpsResponse, Snapshot, WsMessage};

pub async fn health() -> &'static str {
    "ok"
}

fn is_authorized(headers: &HeaderMap, token: Option<&str>) -> bool {
    let Some(token) = token else {
        return true;
    };

    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let scratchpad_token = headers
        .get("x-scratchpad-token")
        .and_then(|value| value.to_str().ok());

    bearer_token == Some(token) || scratchpad_token == Some(token)
}

fn unauthorized() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "unauthorized".to_string())
}

pub async fn push_ops(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PushOpsRequest>,
) -> Result<Json<PushOpsResponse>, (StatusCode, String)> {
    if !is_authorized(&headers, state.server_token.as_deref()) {
        return Err(unauthorized());
    }

    let mut accepted = 0;

    for op in &req.ops {
        match state.db.push_op(&req.workspace_id, op) {
            Ok(_) => {
                accepted += 1;
                let msg = WsMessage {
                    msg_type: "op".to_string(),
                    workspace_id: Some(req.workspace_id.clone()),
                    ops: Some(vec![op.clone()]),
                    error: None,
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = state.tx.send(json);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to push op: {e}");
            }
        }
    }

    Ok(Json(PushOpsResponse { accepted }))
}

pub async fn get_ops(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<GetOpsQuery>,
) -> Result<Json<Vec<Op>>, (StatusCode, String)> {
    if !is_authorized(&headers, state.server_token.as_deref()) {
        return Err(unauthorized());
    }

    match state.db.get_ops(&workspace_id, query.after) {
        Ok(ops) => Ok(Json(ops)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn get_snapshot(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    if !is_authorized(&headers, state.server_token.as_deref()) {
        return Err(unauthorized());
    }

    match state.db.get_snapshot(&workspace_id) {
        Ok(Some(snapshot)) => Ok(Json(snapshot).into_response()),
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn save_snapshot(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    Json(mut snapshot): Json<Snapshot>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !is_authorized(&headers, state.server_token.as_deref()) {
        return Err(unauthorized());
    }

    snapshot.workspace_id = workspace_id;
    match state.db.save_snapshot(&snapshot) {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn websocket_handler(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    if !is_authorized(&headers, state.server_token.as_deref()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    let subscribed_workspaces = Arc::new(RwLock::new(HashSet::new()));

    let send_task = {
        let subscribed_workspaces = Arc::clone(&subscribed_workspaces);
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let should_send = match serde_json::from_str::<WsMessage>(&msg) {
                    Ok(ws_msg) => {
                        if let Some(id) = ws_msg.workspace_id.as_ref() {
                            subscribed_workspaces.read().await.contains(id)
                        } else {
                            false
                        }
                    }
                    Err(_) => false,
                };

                if should_send && sender.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        })
    };

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg
            && let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text)
        {
            match ws_msg.msg_type.as_str() {
                "subscribe" => {
                    if let Some(workspace_id) = ws_msg.workspace_id {
                        subscribed_workspaces.write().await.insert(workspace_id);
                    }
                }
                "unsubscribe" => {
                    if let Some(workspace_id) = ws_msg.workspace_id {
                        subscribed_workspaces.write().await.remove(&workspace_id);
                    }
                }
                "push" => {
                    if let (Some(workspace_id), Some(ops)) = (ws_msg.workspace_id, ws_msg.ops) {
                        for op in ops {
                            let _ = state.db.push_op(&workspace_id, &op);
                            let broadcast_msg = WsMessage {
                                msg_type: "op".to_string(),
                                workspace_id: Some(workspace_id.clone()),
                                ops: Some(vec![op]),
                                error: None,
                            };
                            if let Ok(json) = serde_json::to_string(&broadcast_msg) {
                                let _ = state.tx.send(json);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    send_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn authorization_allows_requests_when_no_server_token_is_configured() {
        assert!(is_authorized(&HeaderMap::new(), None));
    }

    #[test]
    fn authorization_accepts_bearer_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret"),
        );

        assert!(is_authorized(&headers, Some("secret")));
    }

    #[test]
    fn authorization_rejects_incorrect_tokens() {
        let mut headers = HeaderMap::new();
        headers.insert("x-scratchpad-token", HeaderValue::from_static("wrong"));

        assert!(!is_authorized(&headers, Some("secret")));
    }
}
