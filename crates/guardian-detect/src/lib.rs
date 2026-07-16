//! Behavioral abuse / cryptominer heuristics.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Utc;
use guardian_core::{GuardianConfig, GuardianEvent, ProcessSample, SystemSample};

#[derive(Debug, Clone)]
pub struct AbuseHit {
    pub pid: u32,
    pub name: String,
    pub score: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct PidTrack {
    high_cpu_since: Option<Instant>,
    last_score: u32,
    build_cooldown_until: Option<Instant>,
}

pub struct AbuseDetector {
    tracks: HashMap<u32, PidTrack>,
    allow_paths: Vec<String>,
    whitelist: Vec<String>,
    trusted_pids: Vec<u32>,
}

impl AbuseDetector {
    pub fn new(cfg: &GuardianConfig) -> Self {
        Self {
            tracks: HashMap::new(),
            allow_paths: cfg.allow_paths.clone(),
            whitelist: cfg.whitelist.clone(),
            trusted_pids: cfg.trusted_pids.clone(),
        }
    }

    pub fn reload_trust(&mut self, cfg: &GuardianConfig) {
        self.allow_paths = cfg.allow_paths.clone();
        self.whitelist = cfg.whitelist.clone();
        self.trusted_pids = cfg.trusted_pids.clone();
    }

    pub fn trust_pid(&mut self, pid: u32) {
        if !self.trusted_pids.contains(&pid) {
            self.trusted_pids.push(pid);
        }
    }

    pub fn evaluate(&mut self, sample: &SystemSample) -> Vec<AbuseHit> {
        let now = Instant::now();
        let live: std::collections::HashSet<u32> =
            sample.processes.iter().map(|p| p.pid).collect();
        self.tracks.retain(|pid, _| live.contains(pid));

        let mut hits = Vec::new();
        for proc in &sample.processes {
            if self.is_allowed(proc) {
                // Mark build-like trees for miner cool-down
                if is_toolchain(proc) {
                    let entry = self.tracks.entry(proc.pid).or_insert(PidTrack {
                        high_cpu_since: None,
                        last_score: 0,
                        build_cooldown_until: None,
                    });
                    entry.build_cooldown_until = Some(now + Duration::from_secs(30 * 60));
                }
                continue;
            }

            let (score, reasons) = self.score_process(proc, now);
            let entry = self.tracks.entry(proc.pid).or_insert(PidTrack {
                high_cpu_since: None,
                last_score: 0,
                build_cooldown_until: None,
            });

            if let Some(until) = entry.build_cooldown_until {
                if until > now && score < 90 {
                    // Suppress miner-like alerts during known builds unless extreme
                    continue;
                }
            }

            entry.last_score = score;
            if score >= 70 {
                hits.push(AbuseHit {
                    pid: proc.pid,
                    name: proc.name.clone(),
                    score,
                    reasons,
                });
            }
        }
        hits
    }

    pub fn to_events(hits: &[AbuseHit]) -> Vec<GuardianEvent> {
        hits.iter()
            .map(|h| GuardianEvent::Abuse {
                pid: h.pid,
                name: h.name.clone(),
                score: h.score,
                reasons: h.reasons.clone(),
                at: Utc::now(),
            })
            .collect()
    }

    fn is_allowed(&self, proc: &ProcessSample) -> bool {
        if self.trusted_pids.contains(&proc.pid) {
            return true;
        }
        let name = proc.name.to_lowercase();
        if self.whitelist.iter().any(|w| {
            let w = w.to_lowercase();
            name == w || name.contains(&w) || w.contains(&name)
        }) {
            return true;
        }
        if let Some(path) = &proc.path {
            let p = path.to_lowercase();
            if self
                .allow_paths
                .iter()
                .any(|a| p.contains(&a.to_lowercase()))
            {
                return true;
            }
            if self.whitelist.iter().any(|w| p.contains(&w.to_lowercase())) {
                return true;
            }
        }
        is_toolchain(proc)
    }

    fn score_process(&mut self, proc: &ProcessSample, now: Instant) -> (u32, Vec<String>) {
        let mut score: u32 = 0;
        let mut reasons = Vec::new();
        let name = proc.name.to_lowercase();

        // Sustained high CPU
        let track = self.tracks.entry(proc.pid).or_insert(PidTrack {
            high_cpu_since: None,
            last_score: 0,
            build_cooldown_until: None,
        });
        if proc.cpu_percent >= 85.0 {
            if track.high_cpu_since.is_none() {
                track.high_cpu_since = Some(now);
            }
            if let Some(since) = track.high_cpu_since {
                if now.duration_since(since) >= Duration::from_secs(120) {
                    score += 35;
                    reasons.push("sustained_high_cpu".into());
                }
            }
        } else {
            track.high_cpu_since = None;
        }

        let io = proc.disk_read_bytes_per_sec + proc.disk_write_bytes_per_sec;
        if proc.cpu_percent >= 80.0 && io < 50_000 {
            score += 15;
            reasons.push("high_cpu_low_disk".into());
        }

        if let Some(path) = &proc.path {
            let p = path.to_lowercase();
            if p.contains(r"\temp\")
                || p.contains(r"\appdata\local\temp\")
                || p.contains(r"\appdata\roaming\")
            {
                // Exclude known app folders under roaming that are legit
                if !p.contains(r"\npm\") && !p.contains(r"\cursor\") {
                    score += 20;
                    reasons.push("suspicious_path".into());
                }
            }
        }

        let script_hosts = ["wscript.exe", "cscript.exe", "mshta.exe", "powershell.exe"];
        if script_hosts.iter().any(|h| name == *h) {
            if let Some(cmd) = &proc.cmd_line {
                let c = cmd.to_lowercase();
                if c.contains("-enc") || c.contains("-encodedcommand") || c.contains("frombase64") {
                    score += 25;
                    reasons.push("encoded_script_host".into());
                } else if proc.cpu_percent >= 70.0 {
                    score += 15;
                    reasons.push("hot_script_host".into());
                }
            } else if proc.cpu_percent >= 70.0 {
                score += 15;
                reasons.push("hot_script_host".into());
            }
        }

        let miner_tokens = [
            "xmrig",
            "stratum+tcp",
            "stratum+ssl",
            "mining pool",
            "nicehash",
            "monero",
            "minerd",
            "cpuminer",
        ];
        let hay = format!(
            "{} {} {}",
            name,
            proc.path.clone().unwrap_or_default().to_lowercase(),
            proc.cmd_line.clone().unwrap_or_default().to_lowercase()
        );
        for tok in miner_tokens {
            if hay.contains(tok) {
                score += 40;
                reasons.push(format!("miner_token:{tok}"));
                break;
            }
        }

        // Parent anomaly is approximate: browsers/office as parent names in sample
        // (full parent lookup done by caller attaching parent_pid; we check name patterns via cmd)
        if proc.parent_pid != 0 && proc.cpu_percent >= 70.0 {
            // Points applied lightly; service may enrich
            if name.ends_with(".tmp.exe") || name.chars().filter(|c| c.is_ascii_hexdigit()).count() > 8 {
                score += 10;
                reasons.push("suspicious_name".into());
            }
        }

        (score.min(100), reasons)
    }
}

fn is_toolchain(proc: &ProcessSample) -> bool {
    let name = proc.name.to_lowercase();
    matches!(
        name.as_str(),
        "cargo.exe"
            | "rustc.exe"
            | "rustdoc.exe"
            | "cl.exe"
            | "link.exe"
            | "msbuild.exe"
            | "dotnet.exe"
            | "npm.exe"
            | "pnpm.exe"
            | "yarn.exe"
            | "node.exe"
            | "python.exe"
            | "pip.exe"
            | "git.exe"
            | "cmake.exe"
            | "ninja.exe"
    )
}

/// Enrich hits when parent looks like browser/Office.
pub fn apply_parent_anomaly(hit_score: u32, parent_name: Option<&str>) -> (u32, Option<String>) {
    let Some(parent) = parent_name.map(|s| s.to_lowercase()) else {
        return (hit_score, None);
    };
    let browsers = [
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "brave.exe",
        "opera.exe",
        "winword.exe",
        "excel.exe",
        "powerpnt.exe",
        "outlook.exe",
    ];
    if browsers.iter().any(|b| parent == *b) {
        ((hit_score + 20).min(100), Some("parent_anomaly".into()))
    } else {
        (hit_score, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use guardian_core::SystemSample;

    fn sample(procs: Vec<ProcessSample>) -> SystemSample {
        SystemSample {
            timestamp: Utc::now(),
            cpu_percent: 50.0,
            memory_total_bytes: 8 << 30,
            memory_available_bytes: 4 << 30,
            memory_commit_percent: 40.0,
            disk_busy_percent: 10.0,
            disk_queue_length: 0.1,
            disk_io_bytes_per_sec: 0,
            hard_faults_per_sec: 1.0,
            processes: procs,
        }
    }

    #[test]
    fn flags_xmrig_name() {
        let cfg = GuardianConfig::default();
        let mut det = AbuseDetector::new(&cfg);
        let s = sample(vec![ProcessSample {
            pid: 9,
            parent_pid: 1,
            name: "xmrig.exe".into(),
            path: Some(r"C:\Users\x\AppData\Local\Temp\xmrig.exe".into()),
            cpu_percent: 90.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: Some("xmrig --url stratum+tcp://pool".into()),
        }]);
        let hits = det.evaluate(&s);
        assert!(!hits.is_empty());
        assert!(hits[0].score >= 70);
    }

    #[test]
    fn does_not_flag_cargo() {
        let cfg = GuardianConfig::default();
        let mut det = AbuseDetector::new(&cfg);
        let s = sample(vec![ProcessSample {
            pid: 11,
            parent_pid: 1,
            name: "cargo.exe".into(),
            path: Some(r"C:\Users\x\.cargo\bin\cargo.exe".into()),
            cpu_percent: 95.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 10_000_000,
            disk_write_bytes_per_sec: 10_000_000,
            cmd_line: Some("cargo build".into()),
        }]);
        let hits = det.evaluate(&s);
        assert!(hits.is_empty());
    }
}
