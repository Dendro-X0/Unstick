use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThrottleLevel {
    None,
    BelowNormal,
    Idle,
    Suspend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: Option<String>,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_read_bytes_per_sec: u64,
    pub disk_write_bytes_per_sec: u64,
    pub cmd_line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSample {
    pub timestamp: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_commit_percent: f32,
    pub disk_busy_percent: f32,
    pub disk_queue_length: f32,
    /// Aggregate process disk read+write bytes/sec (for hardware calibration).
    #[serde(default)]
    pub disk_io_bytes_per_sec: u64,
    pub hard_faults_per_sec: f32,
    pub processes: Vec<ProcessSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardianEvent {
    Pressure {
        band: String,
        score: f32,
        at: DateTime<Utc>,
    },
    Throttle {
        pid: u32,
        name: String,
        level: ThrottleLevel,
        reason: String,
        at: DateTime<Utc>,
    },
    Suspend {
        pid: u32,
        name: String,
        reason: String,
        at: DateTime<Utc>,
    },
    Resume {
        pid: u32,
        name: String,
        reason: String,
        at: DateTime<Utc>,
    },
    Abuse {
        pid: u32,
        name: String,
        score: u32,
        reasons: Vec<String>,
        at: DateTime<Utc>,
    },
    Info {
        message: String,
        at: DateTime<Utc>,
    },
}
