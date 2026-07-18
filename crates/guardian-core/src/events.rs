//! Persistent event log helpers (`events.jsonl`).

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::config::events_path;
use crate::types::GuardianEvent;

/// Read up to `limit` newest events from the JSONL log (newest first).
pub fn read_recent_events(limit: usize) -> Vec<GuardianEvent> {
    let limit = limit.max(1).min(200);
    let path = events_path();
    let Ok(mut file) = File::open(&path) else {
        return Vec::new();
    };
    let Ok(meta) = file.metadata() else {
        return Vec::new();
    };
    let len = meta.len();
    // Read a tail window (~256 KiB) then parse lines.
    let window = 256 * 1024u64;
    let start = len.saturating_sub(window);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();
    if start > 0 && !lines.is_empty() {
        // First line may be a partial JSON fragment — drop it.
        lines.remove(0);
    }
    let mut out = Vec::new();
    for line in lines.iter().rev() {
        if let Ok(ev) = serde_json::from_str::<GuardianEvent>(line) {
            out.push(ev);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// Prefer in-memory ring, fall back to `events.jsonl` after restart.
pub fn recent_events_for_client(mem: &[GuardianEvent], limit: usize) -> Vec<GuardianEvent> {
    let limit = limit.max(1).min(200);
    if !mem.is_empty() {
        return mem.iter().rev().take(limit).cloned().collect();
    }
    read_recent_events(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Write;

    #[test]
    fn parses_jsonl_tail() {
        let dir = std::env::temp_dir().join(format!(
            "unstick-events-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("events.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..5 {
            let ev = GuardianEvent::Info {
                message: format!("msg-{i}"),
                at: Utc::now(),
            };
            writeln!(f, "{}", serde_json::to_string(&ev).unwrap()).unwrap();
        }
        // Point events_path via env? config_dir uses LOCALAPPDATA — unit test
        // the parser path by reading the file we wrote directly.
        let raw = std::fs::read_to_string(&path).unwrap();
        let mut parsed = Vec::new();
        for line in raw.lines().rev() {
            if let Ok(ev) = serde_json::from_str::<GuardianEvent>(line) {
                parsed.push(ev);
            }
        }
        assert_eq!(parsed.len(), 5);
        match &parsed[0] {
            GuardianEvent::Info { message, .. } => assert_eq!(message, "msg-4"),
            _ => panic!("expected Info"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mem_preferred_over_empty_file() {
        let mem = vec![GuardianEvent::Info {
            message: "live".into(),
            at: Utc::now(),
        }];
        let out = recent_events_for_client(&mem, 10);
        assert_eq!(out.len(), 1);
    }
}
