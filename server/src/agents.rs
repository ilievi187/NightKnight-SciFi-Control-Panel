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
    pub last_activity_unix: Option<u64>,
    pub observed_at_unix: u64,
    pub token_usage: Option<u64>,
    pub context_usage: Option<f64>,
    pub cost_usd: Option<f64>,
    pub recent_errors: Vec<String>,
    pub data_source: String,
    pub last_refresh_unix: u64,
}

/// OpenCode session time information.
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct OpenCodeTime {
    created: Option<u64>,
    updated: Option<u64>,
    idle: Option<u64>,
    viewed: Option<u64>,
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
    outcome: Option<String>,
    time: Option<OpenCodeTime>,
    title: Option<String>,
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

/// Determine session status from OpenCode session data.
/// Uses outcome and time fields, NOT just presence of agent field.
fn determine_status(session: &OpenCodeSession) -> AgentStatus {
    // Check outcome first
    if let Some(outcome) = &session.outcome {
        match outcome.as_str() {
            "succeeded" => return AgentStatus::Idle,
            "failed" => return AgentStatus::Failed,
            _ => {}
        }
    }

    // Check time fields for activity
    if let Some(time) = &session.time {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // If updated recently (within 60 seconds), consider running
        if let Some(updated) = time.updated {
            let age_ms = now_ms.saturating_sub(updated);
            if age_ms < 60_000 {
                return AgentStatus::Running;
            }
        }

        // If idle timestamp exists and is recent, consider idle
        if let Some(idle) = time.idle {
            let age_ms = now_ms.saturating_sub(idle);
            if age_ms < 300_000 {
                return AgentStatus::Idle;
            }
        }
    }

    AgentStatus::Unknown
}

/// Get last activity timestamp from session.
fn last_activity(session: &OpenCodeSession) -> Option<u64> {
    session.time.as_ref().and_then(|t| {
        // Prefer updated, then idle, then created
        t.updated.or(t.idle).or(t.created).map(|ms| ms / 1000)
    })
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
                last_activity_unix: None,
                observed_at_unix: now,
                token_usage: None,
                context_usage: None,
                cost_usd: None,
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
                last_activity_unix: None,
                observed_at_unix: now,
                token_usage: None,
                context_usage: None,
                cost_usd: None,
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
                last_activity_unix: None,
                observed_at_unix: now,
                token_usage: None,
                context_usage: None,
                cost_usd: None,
                recent_errors: vec!["Invalid JSON response".to_string()],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            }];
        }
    };

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
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
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
                    + t.cache
                        .as_ref()
                        .map(|c| c.read.unwrap_or(0) + c.write.unwrap_or(0))
                        .unwrap_or(0)
            });

            let status = determine_status(&session);

            // Use session title as current task if available
            let current_task = session.title.clone();

            Some(AgentInfo {
                id: session.id.clone(),
                name: session
                    .agent
                    .clone()
                    .unwrap_or_else(|| "OpenCode Session".to_string()),
                provider: "opencode".to_string(),
                host: "local".to_string(),
                model: model_name,
                status,
                current_task,
                last_activity_unix: last_activity(&session),
                observed_at_unix: now,
                token_usage: total_tokens,
                context_usage: None,
                cost_usd: session.cost,
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
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
            recent_errors: vec!["Not configured (set OPENCODE_ENDPOINT)".to_string()],
            data_source: "none".to_string(),
            last_refresh_unix: now,
        });
    }

    agents
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_status_succeeded_is_idle() {
        let session = OpenCodeSession {
            id: "test".to_string(),
            project_id: None,
            agent: Some("build".to_string()),
            model: None,
            cost: None,
            tokens: None,
            outcome: Some("succeeded".to_string()),
            time: None,
            title: None,
        };
        assert_eq!(determine_status(&session), AgentStatus::Idle);
    }

    #[test]
    fn determine_status_failed_is_failed() {
        let session = OpenCodeSession {
            id: "test".to_string(),
            project_id: None,
            agent: Some("build".to_string()),
            model: None,
            cost: None,
            tokens: None,
            outcome: Some("failed".to_string()),
            time: None,
            title: None,
        };
        assert_eq!(determine_status(&session), AgentStatus::Failed);
    }

    #[test]
    fn determine_status_no_outcome_unknown() {
        let session = OpenCodeSession {
            id: "test".to_string(),
            project_id: None,
            agent: Some("build".to_string()),
            model: None,
            cost: None,
            tokens: None,
            outcome: None,
            time: None,
            title: None,
        };
        assert_eq!(determine_status(&session), AgentStatus::Unknown);
    }

    #[test]
    fn last_activity_prefers_updated() {
        let session = OpenCodeSession {
            id: "test".to_string(),
            project_id: None,
            agent: None,
            model: None,
            cost: None,
            tokens: None,
            outcome: None,
            time: Some(OpenCodeTime {
                created: Some(1000),
                updated: Some(3000),
                idle: Some(2000),
                viewed: None,
            }),
            title: None,
        };
        assert_eq!(last_activity(&session), Some(3));
    }

    #[test]
    fn last_activity_falls_back_to_created() {
        let session = OpenCodeSession {
            id: "test".to_string(),
            project_id: None,
            agent: None,
            model: None,
            cost: None,
            tokens: None,
            outcome: None,
            time: Some(OpenCodeTime {
                created: Some(1000),
                updated: None,
                idle: None,
                viewed: None,
            }),
            title: None,
        };
        assert_eq!(last_activity(&session), Some(1));
    }

    #[test]
    fn open_code_config_from_env_none() {
        // Without env var set, should return None
        std::env::remove_var("OPENCODE_ENDPOINT");
        assert!(OpenCodeConfig::from_env().is_none());
    }

    #[test]
    fn agent_status_serializes() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn agent_info_serializes() {
        let info = AgentInfo {
            id: "test-1".to_string(),
            name: "Test Agent".to_string(),
            provider: "opencode".to_string(),
            host: "local".to_string(),
            model: Some("opencode-go/kimi-k3".to_string()),
            status: AgentStatus::Idle,
            current_task: Some("NightKnight development".to_string()),
            last_activity_unix: Some(1792846204),
            observed_at_unix: 1792846204,
            token_usage: Some(6000000),
            context_usage: None,
            cost_usd: Some(3.13),
            recent_errors: vec![],
            data_source: "opencode-api".to_string(),
            last_refresh_unix: 1792846204,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"cost_usd\":3.13"));
        assert!(json.contains("\"token_usage\":6000000"));
    }
}
