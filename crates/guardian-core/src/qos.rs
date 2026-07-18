//! Portable QoS / App Nap control plane (Apple Energy Efficiency Guide).
//!
//! Windows maps these intents onto priority / SoftOnly vs LastResort.
//! Darwin apply lives in `guardian-mac`.

use serde::{Deserialize, Serialize};

use crate::advisory::ThermalLevel;
use crate::config::CriticalGuardMode;
use crate::pressure::{DiskLockMode, MemLockMode, PressureBand};
use crate::types::{FocusProfile, ThrottleLevel};

/// Quality-of-service class (Apple NSQualityOfService / GCD QoS analogue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QosClass {
    UserInteractive,
    UserInitiated,
    Utility,
    Background,
    #[default]
    Default,
}

impl QosClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserInteractive => "user_interactive",
            Self::UserInitiated => "user_initiated",
            Self::Utility => "utility",
            Self::Background => "background",
            Self::Default => "default",
        }
    }

    /// Map to the Windows soft-throttle ladder.
    pub fn to_throttle_level(self) -> ThrottleLevel {
        match self {
            Self::UserInteractive | Self::UserInitiated | Self::Default => ThrottleLevel::None,
            Self::Utility => ThrottleLevel::BelowNormal,
            Self::Background => ThrottleLevel::Idle,
        }
    }
}

/// Whether Unstick should cooperate with OS idle/Nap or force a pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NapPolicy {
    /// Soft throttle / QoS only — App Nap–compatible default.
    #[default]
    Cooperate,
    /// Last-resort pause analogue (NtSuspend on Windows; avoided on macOS).
    ForcePause,
}

impl NapPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cooperate => "cooperate",
            Self::ForcePause => "force_pause",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QosPlan {
    pub focus: QosClass,
    pub background: QosClass,
    pub nap: NapPolicy,
}

/// Plan QoS / Nap from Guard mode, pressure, focus profile, and thermal level.
pub fn plan_qos(
    mode: CriticalGuardMode,
    band: PressureBand,
    disk_lock: DiskLockMode,
    mem_lock: MemLockMode,
    focus: FocusProfile,
    thermal: ThermalLevel,
    allow_force_pause: bool,
) -> QosPlan {
    let focus_qos = match focus {
        FocusProfile::Play | FocusProfile::Dev => QosClass::UserInteractive,
        FocusProfile::Other => QosClass::UserInitiated,
    };

    let hard = matches!(band, PressureBand::Emergency)
        || disk_lock == DiskLockMode::Hard
        || mem_lock == MemLockMode::Hard;
    let soft_lock = disk_lock == DiskLockMode::Soft || mem_lock == MemLockMode::Soft;
    let background = if hard || matches!(band, PressureBand::Throttle) || soft_lock {
        if hard {
            QosClass::Background
        } else {
            QosClass::Utility
        }
    } else if matches!(band, PressureBand::Warn) {
        QosClass::Default
    } else {
        QosClass::Default
    };

    let nap = match mode {
        CriticalGuardMode::SoftOnly => NapPolicy::Cooperate,
        CriticalGuardMode::LastResortSuspend => {
            if thermal == ThermalLevel::Serious {
                NapPolicy::Cooperate
            } else if allow_force_pause && hard {
                NapPolicy::ForcePause
            } else {
                NapPolicy::Cooperate
            }
        }
    };

    QosPlan {
        focus: focus_qos,
        background,
        nap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_only_never_force_pause() {
        let q = plan_qos(
            CriticalGuardMode::SoftOnly,
            PressureBand::Emergency,
            DiskLockMode::Hard,
            MemLockMode::Off,
            FocusProfile::Dev,
            ThermalLevel::Nominal,
            true,
        );
        assert_eq!(q.nap, NapPolicy::Cooperate);
        assert_eq!(q.focus, QosClass::UserInteractive);
        assert_eq!(q.background, QosClass::Background);
    }

    #[test]
    fn last_resort_can_force_pause_when_allowed() {
        let q = plan_qos(
            CriticalGuardMode::LastResortSuspend,
            PressureBand::Emergency,
            DiskLockMode::Off,
            MemLockMode::Off,
            FocusProfile::Other,
            ThermalLevel::Nominal,
            true,
        );
        assert_eq!(q.nap, NapPolicy::ForcePause);
        assert_eq!(q.focus, QosClass::UserInitiated);
    }

    #[test]
    fn serious_thermal_blocks_force_pause() {
        let q = plan_qos(
            CriticalGuardMode::LastResortSuspend,
            PressureBand::Emergency,
            DiskLockMode::Hard,
            MemLockMode::Off,
            FocusProfile::Play,
            ThermalLevel::Serious,
            true,
        );
        assert_eq!(q.nap, NapPolicy::Cooperate);
    }

    #[test]
    fn background_qos_maps_to_idle() {
        assert_eq!(
            QosClass::Background.to_throttle_level(),
            ThrottleLevel::Idle
        );
        assert_eq!(
            QosClass::Utility.to_throttle_level(),
            ThrottleLevel::BelowNormal
        );
    }
}
