use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::APP_NAME;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianConfig {
    pub pause_until: Option<DateTime<Utc>>,
    /// Critical Guard: NtSuspendProcess under emergency (default ON).
    #[serde(default = "default_true")]
    pub emergency_suspend: bool,
    #[serde(default = "default_max_suspend_pids")]
    pub max_suspend_pids: usize,
    #[serde(default = "default_max_suspend_secs")]
    pub max_suspend_secs: u64,
    /// Disk Lock: PDH-driven soft/hard actions on system disk saturation.
    #[serde(default = "default_true")]
    pub disk_lock_enabled: bool,
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
fn default_disk_busy_soft() -> f32 {
    85.0
}
fn default_disk_busy_hard() -> f32 {
    95.0
}
fn default_disk_busy_streak() -> u32 {
    2
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            pause_until: None,
            emergency_suspend: true,
            max_suspend_pids: 6,
            max_suspend_secs: 45,
            disk_lock_enabled: true,
            disk_lock_adaptive: true,
            disk_busy_soft_pct: 85.0,
            disk_busy_hard_pct: 95.0,
            disk_busy_streak: 2,
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

pub fn load_config() -> GuardianConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let mut cfg: GuardianConfig = serde_json::from_str(&raw).unwrap_or_default();
            cfg.normalize_whitelist();
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
        assert_eq!(cfg.max_suspend_pids, 6);
        assert_eq!(cfg.max_suspend_secs, 45);
        assert!(cfg.disk_lock_enabled);
        assert!(cfg.disk_lock_adaptive);
        assert_eq!(cfg.disk_busy_soft_pct, 85.0);
        assert_eq!(cfg.disk_busy_hard_pct, 95.0);
        assert_eq!(cfg.disk_busy_streak, 2);
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
}
