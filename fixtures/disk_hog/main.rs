//! Decoy sequential disk writer for Disk Lock L3 proof.
//! Writes/reads a temp file on the OS volume to raise Active Time.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let mb: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256)
        .clamp(64, 4096);
    let secs: u64 = env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(45);

    let dir = env::temp_dir().join("unstick-disk-hog");
    let _ = fs::create_dir_all(&dir);
    let path: PathBuf = dir.join(format!("hog-{}.bin", std::process::id()));
    eprintln!(
        "disk-hog: {} MiB file {} for ~{}s (pid {})",
        mb,
        path.display(),
        secs,
        std::process::id()
    );

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
