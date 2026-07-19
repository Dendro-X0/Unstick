//! Decoy sequential disk writer for Disk Lock / hardware-control L3 soak.
//! Writes/reads a temp file (prefer OS volume via TEMP) to raise Active Time.
//!
//! Usage:
//!   cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml -- [MiB] [secs]
//!   cargo run --release --manifest-path fixtures/disk_hog/Cargo.toml -- cliff
//!
//! Defaults: 1024 MiB × 180s. `cliff` preset: 2048 MiB × 300s (freeze-cliff soak).
//! Not a product feature — soak / proof only.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (mb, secs, preset) = parse_args(&args);

    let dir = env::temp_dir().join("unstick-disk-hog");
    let _ = fs::create_dir_all(&dir);
    let path: PathBuf = dir.join(format!("hog-{}.bin", std::process::id()));
    eprintln!(
        "disk-hog{}: {} MiB file {} for ~{}s (pid {})",
        preset,
        mb,
        path.display(),
        secs,
        std::process::id()
    );
    eprintln!("  tip: keep Guard LIVE; watch tripwire monitoring vs soft capping");

    let bytes = mb * 1024 * 1024;
    let chunk = vec![0x5Au8; 1024 * 1024];
    let deadline = Instant::now() + Duration::from_secs(secs);

    while Instant::now() < deadline {
        {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .expect("create hog file");
            let mut written = 0usize;
            while written < bytes {
                f.write_all(&chunk).expect("write");
                written += chunk.len();
            }
            let _ = f.flush();
        }
        {
            let mut f = File::open(&path).expect("reopen");
            let mut buf = vec![0u8; 1024 * 1024];
            let mut read_total = 0usize;
            while read_total < bytes {
                match f.read(&mut buf) {
                    Ok(0) => {
                        let _ = f.seek(SeekFrom::Start(0));
                    }
                    Ok(n) => read_total += n,
                    Err(_) => break,
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    let _ = fs::remove_file(&path);
    eprintln!("disk-hog done");
}

fn parse_args(args: &[String]) -> (usize, u64, &'static str) {
    if args.first().map(|s| s.as_str()) == Some("cliff") {
        // ≥1 GiB and ≥2–5 min — freeze-cliff soak preset (roadmap S1).
        return (2048, 300, " [cliff]");
    }
    if args.first().map(|s| s.as_str()) == Some("help")
        || args.first().map(|s| s.as_str()) == Some("--help")
        || args.first().map(|s| s.as_str()) == Some("-h")
    {
        eprintln!(
            "disk-hog [MiB] [secs] | disk-hog cliff\n\
             defaults: 1024 MiB, 180s (max MiB 8192, max secs 1800)\n\
             cliff:    2048 MiB, 300s"
        );
        std::process::exit(0);
    }

    let mb: usize = args
        .first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024)
        .clamp(64, 8192);
    let secs: u64 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(180)
        .clamp(15, 1800);
    (mb, secs, "")
}
