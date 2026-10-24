use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Normalized agent status.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum AgentStatus {
    Running,
    Idle,
    Failed,
    Offline,
    Unknown,
}

/// Normalized AI agent information.
#[derive(Serialize, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub host: String,
    pub model: Option<String>,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub last_heartbeat_unix: Option<u64>,
    pub token_usage: Option<u64>,
    pub context_usage: Option<f64>,
    pub recent_errors: Vec<String>,
    pub data_source: String,
    pub last_refresh_unix: u64,
}

/// OpenCode session response (subset of API response).
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct OpenCodeSession {
    id: String,
    #[serde(rename = "projectID")]
    project_id: Option<String>,
    agent: Option<String>,
    model: Option<OpenCodeModel>,
    cost: Option<f64>,
    tokens: Option<OpenCodeTokens>,
}

#[derive(serde::Deserialize, Debug)]
struct OpenCodeModel {
    id: String,
    #[serde(rename = "providerID")]
    provider_id: String,
}

#[derive(serde::Deserialize, Debug)]
struct OpenCodeTokens {
    input: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    cache: Option<OpenCodeCache>,
}

#[derive(serde::Deserialize, Debug)]
struct OpenCodeCache {
    read: Option<u64>,
    write: Option<u64>,
}

/// OpenCode adapter configuration.
#[derive(Clone, Debug)]
pub struct OpenCodeConfig {
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_secs: u64,
}

impl OpenCodeConfig {
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("OPENCODE_ENDPOINT").ok()?;
        let username = std::env::var("OPENCODE_USERNAME").ok();
        let password = std::env::var("OPENCODE_PASSWORD").ok();
        Some(Self {
            endpoint,
            username,
            password,
            timeout_secs: 5,
        })
    }
}

/// Fetch OpenCode sessions and convert to AgentInfo list.
pub async fn fetch_opencode_agents(config: &OpenCodeConfig) -> Vec<AgentInfo> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .unwrap_or_default();

    let url = format!("{}/api/session", config.endpoint.trim_end_matches('/'));

    let mut req = client.get(&url);
    if let (Some(user), Some(pass)) = (&config.username, &config.password) {
        req = req.basic_auth(user, Some(pass));
    }

    let response = match req.send().await {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            return vec![AgentInfo {
                id: "opencode-error".to_string(),
                name: "OpenCode".to_string(),
                provider: "opencode".to_string(),
                host: "unknown".to_string(),
                model: None,
                status: AgentStatus::Offline,
                current_task: None,
                last_heartbeat_unix: None,
                token_usage: None,
                context_usage: None,
                recent_errors: vec![format!("HTTP {}", resp.status())],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            }];
        }
        Err(e) => {
            return vec![AgentInfo {
                id: "opencode-error".to_string(),
                name: "OpenCode".to_string(),
                provider: "opencode".to_string(),
                host: "unknown".to_string(),
                model: None,
                status: AgentStatus::Offline,
                current_task: None,
                last_heartbeat_unix: None,
                token_usage: None,
                context_usage: None,
                recent_errors: vec![if e.is_timeout() {
                    "Connection timed out".to_string()
                } else {
                    "Connection failed".to_string()
                }],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            }];
        }
    };

    let sessions: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(_) => {
            return vec![AgentInfo {
                id: "opencode-error".to_string(),
                name: "OpenCode".to_string(),
                provider: "opencode".to_string(),
                host: "unknown".to_string(),
                model: None,
                status: AgentStatus::Unknown,
                current_task: None,
                last_heartbeat_unix: None,
                token_usage: None,
                context_usage: None,
                recent_errors: vec!["Invalid JSON response".to_string()],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            }];
        }
    };

    // Parse sessions from response
    let session_list = sessions
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    if session_list.is_empty() {
        return vec![AgentInfo {
            id: "opencode-none".to_string(),
            name: "OpenCode".to_string(),
            provider: "opencode".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Idle,
            current_task: None,
            last_heartbeat_unix: Some(now),
            token_usage: None,
            context_usage: None,
            recent_errors: vec![],
            data_source: "opencode-api".to_string(),
            last_refresh_unix: now,
        }];
    }

    session_list
        .iter()
        .filter_map(|s| {
            let session: OpenCodeSession = serde_json::from_value(s.clone()).ok()?;
            let model_name = session
                .model
                .as_ref()
                .map(|m| format!("{}/{}", m.provider_id, m.id));

            let total_tokens = session.tokens.as_ref().map(|t| {
                t.input.unwrap_or(0)
                    + t.output.unwrap_or(0)
                    + t.reasoning.unwrap_or(0)
                    + t.cache.as_ref().map(|c| c.read.unwrap_or(0) + c.write.unwrap_or(0)).unwrap_or(0)
            });

            let status = if session.agent.is_some() {
                AgentStatus::Running
            } else {
                AgentStatus::Idle
            };

            Some(AgentInfo {
                id: session.id.clone(),
                name: session.agent.clone().unwrap_or_else(|| "OpenCode Session".to_string()),
                provider: "opencode".to_string(),
                host: "local".to_string(),
                model: model_name,
                status,
                current_task: None, // OpenCode doesn't expose current task in session list
                last_heartbeat_unix: Some(now),
                token_usage: total_tokens,
                context_usage: None,
                recent_errors: vec![],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            })
        })
        .collect()
}

/// List all configured agents.
pub async fn list_agents() -> Vec<AgentInfo> {
    let mut agents = Vec::new();

    // OpenCode
    if let Some(config) = OpenCodeConfig::from_env() {
        agents.extend(fetch_opencode_agents(&config).await);
    } else {
        // No OpenCode configured - show as unavailable
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        agents.push(AgentInfo {
            id: "opencode-unconfigured".to_string(),
            name: "OpenCode".to_string(),
            provider: "opencode".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Unknown,
            current_task: None,
            last_heartbeat_unix: None,
            token_usage: None,
            context_usage: None,
            recent_errors: vec!["Not configured (set OPENCODE_ENDPOINT)".to_string()],
            data_source: "none".to_string(),
            last_refresh_unix: now,
        });
    }

    agents
}