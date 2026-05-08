use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct ClientRequest {
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientResponse {
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl ClientResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }
}

impl ClientRequest {
    pub fn initialize(id: u64) -> Self {
        Self {
            id,
            method: "initialize".to_string(),
            params: json!({
                "clientInfo": {
                    "name": "symphony",
                    "title": "Symphony",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        }
    }

    pub fn thread_start(
        id: u64,
        cwd: &str,
        approval_policy: &str,
        sandbox: &str,
        dynamic_tools: Vec<Value>,
    ) -> Self {
        Self {
            id,
            method: "thread/start".to_string(),
            params: json!({
                "cwd": cwd,
                "approvalPolicy": approval_policy,
                "sandbox": sandbox,
                "dynamicTools": dynamic_tools,
                "threadSource": "user",
                "sessionStartSource": "startup",
                "ephemeral": false
            }),
        }
    }

    pub fn turn_start(
        id: u64,
        thread_id: &str,
        cwd: &str,
        prompt: &str,
        approval_policy: &str,
        sandbox_policy: &str,
    ) -> Self {
        Self {
            id,
            method: "turn/start".to_string(),
            params: json!({
                "threadId": thread_id,
                "cwd": cwd,
                "approvalPolicy": approval_policy,
                "sandboxPolicy": sandbox_policy_value(sandbox_policy),
                "input": [
                    { "type": "text", "text": prompt }
                ]
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerMessage {
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: Option<i64>,
    pub message: String,
    #[serde(default)]
    pub data: Value,
}

pub fn extract_thread_id(result: &Value) -> Option<String> {
    result
        .pointer("/thread/id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub fn extract_turn_id(result: &Value) -> Option<String> {
    result
        .pointer("/turn/id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub fn github_issue_tool_spec() -> Value {
    json!({
        "name": "github_issue",
        "description": "Read or update the current GitHub issue through Symphony's configured tracker credentials.",
        "inputSchema": {
            "type": "object",
            "required": ["action", "issue_id"],
            "properties": {
                "action": { "type": "string", "enum": ["comment", "set_state", "close", "link_pr"] },
                "issue_id": { "type": "string" },
                "body": { "type": "string" },
                "state": { "type": "string" },
                "pr_number": { "type": "integer", "minimum": 1 }
            },
            "additionalProperties": false
        }
    })
}

fn sandbox_policy_value(policy: &str) -> Value {
    match policy {
        "danger-full-access" | "dangerFullAccess" => json!({ "type": "dangerFullAccess" }),
        "read-only" | "readOnly" => json!({ "type": "readOnly" }),
        "workspace-write" | "workspaceWrite" => json!({ "type": "workspaceWrite" }),
        "external-sandbox" | "externalSandbox" => json!({ "type": "externalSandbox" }),
        other => json!(other),
    }
}

#[cfg(test)]
mod tests {
    use super::ClientRequest;

    #[test]
    fn thread_start_uses_current_codex_session_start_source() {
        let request =
            ClientRequest::thread_start(1, "/tmp/work", "never", "workspace-write", vec![]);

        assert_eq!(request.method, "thread/start");
        assert_eq!(request.params["sessionStartSource"], "startup");
    }

    #[test]
    fn turn_start_maps_sandbox_policy_for_current_codex_schema() {
        let request = ClientRequest::turn_start(
            2,
            "thread-1",
            "/tmp/work",
            "finish",
            "never",
            "danger-full-access",
        );

        assert_eq!(request.method, "turn/start");
        assert_eq!(request.params["sandboxPolicy"]["type"], "dangerFullAccess");
    }
}
