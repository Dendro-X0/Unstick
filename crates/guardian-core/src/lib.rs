//! Shared types, pressure scoring, and policy planning for Unstick.

mod config;
mod disk_calibrate;
mod ipc;
mod policy;
mod pressure;
mod suspend_persist;
mod types;

pub use config::{GuardianConfig, load_config, save_config, config_dir, events_path, status_path};
pub use disk_calibrate::{saturation_index, DiskCalibrator};
pub use ipc::{
    AbuseSummary, ApplyDeniedSummary, ClientRequest, ServerPush, StatusSnapshot, SuspendedSummary,
    ThrottleSummary, PIPE_NAME,
};
pub use policy::{ActionPlan, PlannedAction, PolicyEngine, ProtectedSet};
pub use pressure::{
    evaluate_tripwire, score_pressure, score_pressure_tracked, update_disk_lock_streaks,
    DiskLockMode, DiskLockThresholds, HysteresisTracker, PressureBand, PressureInputs,
    PressureState,
};
pub use suspend_persist::{
    build_persist_file, clear_suspend_ledger, ledger_is_stale, load_suspend_ledger,
    save_suspend_ledger, suspend_ledger_path, PersistedSuspendEntry, PersistedSuspendFile,
};
pub use types::{GuardianEvent, ProcessSample, SystemSample, ThrottleLevel};

pub const APP_NAME: &str = "Unstick";
pub const SERVICE_BIN: &str = "guardian-service";
pub const TRAY_BIN: &str = "guardian-tray";
/// Workspace package version (e.g. 0.1.0).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
