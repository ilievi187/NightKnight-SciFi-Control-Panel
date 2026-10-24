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
    pub tokens_per_second: Option<f64>,
    pub cost_usd: Option<f64>,
    pub recent_errors: Vec<String>,
    pub data_source: String,
    pub last_refresh_unix: u64,
}

// ============================================================
// OpenCode Adapter
// ============================================================

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

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct OpenCodeTime {
    created: Option<u64>,
    updated: Option<u64>,
    idle: Option<u64>,
    viewed: Option<u64>,
}

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

fn determine_opencode_status(session: &OpenCodeSession) -> AgentStatus {
    if let Some(outcome) = &session.outcome {
        match outcome.as_str() {
            "succeeded" => return AgentStatus::Idle,
            "failed" => return AgentStatus::Failed,
            _ => {}
        }
    }

    if let Some(time) = &session.time {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        if let Some(updated) = time.updated {
            let age_ms = now_ms.saturating_sub(updated);
            if age_ms < 60_000 {
                return AgentStatus::Running;
            }
        }

        if let Some(idle) = time.idle {
            let age_ms = now_ms.saturating_sub(idle);
            if age_ms < 300_000 {
                return AgentStatus::Idle;
            }
        }
    }

    AgentStatus::Unknown
}

fn opencode_last_activity(session: &OpenCodeSession) -> Option<u64> {
    session
        .time
        .as_ref()
        .and_then(|t| t.updated.or(t.idle).or(t.created).map(|ms| ms / 1000))
}

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
            return vec![error_agent(
                "opencode",
                "OpenCode",
                &format!("HTTP {}", resp.status()),
                now,
            )];
        }
        Err(e) => {
            return vec![error_agent(
                "opencode",
                "OpenCode",
                if e.is_timeout() {
                    "Connection timed out"
                } else {
                    "Connection failed"
                },
                now,
            )];
        }
    };

    let sessions: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(_) => {
            return vec![error_agent(
                "opencode",
                "OpenCode",
                "Invalid JSON response",
                now,
            )];
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
            tokens_per_second: None,
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

            // Calculate average tokens per second from session duration
            let tps = if let (Some(tokens), Some(time)) = (total_tokens, &session.time) {
                if let (Some(created), Some(updated)) = (time.created, time.updated) {
                    let duration_ms = updated.saturating_sub(created);
                    if duration_ms > 0 {
                        Some(tokens as f64 * 1000.0 / duration_ms as f64)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let status = determine_opencode_status(&session);
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
                last_activity_unix: opencode_last_activity(&session),
                observed_at_unix: now,
                token_usage: total_tokens,
                context_usage: None,
                tokens_per_second: tps,
                cost_usd: session.cost,
                recent_errors: vec![],
                data_source: "opencode-api".to_string(),
                last_refresh_unix: now,
            })
        })
        .collect()
}

// ============================================================
// Hermes Agent Adapter
// ============================================================

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct HermesSession {
    id: String,
    source: Option<String>,
    user_id: Option<String>,
    model: Option<String>,
    title: Option<String>,
    started_at: Option<f64>,
    ended_at: Option<f64>,
    end_reason: Option<String>,
    message_count: Option<u64>,
    tool_call_count: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    estimated_cost_usd: Option<f64>,
    actual_cost_usd: Option<f64>,
    api_call_count: Option<u64>,
    parent_session_id: Option<String>,
    last_active: Option<f64>,
    preview: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HermesConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

impl HermesConfig {
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("HERMES_ENDPOINT").ok()?;
        let api_key = std::env::var("HERMES_API_KEY").ok();
        Some(Self {
            endpoint,
            api_key,
            timeout_secs: 5,
        })
    }
}

fn determine_hermes_status(session: &HermesSession) -> AgentStatus {
    // If ended_at is set, session is completed
    if session.ended_at.is_some() {
        if session.end_reason.as_deref() == Some("error") {
            return AgentStatus::Failed;
        }
        return AgentStatus::Idle;
    }

    // Check last_active recency
    if let Some(last_active) = session.last_active {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let age_secs = now - last_active;
        if age_secs < 60.0 {
            return AgentStatus::Running;
        }
        if age_secs < 300.0 {
            return AgentStatus::Idle;
        }
    }

    AgentStatus::Unknown
}

fn hermes_last_activity(session: &HermesSession) -> Option<u64> {
    session.last_active.map(|ts| ts as u64)
}

fn hermes_token_usage(session: &HermesSession) -> Option<u64> {
    let input = session.input_tokens.unwrap_or(0);
    let output = session.output_tokens.unwrap_or(0);
    let reasoning = session.reasoning_tokens.unwrap_or(0);
    let cache_read = session.cache_read_tokens.unwrap_or(0);
    let cache_write = session.cache_write_tokens.unwrap_or(0);
    let total = input + output + reasoning + cache_read + cache_write;
    if total > 0 {
        Some(total)
    } else {
        None
    }
}

pub async fn fetch_hermes_agents(config: &HermesConfig) -> Vec<AgentInfo> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .unwrap_or_default();

    let url = format!("{}/api/sessions", config.endpoint.trim_end_matches('/'));

    let mut req = client.get(&url);
    if let Some(key) = &config.api_key {
        req = req.bearer_auth(key);
    }

    let response = match req.send().await {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            return vec![error_agent(
                "hermes",
                "Hermes Agent",
                &format!("HTTP {}", resp.status()),
                now,
            )];
        }
        Err(e) => {
            return vec![error_agent(
                "hermes",
                "Hermes Agent",
                if e.is_timeout() {
                    "Connection timed out"
                } else {
                    "Connection failed"
                },
                now,
            )];
        }
    };

    let body: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(_) => {
            return vec![error_agent(
                "hermes",
                "Hermes Agent",
                "Invalid JSON response",
                now,
            )];
        }
    };

    let session_list = body
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    if session_list.is_empty() {
        return vec![AgentInfo {
            id: "hermes-none".to_string(),
            name: "Hermes Agent".to_string(),
            provider: "hermes".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Idle,
            current_task: None,
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
            tokens_per_second: None,
            recent_errors: vec![],
            data_source: "hermes-api".to_string(),
            last_refresh_unix: now,
        }];
    }

    session_list
        .iter()
        .filter_map(|s| {
            let session: HermesSession = serde_json::from_value(s.clone()).ok()?;

            let status = determine_hermes_status(&session);
            let tokens = hermes_token_usage(&session);
            let cost = session.actual_cost_usd.or(session.estimated_cost_usd);

            // Calculate average tokens per second from session duration
            let tps = if let (Some(t), Some(started), Some(last)) =
                (tokens, session.started_at, session.last_active)
            {
                let duration = last - started;
                if duration > 0.0 {
                    Some(t as f64 / duration)
                } else {
                    None
                }
            } else {
                None
            };

            Some(AgentInfo {
                id: session.id.clone(),
                name: session
                    .title
                    .clone()
                    .unwrap_or_else(|| "Hermes Session".to_string()),
                provider: "hermes".to_string(),
                host: "local".to_string(),
                model: session.model.clone(),
                status,
                current_task: session.title.clone(),
                last_activity_unix: hermes_last_activity(&session),
                observed_at_unix: now,
                token_usage: tokens,
                context_usage: None,
                tokens_per_second: tps,
                cost_usd: cost,
                recent_errors: vec![],
                data_source: "hermes-api".to_string(),
                last_refresh_unix: now,
            })
        })
        .collect()
}

// ============================================================
// Paperclip Adapter
// ============================================================

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct PaperclipAgent {
    id: String,
    name: Option<String>,
    status: Option<String>,
    #[serde(rename = "currentTaskId")]
    current_task_id: Option<String>,
    #[serde(rename = "currentTaskTitle")]
    current_task_title: Option<String>,
    model: Option<String>,
    #[serde(rename = "lastHeartbeatAt")]
    last_heartbeat_at: Option<String>,
    #[serde(rename = "tokenUsage")]
    token_usage: Option<u64>,
    #[serde(rename = "contextUsage")]
    context_usage: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct PaperclipConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

impl PaperclipConfig {
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("PAPERCLIP_ENDPOINT").ok()?;
        let api_key = std::env::var("PAPERCLIP_API_KEY").ok();
        Some(Self {
            endpoint,
            api_key,
            timeout_secs: 5,
        })
    }
}

fn determine_paperclip_status(status: &Option<String>) -> AgentStatus {
    match status.as_deref() {
        Some("running") | Some("active") => AgentStatus::Running,
        Some("idle") => AgentStatus::Idle,
        Some("failed") | Some("error") => AgentStatus::Failed,
        Some("offline") => AgentStatus::Offline,
        _ => AgentStatus::Unknown,
    }
}

pub async fn fetch_paperclip_agents(config: &PaperclipConfig) -> Vec<AgentInfo> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .unwrap_or_default();

    // First, fetch companies to get company IDs
    let companies_url = format!("{}/api/companies", config.endpoint.trim_end_matches('/'));
    let mut companies_req = client.get(&companies_url);
    if let Some(key) = &config.api_key {
        companies_req = companies_req.header("X-API-Key", key);
    }

    let companies_resp = match companies_req.send().await {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            return vec![error_agent(
                "paperclip",
                "Paperclip",
                &format!("HTTP {} (companies)", resp.status()),
                now,
            )];
        }
        Err(e) => {
            return vec![error_agent(
                "paperclip",
                "Paperclip",
                if e.is_timeout() {
                    "Connection timed out"
                } else {
                    "Connection failed"
                },
                now,
            )];
        }
    };

    let companies_body: serde_json::Value = match companies_resp.json().await {
        Ok(v) => v,
        Err(_) => {
            return vec![error_agent(
                "paperclip",
                "Paperclip",
                "Invalid JSON (companies)",
                now,
            )];
        }
    };

    let company_list = companies_body.as_array().cloned().unwrap_or_default();

    // Collect agents from all companies
    let mut all_agents = Vec::new();
    for company in &company_list {
        let company_id = match company.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };

        let url = format!(
            "{}/api/companies/{}/agents",
            config.endpoint.trim_end_matches('/'),
            company_id
        );
        let mut req = client.get(&url);
        if let Some(key) = &config.api_key {
            req = req.header("X-API-Key", key);
        }

        let response = match req.send().await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                all_agents.push(error_agent(
                    "paperclip",
                    "Paperclip",
                    &format!("HTTP {} (agents)", resp.status()),
                    now,
                ));
                continue;
            }
            Err(e) => {
                all_agents.push(error_agent(
                    "paperclip",
                    "Paperclip",
                    if e.is_timeout() {
                        "Connection timed out"
                    } else {
                        "Connection failed"
                    },
                    now,
                ));
                continue;
            }
        };

        let body: serde_json::Value = match response.json().await {
            Ok(v) => v,
            Err(_) => {
                all_agents.push(error_agent(
                    "paperclip",
                    "Paperclip",
                    "Invalid JSON response",
                    now,
                ));
                continue;
            }
        };

        let agent_list = body
            .get("data")
            .and_then(|d| d.as_array())
            .or_else(|| body.get("agents").and_then(|a| a.as_array()))
            .or_else(|| body.as_array())
            .cloned()
            .unwrap_or_default();

        for a in &agent_list {
            if let Some(agent) = parse_paperclip_agent(a, now) {
                all_agents.push(agent);
            }
        }
    }

    if all_agents.is_empty() {
        all_agents.push(AgentInfo {
            id: "paperclip-none".to_string(),
            name: "Paperclip".to_string(),
            provider: "paperclip".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Idle,
            current_task: None,
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
            tokens_per_second: None,
            recent_errors: vec![],
            data_source: "paperclip-api".to_string(),
            last_refresh_unix: now,
        });
    }

    all_agents
}

fn parse_paperclip_agent(a: &serde_json::Value, now: u64) -> Option<AgentInfo> {
    let agent: PaperclipAgent = serde_json::from_value(a.clone()).ok()?;
    let status = determine_paperclip_status(&agent.status);

    Some(AgentInfo {
        id: agent.id.clone(),
        name: agent
            .name
            .clone()
            .unwrap_or_else(|| "Paperclip Agent".to_string()),
        provider: "paperclip".to_string(),
        host: "local".to_string(),
        model: agent.model.clone(),
        status,
        current_task: agent.current_task_title.clone(),
        last_activity_unix: None,
        observed_at_unix: now,
        token_usage: agent.token_usage,
        context_usage: agent.context_usage,
        cost_usd: None,
        tokens_per_second: None,
        recent_errors: vec![],
        data_source: "paperclip-api".to_string(),
        last_refresh_unix: now,
    })
}

// ============================================================
// Shared helpers
// ============================================================

fn error_agent(provider: &str, name: &str, error: &str, now: u64) -> AgentInfo {
    AgentInfo {
        id: format!("{}-error", provider),
        name: name.to_string(),
        provider: provider.to_string(),
        host: "unknown".to_string(),
        model: None,
        status: AgentStatus::Offline,
        current_task: None,
        last_activity_unix: None,
        observed_at_unix: now,
        token_usage: None,
        context_usage: None,
        cost_usd: None,
        tokens_per_second: None,
        recent_errors: vec![error.to_string()],
        data_source: format!("{}-api", provider),
        last_refresh_unix: now,
    }
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
            tokens_per_second: None,
            recent_errors: vec!["Not configured (set OPENCODE_ENDPOINT)".to_string()],
            data_source: "none".to_string(),
            last_refresh_unix: now,
        });
    }

    // Paperclip
    if let Some(config) = PaperclipConfig::from_env() {
        agents.extend(fetch_paperclip_agents(&config).await);
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        agents.push(AgentInfo {
            id: "paperclip-unconfigured".to_string(),
            name: "Paperclip".to_string(),
            provider: "paperclip".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Unknown,
            current_task: None,
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
            tokens_per_second: None,
            recent_errors: vec!["Not configured (set PAPERCLIP_ENDPOINT)".to_string()],
            data_source: "none".to_string(),
            last_refresh_unix: now,
        });
    }

    // Hermes Agent
    if let Some(config) = HermesConfig::from_env() {
        agents.extend(fetch_hermes_agents(&config).await);
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        agents.push(AgentInfo {
            id: "hermes-unconfigured".to_string(),
            name: "Hermes Agent".to_string(),
            provider: "hermes".to_string(),
            host: "local".to_string(),
            model: None,
            status: AgentStatus::Unknown,
            current_task: None,
            last_activity_unix: None,
            observed_at_unix: now,
            token_usage: None,
            context_usage: None,
            cost_usd: None,
            tokens_per_second: None,
            recent_errors: vec![
                "Not configured (set HERMES_ENDPOINT and HERMES_API_KEY)".to_string()
            ],
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

    // OpenCode tests
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
        assert_eq!(determine_opencode_status(&session), AgentStatus::Idle);
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
        assert_eq!(determine_opencode_status(&session), AgentStatus::Failed);
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
        assert_eq!(determine_opencode_status(&session), AgentStatus::Unknown);
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
        assert_eq!(opencode_last_activity(&session), Some(3));
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
        assert_eq!(opencode_last_activity(&session), Some(1));
    }

    // Hermes tests
    #[test]
    fn hermes_status_ended_with_error_is_failed() {
        let session = HermesSession {
            id: "test".to_string(),
            source: None,
            user_id: None,
            model: Some("gpt-6-luna-900k".to_string()),
            title: None,
            started_at: Some(1000.0),
            ended_at: Some(2000.0),
            end_reason: Some("error".to_string()),
            message_count: None,
            tool_call_count: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            estimated_cost_usd: None,
            actual_cost_usd: None,
            api_call_count: None,
            parent_session_id: None,
            last_active: None,
            preview: None,
        };
        assert_eq!(determine_hermes_status(&session), AgentStatus::Failed);
    }

    #[test]
    fn hermes_status_ended_normally_is_idle() {
        let session = HermesSession {
            id: "test".to_string(),
            source: None,
            user_id: None,
            model: Some("gpt-6-luna-900k".to_string()),
            title: None,
            started_at: Some(1000.0),
            ended_at: Some(2000.0),
            end_reason: None,
            message_count: None,
            tool_call_count: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            estimated_cost_usd: None,
            actual_cost_usd: None,
            api_call_count: None,
            parent_session_id: None,
            last_active: None,
            preview: None,
        };
        assert_eq!(determine_hermes_status(&session), AgentStatus::Idle);
    }

    #[test]
    fn hermes_token_usage_sums_all() {
        let session = HermesSession {
            id: "test".to_string(),
            source: None,
            user_id: None,
            model: None,
            title: None,
            started_at: None,
            ended_at: None,
            end_reason: None,
            message_count: None,
            tool_call_count: None,
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_read_tokens: Some(200),
            cache_write_tokens: Some(10),
            reasoning_tokens: Some(25),
            estimated_cost_usd: None,
            actual_cost_usd: None,
            api_call_count: None,
            parent_session_id: None,
            last_active: None,
            preview: None,
        };
        assert_eq!(hermes_token_usage(&session), Some(385));
    }

    #[test]
    fn hermes_token_usage_none_when_zero() {
        let session = HermesSession {
            id: "test".to_string(),
            source: None,
            user_id: None,
            model: None,
            title: None,
            started_at: None,
            ended_at: None,
            end_reason: None,
            message_count: None,
            tool_call_count: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            estimated_cost_usd: None,
            actual_cost_usd: None,
            api_call_count: None,
            parent_session_id: None,
            last_active: None,
            preview: None,
        };
        assert_eq!(hermes_token_usage(&session), None);
    }

    #[test]
    fn opencode_config_from_env_none() {
        std::env::remove_var("OPENCODE_ENDPOINT");
        assert!(OpenCodeConfig::from_env().is_none());
    }

    #[test]
    fn hermes_config_from_env_none() {
        std::env::remove_var("HERMES_ENDPOINT");
        assert!(HermesConfig::from_env().is_none());
    }

    #[test]
    fn agent_status_serializes() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn paperclip_config_from_env_none() {
        std::env::remove_var("PAPERCLIP_ENDPOINT");
        assert!(PaperclipConfig::from_env().is_none());
    }

    #[test]
    fn determine_paperclip_status_running() {
        assert_eq!(
            determine_paperclip_status(&Some("running".to_string())),
            AgentStatus::Running
        );
        assert_eq!(
            determine_paperclip_status(&Some("active".to_string())),
            AgentStatus::Running
        );
    }

    #[test]
    fn determine_paperclip_status_idle() {
        assert_eq!(
            determine_paperclip_status(&Some("idle".to_string())),
            AgentStatus::Idle
        );
    }

    #[test]
    fn determine_paperclip_status_failed() {
        assert_eq!(
            determine_paperclip_status(&Some("failed".to_string())),
            AgentStatus::Failed
        );
        assert_eq!(
            determine_paperclip_status(&Some("error".to_string())),
            AgentStatus::Failed
        );
    }

    #[test]
    fn determine_paperclip_status_unknown() {
        assert_eq!(determine_paperclip_status(&None), AgentStatus::Unknown);
        assert_eq!(
            determine_paperclip_status(&Some("unknown".to_string())),
            AgentStatus::Unknown
        );
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
            tokens_per_second: Some(1000.0),
            recent_errors: vec![],
            data_source: "opencode-api".to_string(),
            last_refresh_unix: 1792846204,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"cost_usd\":3.13"));
        assert!(json.contains("\"token_usage\":6000000"));
        assert!(json.contains("\"last_activity_unix\":1792846204"));
        assert!(json.contains("\"observed_at_unix\":1792846204"));
    }
}
