//! Windows sensors and soft-throttle executors.

mod sensors;
mod throttle;

pub use sensors::WinSensor;
pub use throttle::{elevation_likely, ApplyOutcome, SuspendEntry, SuspendLedger, ThrottleExecutor};
