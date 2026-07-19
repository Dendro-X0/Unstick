//! Guard profiles (v0.7) — Soft policy skins over existing knobs.

use crate::config::{CriticalGuardMode, GuardianConfig};

/// Applyable profile ids (snake_case in config / IPC).
pub const PROFILE_DEV: &str = "dev";
pub const PROFILE_GAMING: &str = "gaming";
pub const PROFILE_QUIET: &str = "quiet";

pub fn default_active_profile() -> String {
    PROFILE_DEV.to_string()
}

pub fn normalize_profile_id(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dev" | "development" => Some(PROFILE_DEV),
        "gaming" | "game" => Some(PROFILE_GAMING),
        "quiet" | "headroom" => Some(PROFILE_QUIET),
        _ => None,
    }
}

/// Merge preset whitelist + Soft knobs. Never enables Suspend. Additive whitelist only.
pub fn apply_profile(cfg: &mut GuardianConfig, profile: &str) -> Result<&'static str, String> {
    let id = normalize_profile_id(profile)
        .ok_or_else(|| format!("unknown profile '{profile}' (use dev|gaming|quiet)"))?;

    cfg.critical_guard_mode = CriticalGuardMode::SoftOnly;
    // Do not force experimental_suspend off if user already opted in — SoftOnly mode still gates.
    cfg.disk_control_enabled = true;
    cfg.mem_control_enabled = true;
    cfg.idle_under_stress_enabled = true;
    cfg.disk_lock_enabled = true;
    cfg.mem_lock_enabled = true;

    match id {
        PROFILE_DEV => {
            cfg.disk_busy_soft_pct = 85.0;
            cfg.disk_busy_hard_pct = 95.0;
            cfg.mem_avail_soft_pct = 15.0;
            cfg.mem_avail_hard_pct = 8.0;
            cfg.idle_escalate_streak = 4;
            cfg.idle_release_streak = 2;
            cfg.max_soft_demote_secs = 45;
            for e in DEV_WHITELIST {
                let _ = cfg.add_whitelist((*e).to_string());
            }
        }
        PROFILE_GAMING => {
            cfg.disk_busy_soft_pct = 85.0;
            cfg.disk_busy_hard_pct = 95.0;
            cfg.mem_avail_soft_pct = 15.0;
            cfg.mem_avail_hard_pct = 8.0;
            cfg.idle_escalate_streak = 5;
            cfg.idle_release_streak = 2;
            cfg.max_soft_demote_secs = 45;
            for e in GAMING_WHITELIST {
                let _ = cfg.add_whitelist((*e).to_string());
            }
        }
        PROFILE_QUIET => {
            cfg.disk_busy_soft_pct = 75.0;
            cfg.disk_busy_hard_pct = 90.0;
            cfg.mem_avail_soft_pct = 20.0;
            cfg.mem_avail_hard_pct = 12.0;
            cfg.idle_escalate_streak = 3;
            cfg.idle_release_streak = 2;
            cfg.max_soft_demote_secs = 30;
        }
        _ => unreachable!(),
    }

    cfg.active_profile = id.to_string();
    Ok(id)
}

const DEV_WHITELIST: &[&str] = &[
    "Code.exe",
    "Cursor.exe",
    "devenv.exe",
    "idea64.exe",
    "WindowsTerminal.exe",
];

const GAMING_WHITELIST: &[&str] = &[
    "steam.exe",
    "steamwebhelper.exe",
    "EpicGamesLauncher.exe",
    "Battle.net.exe",
    "RiotClientServices.exe",
    "GalaxyClient.exe",
    "upc.exe",
    "UbisoftConnect.exe",
    "Origin.exe",
    "EADesktop.exe",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaming_merges_whitelist_soft_only() {
        let mut cfg = GuardianConfig::default();
        assert!(cfg.add_whitelist("mygame.exe".into()));
        let id = apply_profile(&mut cfg, "Gaming").unwrap();
        assert_eq!(id, PROFILE_GAMING);
        assert_eq!(cfg.active_profile, PROFILE_GAMING);
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        assert_eq!(cfg.idle_escalate_streak, 5);
        assert!(cfg.whitelist.iter().any(|w| w.eq_ignore_ascii_case("steam.exe")));
        assert!(cfg.whitelist.iter().any(|w| w.eq_ignore_ascii_case("mygame.exe")));
    }

    #[test]
    fn quiet_earlier_tripwires() {
        let mut cfg = GuardianConfig::default();
        apply_profile(&mut cfg, "quiet").unwrap();
        assert_eq!(cfg.disk_busy_soft_pct, 75.0);
        assert_eq!(cfg.disk_busy_hard_pct, 90.0);
        assert_eq!(cfg.mem_avail_soft_pct, 20.0);
        assert_eq!(cfg.max_soft_demote_secs, 30);
        assert_eq!(cfg.idle_escalate_streak, 3);
    }

    #[test]
    fn dev_restores_defaults() {
        let mut cfg = GuardianConfig::default();
        apply_profile(&mut cfg, "quiet").unwrap();
        apply_profile(&mut cfg, "dev").unwrap();
        assert_eq!(cfg.disk_busy_soft_pct, 85.0);
        assert_eq!(cfg.max_soft_demote_secs, 45);
        assert_eq!(cfg.idle_escalate_streak, 4);
        assert_eq!(cfg.active_profile, PROFILE_DEV);
    }

    #[test]
    fn unknown_profile_errors() {
        let mut cfg = GuardianConfig::default();
        assert!(apply_profile(&mut cfg, "boost").is_err());
    }
}
