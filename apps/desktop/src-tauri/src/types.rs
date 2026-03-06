use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSession {
    pub id: i64,
    pub app_name: String,
    pub bundle_id: String,
    pub window_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub is_idle: bool,
    pub metadata: String,
    pub project: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Heartbeat {
    pub app_name: String,
    pub bundle_id: String,
    pub window_title: String,
    pub is_idle: bool,
    pub timestamp: DateTime<Utc>,
    pub project: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsage {
    pub app_name: String,
    pub bundle_id: String,
    pub total_secs: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: String,
    pub total_active_secs: i64,
    pub total_idle_secs: i64,
    pub apps: Vec<AppUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUsage {
    pub project: String,
    pub total_secs: i64,
    pub active_secs: i64,
    pub agent_secs: i64,
    pub session_count: i64,
    pub details: Vec<ProjectDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetail {
    pub label: String,
    pub total_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub initials: String,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceWithProjects {
    pub space: Space,
    pub projects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceUsage {
    pub space: Option<Space>,
    pub projects: Vec<ProjectUsage>,
    pub total_secs: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone)]
pub struct TrackerConfig {
    pub poll_interval_secs: u64,
    pub idle_threshold_secs: u64,
    pub merge_gap_secs: i64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            idle_threshold_secs: 300,
            merge_gap_secs: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub agent: String,
    pub project: String,
    pub session_ref: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProjectUsage {
    pub project: String,
    pub total_secs: i64,
    pub session_count: i64,
    pub agents: Vec<AgentUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsage {
    pub agent: String,
    pub total_secs: i64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub date: String,
    pub total_agent_secs: i64,
    pub projects: Vec<AgentProjectUsage>,
}
