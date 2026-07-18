//! Decoy high-RSS process for Mem Lock L3 proof.
//! Allocates a large working set and holds it until Ctrl+C / kill.

use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let mb: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(512)
        .clamp(64, 8192);
    let bytes = mb * 1024 * 1024;
    eprintln!("mem-hog fixture: allocating ~{mb} MiB (pid {})", std::process::id());
    let mut buf = vec![0u8; bytes];
    // Touch pages so they commit into the working set.
    for chunk in buf.chunks_mut(4096) {
        chunk[0] = 0xA5;
    }
    eprintln!(
        "mem-hog ready: {} MiB touched; holding idle (no page re-touch)",
        mb
    );
    let _ = io::stdout().flush();
    // Keep allocation alive; do not re-fault pages so EmptyWorkingSet stays visible.
    let _keep = buf;
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
