use serde::{Deserialize, Serialize};

use crate::pressure::{DiskLockMode, PressureBand};
use crate::types::{GuardianEvent, ProcessSample, ThrottleLevel};

pub const PIPE_NAME: &str = r"\\.\pipe\unstick";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientRequest {
    GetStatus,
    Pause { minutes: u32 },
    Resume,
    TrustPid { pid: u32 },
    AddAllowPath { path: String },
    AddWhitelist { entry: String },
    RemoveWhitelist { entry: String },
    Events { limit: usize },
    SetCriticalGuard { enabled: bool },
    /// User safe disk usage: soft = Disk Lock limit I/O; hard = pause/suspend offenders.
    SetDiskSafeThresholds { soft_pct: f32, hard_pct: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerPush {
    Status(StatusSnapshot),
    Events { events: Vec<GuardianEvent> },
    Ok { message: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub paused: bool,
    pub pause_until_unix: Option<i64>,
    #[serde(default = "default_true")]
    pub critical_guard: bool,
    pub pressure_score: f32,
    pub pressure_band: PressureBand,
    #[serde(default)]
    pub tripwire: Option<String>,
    #[serde(default)]
    pub disk_lock: DiskLockMode,
    /// Live soft busy% threshold (calibrated when adaptive).
    #[serde(default)]
    pub disk_lock_soft_pct: f32,
    /// Live hard busy% threshold.
    #[serde(default)]
    pub disk_lock_hard_pct: f32,
    #[serde(default)]
    pub disk_calibrated: bool,
    #[serde(default = "default_true")]
    pub disk_lock_adaptive: bool,
    /// Hardware saturation 0–1 from calibrator.
    #[serde(default)]
    pub disk_saturation: f32,
    /// Learned peak useful throughput (bytes/sec).
    #[serde(default)]
    pub disk_peak_io_bps: f32,
    pub cpu_percent: f32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_busy_percent: f32,
    pub disk_queue_length: f32,
    pub top_processes: Vec<ProcessSample>,
    pub recent_throttles: Vec<ThrottleSummary>,
    pub recent_abuse: Vec<AbuseSummary>,
    #[serde(default)]
    pub suspended: Vec<SuspendedSummary>,
    #[serde(default)]
    pub whitelist: Vec<String>,
    pub service_uptime_secs: u64,
    /// App version string (e.g. "0.1.0").
    #[serde(default)]
    pub version: String,
    /// Recent OpenProcess / suspend failures (often elevation).
    #[serde(default)]
    pub apply_denied: Vec<ApplyDeniedSummary>,
    /// Processes resumed from durable ledger at service start (this session).
    #[serde(default)]
    pub recovered_suspends: u32,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleSummary {
    pub pid: u32,
    pub name: String,
    pub level: ThrottleLevel,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbuseSummary {
    pub pid: u32,
    pub name: String,
    pub score: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspendedSummary {
    pub pid: u32,
    pub name: String,
    pub reason: String,
    pub suspended_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyDeniedSummary {
    pub pid: u32,
    pub name: String,
    pub error: String,
    /// True when error looks like access denied / elevation.
    #[serde(default)]
    pub elevation_likely: bool,
}
