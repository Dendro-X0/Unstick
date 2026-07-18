//! Mapped-I/O / large-file fault stress for Mem Lock L4 false-positive proof.
//! Allocates a large anonymous buffer and re-touches every page in a loop so
//! the OS sees sustained memory activity similar to IDE/git working sets,
//! without driving pagefile thrash (no commit beyond the buffer).

use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let mb: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256)
        .clamp(64, 2048);
    let secs: u64 = env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0); // 0 = until killed

    let bytes = mb * 1024 * 1024;
    eprintln!(
        "mapped-io-hog: allocating ~{mb} MiB and re-touching pages (pid {})",
        std::process::id()
    );
    let mut buf = vec![0u8; bytes];
    // Initial touch so pages are resident.
    for chunk in buf.chunks_mut(4096) {
        chunk[0] = 1;
    }
    eprintln!("mapped-io-hog ready");
    let _ = io::stdout().flush();

    let start = Instant::now();
    let mut pass = 0u64;
    loop {
        for chunk in buf.chunks_mut(4096) {
            chunk[0] = chunk[0].wrapping_add(1);
        }
        pass += 1;
        if pass % 16 == 0 {
            eprintln!("mapped-io-hog: passes={pass}");
        }
        if secs > 0 && start.elapsed() >= Duration::from_secs(secs) {
            break;
        }
        // Brief yield so PDH samples see activity without pegging one core 100%.
        thread::sleep(Duration::from_millis(25));
    }
}
