//! Shared types, pressure scoring, and policy planning for Unstick.

mod config;
mod disk_calibrate;
mod ipc;
mod advisory;
mod policy;
mod pressure;
mod qos;
mod suspend_persist;
mod types;

pub use advisory::{
    classify_dpc_isr, classify_thermal_power, dpc_advisory_message, dpc_isr_raw_level,
    thermal_advisory_message, CoolingMode, DpcAdvisoryLevel, ThermalLevel, ThermalPowerInputs,
};
pub use config::{
    CriticalGuardMode, GuardianConfig, load_config, save_config, config_dir, events_path,
    status_path,
};
pub use disk_calibrate::{saturation_index, DiskCalibrator};
pub use ipc::{
    AbuseSummary, ApplyDeniedSummary, ClientRequest, ServerPush, StatusSnapshot, SuspendedSummary,
    ThrottleSummary, PIPE_NAME,
};
pub use policy::{
    classify_focus_profile, focus_tree_pids, ActionPlan, PlannedAction, PolicyEngine, ProtectedSet,
};
pub use pressure::{
    compute_stalls, evaluate_tripwire, looks_like_mapped_io, paging_pressure_evidence,
    score_from_stalls, score_pressure, score_pressure_tracked, update_disk_lock_streaks,
    update_mem_lock_streaks, DiskLockMode, DiskLockThresholds, HysteresisTracker, MemLockMode,
    MemLockThresholds, PressureBand, PressureInputs, PressureState, StallFractions,
};
pub use suspend_persist::{
    build_persist_file, clear_suspend_ledger, ledger_is_stale, load_suspend_ledger,
    save_suspend_ledger, suspend_ledger_path, PersistedSuspendEntry, PersistedSuspendFile,
};
pub use qos::{plan_qos, NapPolicy, QosClass, QosPlan};
pub use types::{FocusProfile, GuardianEvent, ProcessSample, SystemSample, ThrottleLevel};

pub const APP_NAME: &str = "Unstick";
pub const SERVICE_BIN: &str = "guardian-service";
pub const TRAY_BIN: &str = "guardian-tray";
/// Workspace package version (e.g. 0.1.0).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
