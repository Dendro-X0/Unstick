//! Durable suspend ledger so a crashed service can resume orphaned processes.

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSuspendEntry {
    pub pid: u32,
    pub name: String,
    pub reason: String,
    pub suspended_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedSuspendFile {
    pub service_pid: u32,
    pub written_at_unix: i64,
    #[serde(default)]
    pub entries: Vec<PersistedSuspendEntry>,
}

pub fn suspend_ledger_path() -> PathBuf {
    config_dir().join("suspend_ledger.json")
}

pub fn load_suspend_ledger() -> PersistedSuspendFile {
    let path = suspend_ledger_path();
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => PersistedSuspendFile::default(),
    }
}

pub fn save_suspend_ledger(file: &PersistedSuspendFile) -> std::io::Result<()> {
    fs::create_dir_all(config_dir())?;
    let raw = serde_json::to_string_pretty(file).unwrap_or_else(|_| "{}".into());
    fs::write(suspend_ledger_path(), raw)
}

pub fn clear_suspend_ledger() {
    let _ = fs::remove_file(suspend_ledger_path());
}

/// True if ledger is from a previous service instance or older than stale_secs.
pub fn ledger_is_stale(file: &PersistedSuspendFile, current_pid: u32, stale_secs: i64) -> bool {
    if file.entries.is_empty() {
        return false;
    }
    if file.service_pid != 0 && file.service_pid != current_pid {
        return true;
    }
    let age = Utc::now().timestamp() - file.written_at_unix;
    age >= stale_secs
}

pub fn build_persist_file(
    service_pid: u32,
    entries: impl IntoIterator<Item = PersistedSuspendEntry>,
) -> PersistedSuspendFile {
    PersistedSuspendFile {
        service_pid,
        written_at_unix: Utc::now().timestamp(),
        entries: entries.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_when_different_service_pid() {
        let f = PersistedSuspendFile {
            service_pid: 1,
            written_at_unix: Utc::now().timestamp(),
            entries: vec![PersistedSuspendEntry {
                pid: 9,
                name: "x".into(),
                reason: "t".into(),
                suspended_at_unix: Utc::now().timestamp(),
            }],
        };
        assert!(ledger_is_stale(&f, 2, 3600));
        assert!(!ledger_is_stale(&f, 1, 3600));
    }

    #[test]
    fn empty_not_stale() {
        let f = PersistedSuspendFile::default();
        assert!(!ledger_is_stale(&f, 1, 1));
    }
}
