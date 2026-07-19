use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::APP_NAME;

/// How Critical Guard escalates under Emergency / Disk Lock Hard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CriticalGuardMode {
    /// Progressive soft throttle only — never NtSuspend (default).
    #[default]
    SoftOnly,
    /// Soft ladder first; Suspend only after sustained hard pressure streak.
    LastResortSuspend,
}

impl CriticalGuardMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SoftOnly => "soft_only",
            Self::LastResortSuspend => "last_resort_suspend",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianConfig {
    pub pause_until: Option<DateTime<Utc>>,
    /// Critical Guard master enable (soft ladder always; Suspend only with mode).
    #[serde(default = "default_true")]
    pub emergency_suspend: bool,
    /// Soft-only (default) vs last-resort Suspend after streak.
    #[serde(default)]
    pub critical_guard_mode: CriticalGuardMode,
    /// Opt-in NtSuspend (Last-resort). Default **false** (D1 hardware-control path).
    /// Without this, `last_resort_suspend` is forced back to SoftOnly on load.
    #[serde(default)]
    pub experimental_suspend: bool,
    /// Consecutive Emergency/Disk Hard samples before last-resort Suspend.
    #[serde(default = "default_suspend_escalation_streak")]
    pub suspend_escalation_streak: u32,
    #[serde(default = "default_max_suspend_pids")]
    pub max_suspend_pids: usize,
    #[serde(default = "default_max_suspend_secs")]
    pub max_suspend_secs: u64,
    /// Soft demotion TTL: force restore EcoQoS/priority even if still in plan (recovery window).
    #[serde(default = "default_max_soft_demote_secs")]
    pub max_soft_demote_secs: u64,
    /// Disk Lock: PDH-driven soft/hard actions on system disk saturation.
    #[serde(default = "default_true")]
    pub disk_lock_enabled: bool,
    /// D3: closed-loop disk setpoint (u_disk → soft intensity). Default ON.
    #[serde(default = "default_true")]
    pub disk_control_enabled: bool,
    /// D4: closed-loop memory setpoint (u_mem → soft intensity; WS trim paging-gated). Default ON.
    #[serde(default = "default_true")]
    pub mem_control_enabled: bool,
    /// v0.6: allow intensity 3 (Idle + EcoQoS) after sustained cliff at intensity 2.
    #[serde(default = "default_true")]
    pub idle_under_stress_enabled: bool,
    /// Ticks at intensity 2 with cliff before Idle escalate.
    #[serde(default = "default_idle_escalate_streak")]
    pub idle_escalate_streak: u32,
    /// Ticks without cliff before leaving Idle back to Soft ceiling.
    #[serde(default = "default_idle_release_streak")]
    pub idle_release_streak: u32,
    /// Learn soft/hard thresholds from this machine's disk behavior (default ON).
    #[serde(default = "default_true")]
    pub disk_lock_adaptive: bool,
    /// Prior / fallback soft busy% used before calibration (and when adaptive=false).
    #[serde(default = "default_disk_busy_soft")]
    pub disk_busy_soft_pct: f32,
    /// Prior / fallback hard busy%.
    #[serde(default = "default_disk_busy_hard")]
    pub disk_busy_hard_pct: f32,
    #[serde(default = "default_disk_busy_streak")]
    pub disk_busy_streak: u32,
    /// Soft Disk Lock when Avg. Disk sec/Transfer ≥ this (seconds). Default 15ms.
    #[serde(default = "default_disk_latency_soft")]
    pub disk_latency_soft_sec: f32,
    /// Hard Disk Lock / tripwire when latency ≥ this. Default 40ms.
    #[serde(default = "default_disk_latency_hard")]
    pub disk_latency_hard_sec: f32,
    /// Mem Lock: RSS trim when available RAM / commit scarce.
    #[serde(default = "default_true")]
    pub mem_lock_enabled: bool,
    /// Soft when available RAM % of total is below this.
    #[serde(default = "default_mem_avail_soft")]
    pub mem_avail_soft_pct: f32,
    /// Hard when available % below this (and paging evidence by default).
    #[serde(default = "default_mem_avail_hard")]
    pub mem_avail_hard_pct: f32,
    #[serde(default = "default_mem_commit_soft")]
    pub mem_commit_soft_pct: f32,
    #[serde(default = "default_mem_commit_hard")]
    pub mem_commit_hard_pct: f32,
    #[serde(default = "default_mem_lock_streak")]
    pub mem_lock_streak: u32,
    /// Hard Mem Lock requires paging_pressure_evidence.
    #[serde(default = "default_true")]
    pub mem_lock_hard_requires_paging: bool,
    /// Last applied Guard profile (`dev` | `gaming` | `quiet`).
    #[serde(default = "crate::profiles::default_active_profile")]
    pub active_profile: String,
    pub allow_paths: Vec<String>,
    /// User whitelist: never soft-throttle, suspend, or terminate matching processes.
    /// Entries match executable name (e.g. `steam.exe`) or path substring (e.g. `\steam\`).
    #[serde(default)]
    pub whitelist: Vec<String>,
    /// Legacy alias merged into whitelist on load.
    #[serde(default)]
    pub protected_extra: Vec<String>,
    pub job_cpu_rate_percent: u32,
    pub sample_idle_ms: u64,
    pub sample_busy_ms: u64,
    pub trusted_pids: Vec<u32>,
}

fn default_true() -> bool {
    true
}
fn default_max_suspend_pids() -> usize {
    6
}
fn default_max_suspend_secs() -> u64 {
    45
}
fn default_max_soft_demote_secs() -> u64 {
    45
}
fn default_idle_escalate_streak() -> u32 {
    4
}
fn default_idle_release_streak() -> u32 {
    2
}
fn default_disk_busy_soft() -> f32 {
    85.0
}
fn default_disk_busy_hard() -> f32 {
    95.0
}
fn default_disk_busy_streak() -> u32 {
    2
}
fn default_disk_latency_soft() -> f32 {
    0.015
}
fn default_disk_latency_hard() -> f32 {
    0.040
}
fn default_mem_avail_soft() -> f32 {
    15.0
}
fn default_mem_avail_hard() -> f32 {
    8.0
}
fn default_mem_commit_soft() -> f32 {
    90.0
}
fn default_mem_commit_hard() -> f32 {
    95.0
}
fn default_mem_lock_streak() -> u32 {
    2
}
fn default_suspend_escalation_streak() -> u32 {
    3
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            pause_until: None,
            emergency_suspend: true,
            critical_guard_mode: CriticalGuardMode::SoftOnly,
            experimental_suspend: false,
            suspend_escalation_streak: 3,
            max_suspend_pids: 6,
            max_suspend_secs: 45,
            max_soft_demote_secs: 45,
            disk_lock_enabled: true,
            disk_control_enabled: true,
            mem_control_enabled: true,
            idle_under_stress_enabled: true,
            idle_escalate_streak: 4,
            idle_release_streak: 2,
            disk_lock_adaptive: true,
            disk_busy_soft_pct: 85.0,
            disk_busy_hard_pct: 95.0,
            disk_busy_streak: 2,
            disk_latency_soft_sec: 0.015,
            disk_latency_hard_sec: 0.040,
            mem_lock_enabled: true,
            mem_avail_soft_pct: 15.0,
            mem_avail_hard_pct: 8.0,
            mem_commit_soft_pct: 90.0,
            mem_commit_hard_pct: 95.0,
            mem_lock_streak: 2,
            mem_lock_hard_requires_paging: true,
            active_profile: crate::profiles::default_active_profile(),
            allow_paths: default_allow_paths(),
            whitelist: Vec::new(),
            protected_extra: Vec::new(),
            job_cpu_rate_percent: 70,
            sample_idle_ms: 2000,
            sample_busy_ms: 500,
            trusted_pids: Vec::new(),
        }
    }
}

fn default_allow_paths() -> Vec<String> {
    vec![
        r"\.cargo\".to_string(),
        r"\rustup\".to_string(),
        r"\nodejs\".to_string(),
        r"\node\".to_string(),
        r"\docker\".to_string(),
        r"\microsoft visual studio\".to_string(),
        r"\cursor\".to_string(),
        r"\vscode\".to_string(),
        r"\microsoft vs code\".to_string(),
    ]
}

pub fn config_dir() -> PathBuf {
    dirs_fallback().join(APP_NAME)
}

fn dirs_fallback() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local);
    }
    std::env::temp_dir()
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn events_path() -> PathBuf {
    config_dir().join("events.jsonl")
}

pub fn status_path() -> PathBuf {
    config_dir().join("status.json")
}

pub fn config_export_path() -> PathBuf {
    config_dir().join("exports").join("unstick-config.json")
}

pub fn config_import_path() -> PathBuf {
    let preferred = config_dir().join("imports").join("unstick-config.json");
    if preferred.exists() {
        preferred
    } else {
        config_export_path()
    }
}

/// Pretty-print config to the standard export path. Returns that path.
pub fn export_config_json(cfg: &GuardianConfig) -> std::io::Result<PathBuf> {
    let path = config_export_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());
    fs::write(&path, raw)?;
    Ok(path)
}

/// Load config JSON from import/export path; sanitize for live apply.
pub fn import_config_json_from(path: &std::path::Path) -> Result<GuardianConfig, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut cfg: GuardianConfig =
        serde_json::from_str(&raw).map_err(|e| format!("invalid JSON: {e}"))?;
    cfg.pause_until = None;
    cfg.normalize_whitelist();
    cfg.normalize_suspend_product_path();
    Ok(cfg)
}

pub fn import_config_json() -> Result<GuardianConfig, String> {
    import_config_json_from(&config_import_path())
}

pub fn load_config() -> GuardianConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let mut cfg: GuardianConfig = serde_json::from_str(&raw).unwrap_or_default();
            cfg.normalize_whitelist();
            cfg.normalize_suspend_product_path();
            cfg
        }
        Err(_) => {
            let cfg = GuardianConfig::default();
            let _ = save_config(&cfg);
            cfg
        }
    }
}

impl GuardianConfig {
    /// Merge legacy `protected_extra` into `whitelist` and dedupe (case-insensitive).
    pub fn normalize_whitelist(&mut self) {
        for extra in std::mem::take(&mut self.protected_extra) {
            if !self
                .whitelist
                .iter()
                .any(|w| w.eq_ignore_ascii_case(&extra))
            {
                self.whitelist.push(extra);
            }
        }
    }

    /// D1: Suspend is experimental — force SoftOnly unless `experimental_suspend`.
    pub fn normalize_suspend_product_path(&mut self) {
        if !self.experimental_suspend
            && self.critical_guard_mode == CriticalGuardMode::LastResortSuspend
        {
            self.critical_guard_mode = CriticalGuardMode::SoftOnly;
        }
    }

    /// True when NtSuspend / Last-resort is allowed.
    pub fn suspend_allowed(&self) -> bool {
        self.experimental_suspend
            && self.emergency_suspend
            && self.critical_guard_mode == CriticalGuardMode::LastResortSuspend
    }

    pub fn add_whitelist(&mut self, entry: String) -> bool {
        let entry = entry.trim().to_string();
        if entry.is_empty() {
            return false;
        }
        if self
            .whitelist
            .iter()
            .any(|w| w.eq_ignore_ascii_case(&entry))
        {
            return false;
        }
        self.whitelist.push(entry);
        true
    }

    pub fn remove_whitelist(&mut self, entry: &str) -> bool {
        let before = self.whitelist.len();
        self.whitelist
            .retain(|w| !w.eq_ignore_ascii_case(entry));
        self.whitelist.len() < before
    }
}

pub fn save_config(cfg: &GuardianConfig) -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    let raw = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());
    fs::write(config_path(), raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_critical_guard_on() {
        let cfg = GuardianConfig::default();
        assert!(cfg.emergency_suspend);
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        assert!(!cfg.experimental_suspend);
        assert!(!cfg.suspend_allowed());
        assert_eq!(cfg.suspend_escalation_streak, 3);
        assert_eq!(cfg.max_suspend_pids, 6);
        assert_eq!(cfg.max_suspend_secs, 45);
        assert_eq!(cfg.max_soft_demote_secs, 45);
        assert!(cfg.idle_under_stress_enabled);
        assert_eq!(cfg.idle_escalate_streak, 4);
        assert_eq!(cfg.idle_release_streak, 2);
        assert!(cfg.disk_lock_enabled);
        assert!(cfg.disk_lock_adaptive);
        assert_eq!(cfg.disk_busy_soft_pct, 85.0);
        assert_eq!(cfg.disk_busy_hard_pct, 95.0);
        assert_eq!(cfg.disk_busy_streak, 2);
        assert!((cfg.disk_latency_soft_sec - 0.015).abs() < f32::EPSILON);
        assert!((cfg.disk_latency_hard_sec - 0.040).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_mode_deserializes_soft_only() {
        let raw = r#"{"pause_until":null,"emergency_suspend":true,"max_suspend_pids":6,"max_suspend_secs":45,"disk_lock_enabled":true,"disk_lock_adaptive":true,"disk_busy_soft_pct":85.0,"disk_busy_hard_pct":95.0,"disk_busy_streak":2,"allow_paths":[],"whitelist":[],"protected_extra":[],"job_cpu_rate_percent":70,"sample_idle_ms":2000,"sample_busy_ms":500,"trusted_pids":[]}"#;
        let cfg: GuardianConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        assert_eq!(cfg.suspend_escalation_streak, 3);
        assert!(cfg.idle_under_stress_enabled);
        assert_eq!(cfg.idle_escalate_streak, 4);
        assert_eq!(cfg.idle_release_streak, 2);
    }

    #[test]
    fn idle_under_stress_can_disable_via_json() {
        let raw = r#"{"pause_until":null,"idle_under_stress_enabled":false,"idle_escalate_streak":6,"idle_release_streak":3,"allow_paths":[],"job_cpu_rate_percent":70,"sample_idle_ms":2000,"sample_busy_ms":500,"trusted_pids":[]}"#;
        let cfg: GuardianConfig = serde_json::from_str(raw).unwrap();
        assert!(!cfg.idle_under_stress_enabled);
        assert_eq!(cfg.idle_escalate_streak, 6);
        assert_eq!(cfg.idle_release_streak, 3);
    }

    #[test]
    fn whitelist_add_remove_dedupe() {
        let mut cfg = GuardianConfig::default();
        assert!(cfg.add_whitelist("Steam.exe".into()));
        assert!(!cfg.add_whitelist("steam.exe".into()));
        assert!(cfg.remove_whitelist("STEAM.EXE"));
        assert!(cfg.whitelist.is_empty());
    }

    #[test]
    fn normalize_merges_protected_extra() {
        let mut cfg = GuardianConfig::default();
        cfg.protected_extra.push("mygame.exe".into());
        cfg.normalize_whitelist();
        assert!(cfg.whitelist.iter().any(|w| w == "mygame.exe"));
        assert!(cfg.protected_extra.is_empty());
    }

    #[test]
    fn normalize_forces_soft_only_without_experimental_suspend() {
        let mut cfg = GuardianConfig::default();
        cfg.critical_guard_mode = CriticalGuardMode::LastResortSuspend;
        cfg.experimental_suspend = false;
        cfg.normalize_suspend_product_path();
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        assert!(!cfg.suspend_allowed());

        cfg.critical_guard_mode = CriticalGuardMode::LastResortSuspend;
        cfg.experimental_suspend = true;
        cfg.normalize_suspend_product_path();
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::LastResortSuspend);
        assert!(cfg.suspend_allowed());
    }

    #[test]
    fn export_import_round_trip_clears_pause() {
        let dir = std::env::temp_dir().join(format!("unstick-export-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("unstick-config.json");
        let mut cfg = GuardianConfig::default();
        cfg.pause_until = Some(Utc::now());
        assert!(cfg.add_whitelist("game.exe".into()));
        fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded = import_config_json_from(&path).unwrap();
        assert!(loaded.pause_until.is_none());
        assert!(loaded.whitelist.iter().any(|w| w == "game.exe"));
        let _ = fs::remove_dir_all(&dir);
    }
}
