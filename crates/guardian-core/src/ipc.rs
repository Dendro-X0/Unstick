use serde::{Deserialize, Serialize};

use crate::advisory::{CoolingMode, ThermalLevel};
use crate::config::CriticalGuardMode;
use crate::control::DiskControlMode;
use crate::envelope::EnvelopeSnapshot;
use crate::pressure::{DiskLockMode, MemLockMode, PressureBand};
use crate::qos::{NapPolicy, QosClass};
use crate::types::{FocusProfile, GuardianEvent, ProcessSample, ThrottleLevel};

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
    SetCriticalGuardMode { mode: CriticalGuardMode },
    /// User safe disk usage: soft = Disk Lock limit I/O; hard = pause/suspend offenders.
    SetDiskSafeThresholds { soft_pct: f32, hard_pct: f32 },
    /// User safe RAM available %: soft = WS trim; hard = deeper trim / optional Suspend.
    SetMemSafeThresholds { soft_pct: f32, hard_pct: f32 },
    /// Apply Soft policy skin: `dev` | `gaming` | `quiet`.
    SetProfile { profile: String },
    /// Write config JSON to AppData exports\unstick-config.json.
    ExportConfig,
    /// Load config JSON from imports\ or exports\unstick-config.json.
    ImportConfig,
    /// Opt-in short disk_hog prove soak (512 MiB × 90s) if disk-hog.exe is beside the service.
    StartProveDiskHog,
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
    #[serde(default)]
    pub critical_guard_mode: CriticalGuardMode,
    /// NtSuspend / Last-resort requires this opt-in (D1).
    #[serde(default)]
    pub experimental_suspend: bool,
    #[serde(default)]
    pub focus_pid: Option<u32>,
    #[serde(default)]
    pub focus_name: Option<String>,
    #[serde(default)]
    pub focus_profile: FocusProfile,
    /// Planned QoS for the focused tree (Apple Energy Efficiency analogue).
    #[serde(default)]
    pub focus_qos: QosClass,
    /// Planned QoS for background offenders.
    #[serde(default)]
    pub background_qos: QosClass,
    /// Cooperate (SoftOnly / App Nap) vs force_pause (LastResort analogue).
    #[serde(default)]
    pub nap_policy: NapPolicy,
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
    #[serde(default)]
    pub mem_lock: MemLockMode,
    #[serde(default)]
    pub mem_lock_soft_pct: f32,
    #[serde(default)]
    pub mem_lock_hard_pct: f32,
    pub cpu_percent: f32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_busy_percent: f32,
    pub disk_queue_length: f32,
    /// Avg. Disk sec/Transfer (system volume PhysicalDisk).
    #[serde(default)]
    pub disk_latency_sec: f32,
    #[serde(default)]
    pub hard_faults_per_sec: f32,
    #[serde(default)]
    pub pagefile_writes_per_sec: f32,
    #[serde(default)]
    pub paging_file_pct: f32,
    #[serde(default)]
    pub dpc_time_percent: f32,
    #[serde(default)]
    pub interrupt_time_percent: f32,
    /// Detect-only: elevated DPC/ISR (Unstick cannot fix).
    #[serde(default)]
    pub dpc_advisory: Option<String>,
    /// PSI-shaped stall fractions (0..1).
    #[serde(default)]
    pub stall_cpu: f32,
    #[serde(default)]
    pub stall_memory: f32,
    #[serde(default)]
    pub stall_io: f32,
    #[serde(default)]
    pub stall_memory_full: f32,
    #[serde(default)]
    pub stall_io_full: f32,
    #[serde(default)]
    pub stall_thermal: f32,
    /// D2: idle-calibrated hardware envelope + live u_disk / u_mem.
    #[serde(default)]
    pub envelope: EnvelopeSnapshot,
    /// D3/D6: disk closed-loop intensity 0..=3 (3 = Efficiency Idle when gated).
    #[serde(default)]
    pub disk_control_intensity: u8,
    #[serde(default)]
    pub disk_control_mode: DiskControlMode,
    /// D4: memory closed-loop intensity 0..=3 (3 = Efficiency Idle when gated; WS trim requires paging).
    #[serde(default)]
    pub mem_control_intensity: u8,
    #[serde(default)]
    pub mem_control_mode: DiskControlMode,
    #[serde(default)]
    pub on_battery: bool,
    #[serde(default)]
    pub battery_percent: Option<u8>,
    #[serde(default)]
    pub cooling_mode: CoolingMode,
    #[serde(default)]
    pub cpu_mhz_ratio: f32,
    #[serde(default)]
    pub thermal_level: ThermalLevel,
    #[serde(default)]
    pub thermal_advisory: Option<String>,
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
    /// Soft capped applies this service session (disk/mem control or lock reasons).
    #[serde(default)]
    pub session_capped: u32,
    /// Subset of capped with Efficiency Idle reason.
    #[serde(default)]
    pub session_efficiency_idle: u32,
    /// Soft demotions restored (left-plan or Soft TTL) this session.
    #[serde(default)]
    pub session_restored: u32,
    /// Experimental Suspend applies this session.
    #[serde(default)]
    pub session_suspended: u32,
    /// Experimental Suspend resumes this session (excludes soft_restore).
    #[serde(default)]
    pub session_resumed: u32,
    /// Last applied Guard profile (`dev` | `gaming` | `quiet`).
    #[serde(default = "crate::profiles::default_active_profile")]
    pub active_profile: String,
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
