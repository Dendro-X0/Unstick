use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::advisory::{CoolingMode, ThermalLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThrottleLevel {
    None,
    BelowNormal,
    Idle,
    Suspend,
}

/// UI label for the focused app (same policy ladder; no separate engines yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusProfile {
    Dev,
    Play,
    #[default]
    Other,
}

impl FocusProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Play => "play",
            Self::Other => "other",
        }
    }
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
    /// PDH Avg. Disk sec/Transfer on system PhysicalDisk (0 if unavailable).
    #[serde(default)]
    pub disk_latency_sec: f32,
    /// Aggregate process disk read+write bytes/sec (for hardware calibration).
    #[serde(default)]
    pub disk_io_bytes_per_sec: u64,
    pub hard_faults_per_sec: f32,
    /// Memory\Page Writes/sec — pagefile writes only (MS).
    #[serde(default)]
    pub pagefile_writes_per_sec: f32,
    /// Paging File(_Total)\% Usage.
    #[serde(default)]
    pub paging_file_pct: f32,
    /// Processor(_Total) % DPC Time.
    #[serde(default)]
    pub dpc_time_percent: f32,
    /// Processor(_Total) % Interrupt Time.
    #[serde(default)]
    pub interrupt_time_percent: f32,
    #[serde(default)]
    pub on_battery: bool,
    #[serde(default)]
    pub battery_percent: Option<u8>,
    #[serde(default)]
    pub cooling_mode: CoolingMode,
    /// CurrentMhz/MaxMhz average (0 = unknown).
    #[serde(default)]
    pub cpu_mhz_ratio: f32,
    #[serde(default)]
    pub thermal_level: ThermalLevel,
    /// Foreground window process (GetForegroundWindow), if any.
    #[serde(default)]
    pub focus_pid: Option<u32>,
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
