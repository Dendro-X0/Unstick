//! macOS / Darwin QoS + App Nap apply surface.
//!
//! Off Darwin this crate exports stubs (`supported() -> false`).
//! Live pthread/GCD/NSProcessInfo apply lands when building on macOS.

use guardian_core::{NapPolicy, QosClass};

/// Whether Darwin QoS / App Nap apply is available on this build target.
pub fn supported() -> bool {
    cfg!(target_os = "macos")
}

/// Apply a QoS class to the current thread (Darwin). Stub elsewhere.
pub fn apply_thread_qos(_class: QosClass) -> Result<(), String> {
    if !supported() {
        return Err("guardian-mac: QoS apply requires macOS".into());
    }
    // Future: pthread_set_qos_class_self_np / qos_class_t mapping.
    Ok(())
}

/// Begin or end an App Nap–compatible activity for the process.
///
/// `Cooperate` → allow Nap (end user-activity assertion).
/// `ForcePause` is intentionally not mapped to a Suspend analogue on Darwin.
pub fn apply_nap_policy(policy: NapPolicy) -> Result<(), String> {
    if !supported() {
        return Err("guardian-mac: App Nap policy requires macOS".into());
    }
    match policy {
        NapPolicy::Cooperate => {
            // Future: end NSProcessInfo activity / allow App Nap.
            Ok(())
        }
        NapPolicy::ForcePause => {
            // Design: never invent NtSuspend on macOS; lower QoS only.
            Err("guardian-mac: ForcePause is Windows-only; use Background QoS".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_off_macos() {
        assert_eq!(supported(), cfg!(target_os = "macos"));
        if !cfg!(target_os = "macos") {
            assert!(apply_thread_qos(QosClass::UserInteractive).is_err());
            assert!(apply_nap_policy(NapPolicy::Cooperate).is_err());
        }
    }
}
