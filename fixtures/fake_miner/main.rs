//! Decoy high-CPU process with miner-like name tokens for L4 proof.
//! Not a real miner — burns CPU in a tight loop and prints a stratum-like arg banner.

fn main() {
    let _args: Vec<String> = std::env::args().collect();
    eprintln!("fake-miner fixture (CPU burn). Args may include stratum+tcp for heuristic tests.");
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().max(1))
        .unwrap_or(2);
    let mut handles = Vec::new();
    for _ in 0..threads {
        handles.push(std::thread::spawn(|| {
            let mut x = 1u64;
            loop {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                std::hint::black_box(x);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
}
