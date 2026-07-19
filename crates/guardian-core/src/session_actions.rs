//! Session Soft-action aggregates (v0.7) — classification helpers for status counters.

/// Reasons that Monitor labels as **capped** (disk/mem control or Disk/Mem Lock).
pub fn is_session_capped_reason(reason: &str) -> bool {
    reason.starts_with("disk_control:")
        || reason.starts_with("mem_control:")
        || reason.starts_with("disk_lock:")
        || reason.starts_with("mem_lock:")
}

pub fn is_efficiency_idle_reason(reason: &str) -> bool {
    reason.contains("efficiency_idle")
}

pub fn is_soft_restore_reason(reason: &str) -> bool {
    reason.starts_with("soft_restore:")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionActionCounters {
    pub capped: u32,
    pub efficiency_idle: u32,
    pub restored: u32,
    pub suspended: u32,
    pub resumed: u32,
}

impl SessionActionCounters {
    pub fn note_throttle_apply(&mut self, reason: &str) {
        if !is_session_capped_reason(reason) {
            return;
        }
        self.capped = self.capped.saturating_add(1);
        if is_efficiency_idle_reason(reason) {
            self.efficiency_idle = self.efficiency_idle.saturating_add(1);
        }
    }

    pub fn note_soft_restore_ok(&mut self) {
        self.restored = self.restored.saturating_add(1);
    }

    pub fn note_suspend(&mut self) {
        self.suspended = self.suspended.saturating_add(1);
    }

    /// Hard Suspend resume (not Soft demotion restore).
    pub fn note_hard_resume(&mut self) {
        self.resumed = self.resumed.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_and_idle_classification() {
        let mut c = SessionActionCounters::default();
        c.note_throttle_apply("disk_control:ecoqos");
        c.note_throttle_apply("disk_control:efficiency_idle");
        c.note_throttle_apply("pressure:soft"); // ignored
        c.note_throttle_apply("mem_lock:soft");
        assert_eq!(c.capped, 3);
        assert_eq!(c.efficiency_idle, 1);
    }

    #[test]
    fn restore_and_suspend_paths() {
        let mut c = SessionActionCounters::default();
        c.note_soft_restore_ok();
        c.note_soft_restore_ok();
        c.note_suspend();
        c.note_hard_resume();
        assert_eq!(c.restored, 2);
        assert_eq!(c.suspended, 1);
        assert_eq!(c.resumed, 1);
        assert!(is_soft_restore_reason("soft_restore:left_plan"));
        assert!(!is_soft_restore_reason("max_suspend_secs"));
    }
}
