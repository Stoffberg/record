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
}

#[derive(Debug, Clone)]
pub struct Heartbeat {
    pub app_name: String,
    pub bundle_id: String,
    pub window_title: String,
    pub is_idle: bool,
    pub timestamp: DateTime<Utc>,
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
