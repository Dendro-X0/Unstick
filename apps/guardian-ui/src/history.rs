//! Rolling metric samples for sparklines (~60 s at 1 Hz).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const CAPACITY: usize = 60;

#[derive(Clone, Default)]
pub struct MetricHistory {
    cpu: VecDeque<f32>,
    disk: VecDeque<f32>,
    last_push: Option<Instant>,
}

impl MetricHistory {
    pub fn push_sample(&mut self, cpu_percent: f32, disk_percent: f32) {
        let now = Instant::now();
        if let Some(last) = self.last_push {
            if now.duration_since(last) < Duration::from_secs(1) {
                return;
            }
        }
        self.cpu.push_back(cpu_percent.clamp(0.0, 100.0));
        self.disk.push_back(disk_percent.clamp(0.0, 100.0));
        while self.cpu.len() > CAPACITY {
            self.cpu.pop_front();
        }
        while self.disk.len() > CAPACITY {
            self.disk.pop_front();
        }
        self.last_push = Some(now);
    }

    pub fn cpu_slice(&self) -> Vec<f32> {
        self.cpu.iter().copied().collect()
    }

    pub fn disk_slice(&self) -> Vec<f32> {
        self.disk.iter().copied().collect()
    }
}
