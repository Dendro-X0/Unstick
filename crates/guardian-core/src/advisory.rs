//! Detect-only advisories for conditions Unstick cannot remediate.

use serde::{Deserialize, Serialize};

/// Elevated DPC/ISR — driver/hardware latency (MS PerfGuide investigate >20%).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DpcAdvisoryLevel {
    #[default]
    None,
    Warn,
    High,
}

impl DpcAdvisoryLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Warn => "warn",
            Self::High => "high",
        }
    }
}

/// Instantaneous severity from PDH % DPC / % Interrupt (before streak).
pub fn dpc_isr_raw_level(dpc_pct: f32, interrupt_pct: f32) -> DpcAdvisoryLevel {
    let dpc = dpc_pct.max(0.0);
    let irq = interrupt_pct.max(0.0);
    if dpc >= 20.0 || irq >= 20.0 {
        DpcAdvisoryLevel::High
    } else if dpc >= 10.0 || irq >= 10.0 || (dpc + irq) >= 15.0 {
        DpcAdvisoryLevel::Warn
    } else {
        DpcAdvisoryLevel::None
    }
}

/// Apply consecutive-sample streak (default 3) so brief spikes do not flicker.
pub fn classify_dpc_isr(
    dpc_pct: f32,
    interrupt_pct: f32,
    elevated_streak: u32,
    streak_needed: u32,
) -> DpcAdvisoryLevel {
    let raw = dpc_isr_raw_level(dpc_pct, interrupt_pct);
    if raw == DpcAdvisoryLevel::None {
        return DpcAdvisoryLevel::None;
    }
    if elevated_streak < streak_needed.max(1) {
        return DpcAdvisoryLevel::None;
    }
    raw
}

pub fn dpc_advisory_message(level: DpcAdvisoryLevel) -> Option<&'static str> {
    match level {
        DpcAdvisoryLevel::None => None,
        DpcAdvisoryLevel::Warn => Some(
            "Elevated DPC/ISR — may hitch UI/audio. Unstick cannot fix driver latency. Capture: wpr -start GeneralProfile -filemode, reproduce, wpr -stop trace.etl; open in WPA → DPC/ISR by module. Update chipset/network/audio/GPU drivers.",
        ),
        DpcAdvisoryLevel::High => Some(
            "High DPC/ISR (>20%) — threads starved by interrupts/DPCs. Not remediable in user mode. Use WPR/WPA DPC/ISR graphs to identify the driver (.sys), then update/roll back that driver.",
        ),
    }
}

/// Cooling mode from SYSTEM_POWER_INFORMATION.CoolingMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CoolingMode {
    #[default]
    Unknown,
    Active,
    Passive,
}

impl CoolingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Active => "active",
            Self::Passive => "passive",
        }
    }

    pub fn from_po_tz(mode: u8) -> Self {
        match mode {
            0 => Self::Active,  // PO_TZ_ACTIVE
            1 => Self::Passive, // PO_TZ_PASSIVE
            _ => Self::Unknown, // PO_TZ_INVALID_MODE = 2
        }
    }
}

/// Apple-shaped thermal/power level for Windows proxies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThermalLevel {
    #[default]
    Nominal,
    Fair,
    Serious,
}

impl ThermalLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::Fair => "fair",
            Self::Serious => "serious",
        }
    }

    /// Stall contribution 0..1.
    pub fn thermal_some(self) -> f32 {
        match self {
            Self::Nominal => 0.0,
            Self::Fair => 0.35,
            Self::Serious => 0.70,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThermalPowerInputs {
    pub on_battery: bool,
    pub battery_percent: Option<u8>,
    pub cooling: CoolingMode,
    /// CurrentMhz / MaxMhz average (1.0 = full clocks; 0 if unknown).
    pub cpu_mhz_ratio: f32,
}

pub fn classify_thermal_power(inp: &ThermalPowerInputs) -> ThermalLevel {
    let ratio = if inp.cpu_mhz_ratio > 0.0 {
        inp.cpu_mhz_ratio.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let low_battery = inp
        .battery_percent
        .map(|p| inp.on_battery && p <= 20)
        .unwrap_or(false);
    let serious_clock = inp.cooling == CoolingMode::Passive && ratio < 0.70;
    if low_battery || serious_clock {
        return ThermalLevel::Serious;
    }
    if inp.on_battery || inp.cooling == CoolingMode::Passive || ratio < 0.85 {
        return ThermalLevel::Fair;
    }
    ThermalLevel::Nominal
}

pub fn thermal_advisory_message(level: ThermalLevel) -> Option<&'static str> {
    match level {
        ThermalLevel::Nominal => None,
        ThermalLevel::Fair => Some(
            "Power/thermal limiting (battery or reduced CPU clocks). Prefer soft throttle; Suspend is less helpful while the system is thermally constrained.",
        ),
        ThermalLevel::Serious => Some(
            "Serious power/thermal constraint — Suspend disabled. Ease background work only; let the machine cool or plug in AC.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_thresholds() {
        assert_eq!(dpc_isr_raw_level(5.0, 5.0), DpcAdvisoryLevel::None);
        assert_eq!(dpc_isr_raw_level(12.0, 0.0), DpcAdvisoryLevel::Warn);
        assert_eq!(dpc_isr_raw_level(8.0, 8.0), DpcAdvisoryLevel::Warn); // sum 16
        assert_eq!(dpc_isr_raw_level(21.0, 0.0), DpcAdvisoryLevel::High);
        assert_eq!(dpc_isr_raw_level(0.0, 25.0), DpcAdvisoryLevel::High);
    }

    #[test]
    fn streak_gates_advisory() {
        assert_eq!(
            classify_dpc_isr(25.0, 0.0, 1, 3),
            DpcAdvisoryLevel::None
        );
        assert_eq!(
            classify_dpc_isr(25.0, 0.0, 3, 3),
            DpcAdvisoryLevel::High
        );
        assert_eq!(
            classify_dpc_isr(5.0, 5.0, 99, 3),
            DpcAdvisoryLevel::None
        );
    }

    #[test]
    fn high_has_message_none_does_not() {
        assert!(dpc_advisory_message(DpcAdvisoryLevel::High).is_some());
        assert!(dpc_advisory_message(DpcAdvisoryLevel::None).is_none());
    }

    #[test]
    fn thermal_nominal_on_ac_full_clocks() {
        let level = classify_thermal_power(&ThermalPowerInputs {
            on_battery: false,
            battery_percent: None,
            cooling: CoolingMode::Active,
            cpu_mhz_ratio: 1.0,
        });
        assert_eq!(level, ThermalLevel::Nominal);
    }

    #[test]
    fn thermal_fair_on_battery() {
        let level = classify_thermal_power(&ThermalPowerInputs {
            on_battery: true,
            battery_percent: Some(80),
            cooling: CoolingMode::Active,
            cpu_mhz_ratio: 1.0,
        });
        assert_eq!(level, ThermalLevel::Fair);
    }

    #[test]
    fn thermal_serious_low_battery_or_passive_throttle() {
        assert_eq!(
            classify_thermal_power(&ThermalPowerInputs {
                on_battery: true,
                battery_percent: Some(15),
                cooling: CoolingMode::Active,
                cpu_mhz_ratio: 1.0,
            }),
            ThermalLevel::Serious
        );
        assert_eq!(
            classify_thermal_power(&ThermalPowerInputs {
                on_battery: false,
                battery_percent: None,
                cooling: CoolingMode::Passive,
                cpu_mhz_ratio: 0.60,
            }),
            ThermalLevel::Serious
        );
    }
}
