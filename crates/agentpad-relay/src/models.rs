use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_id: Option<i64>,
    pub id: String,
    pub op_type: String,
    pub payload: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushOpsRequest {
    pub workspace_id: String,
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushOpsResponse {
    pub accepted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOpsQuery {
    pub after: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub workspace_id: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_op_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<Vec<Op>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
