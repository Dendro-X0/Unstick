//! System tray / status client for Unstick.
//!
//! Windows GUI subsystem — tray mode has no console. `--cli` / `status` attach one.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod client;

use std::time::Duration;

use anyhow::Result;
use guardian_core::{
    status_path, ClientRequest, DiskControlMode, PressureBand, ServerPush, StatusSnapshot,
};
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
fn attach_cli_console() {
    use windows::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = args.iter().any(|a| a == "--cli" || a == "status");

    if cli {
        #[cfg(windows)]
        attach_cli_console();
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
            .with_target(false)
            .init();
        return run_cli().await;
    }

    #[cfg(windows)]
    {
        if args.iter().any(|a| a == "--tray") || args.is_empty() {
            // Tray: file-less quiet; avoid creating a console for tracing.
            return run_tray().await;
        }
    }

    #[cfg(windows)]
    attach_cli_console();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_target(false)
        .init();
    run_cli().await
}

async fn run_cli() -> Result<()> {
    loop {
        match client::request(ClientRequest::GetStatus).await {
            Ok(ServerPush::Status(s)) => print_status(&s),
            Ok(other) => println!("{other:?}"),
            Err(_) => {
                // Fallback to status file written by service
                if let Ok(raw) = std::fs::read_to_string(status_path()) {
                    if let Ok(s) = serde_json::from_str::<StatusSnapshot>(&raw) {
                        print_status(&s);
                    } else {
                        println!("waiting for guardian-service...");
                    }
                } else {
                    println!("waiting for guardian-service (start guardian-service first)...");
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn print_status(s: &StatusSnapshot) {
    let band = match s.pressure_band {
        PressureBand::Normal => "normal",
        PressureBand::Warn => "warn",
        PressureBand::Throttle => "throttle",
        PressureBand::Emergency => "emergency",
    };
    let ctrl = tray_control_summary(s);
    println!(
        "[{}] score={:.2} cpu={:.0}% mem_avail={:.1}GB disk={:.0}% queue={:.1} ctrl={ctrl} paused={} uptime={}s",
        band,
        s.pressure_score,
        s.cpu_percent,
        s.memory_available_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        s.disk_busy_percent,
        s.disk_queue_length,
        s.paused,
        s.service_uptime_secs
    );
    for p in s.top_processes.iter().take(5) {
        println!(
            "  pid={:<6} cpu={:>5.1}%  {}",
            p.pid, p.cpu_percent, p.name
        );
    }
    if !s.recent_abuse.is_empty() {
        println!("  abuse alerts:");
        for a in &s.recent_abuse {
            println!("    pid={} {} score={} {:?}", a.pid, a.name, a.score, a.reasons);
        }
    }
    println!();
}

/// Live Soft control phrase for tray tooltip / CLI (not session totals).
fn tray_control_summary(s: &StatusSnapshot) -> String {
    if s.paused {
        return "paused".into();
    }
    let mut parts = Vec::new();
    for (label, mode, intensity) in [
        ("disk", s.disk_control_mode, s.disk_control_intensity),
        ("ram", s.mem_control_mode, s.mem_control_intensity),
    ] {
        match mode {
            DiskControlMode::Released => {}
            DiskControlMode::Holding => parts.push(format!("{label} hold i{intensity}")),
            DiskControlMode::Capping if intensity >= 3 => {
                parts.push(format!("{label} idle i{intensity}"))
            }
            DiskControlMode::Capping => parts.push(format!("{label} cap i{intensity}")),
        }
    }
    if parts.is_empty() {
        "monitoring".into()
    } else {
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayIconTone {
    Calm,
    Warn,
    Cap,
    Idle,
    Paused,
    Offline,
}

fn tray_icon_tone(s: Option<&StatusSnapshot>) -> TrayIconTone {
    let Some(s) = s else {
        return TrayIconTone::Offline;
    };
    if s.paused {
        return TrayIconTone::Paused;
    }
    let max_i = s.disk_control_intensity.max(s.mem_control_intensity);
    let capping = s.disk_control_mode == DiskControlMode::Capping
        || s.mem_control_mode == DiskControlMode::Capping;
    let holding = s.disk_control_mode == DiskControlMode::Holding
        || s.mem_control_mode == DiskControlMode::Holding;
    if capping && max_i >= 3 {
        return TrayIconTone::Idle;
    }
    if capping
        || matches!(
            s.pressure_band,
            PressureBand::Throttle | PressureBand::Emergency
        )
    {
        return TrayIconTone::Cap;
    }
    if holding || matches!(s.pressure_band, PressureBand::Warn) {
        return TrayIconTone::Warn;
    }
    TrayIconTone::Calm
}

#[cfg(windows)]
async fn run_tray() -> Result<()> {
    use std::sync::{Arc, Mutex};
    use tao::event::Event;
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let event_loop = EventLoopBuilder::new().build();

    let menu = Menu::new();
    let status_item = MenuItem::new("Status: starting...", false, None);
    let pause_item = MenuItem::new("Pause 15 min", true, None);
    let resume_item = MenuItem::new("Resume", true, None);
    let check_update_item = MenuItem::new("Check for updates", true, None);
    let install_update_item = MenuItem::new("Install update", true, None);
    let open_log_item = MenuItem::new("Open event log folder", true, None);
    let quit_item = MenuItem::new("Quit tray", true, None);
    menu.append(&status_item)?;
    menu.append(&pause_item)?;
    menu.append(&resume_item)?;
    menu.append(&check_update_item)?;
    menu.append(&install_update_item)?;
    menu.append(&open_log_item)?;
    menu.append(&quit_item)?;

    let icon = Icon::from_rgba(make_icon_rgba(32, TrayIconTone::Calm), 32, 32)?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Unstick")
        .with_icon(icon)
        .build()?;

    let last_status: Arc<Mutex<Option<StatusSnapshot>>> = Arc::new(Mutex::new(None));
    let last_status2 = last_status.clone();
    let status_item_id = status_item.id().clone();
    let pause_id = pause_item.id().clone();
    let resume_id = resume_item.id().clone();
    let check_update_id = check_update_item.id().clone();
    let install_update_id = install_update_item.id().clone();
    let open_id = open_log_item.id().clone();
    let quit_id = quit_item.id().clone();

    // Background status poller
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        rt.block_on(async move {
            loop {
                let snap = match client::request(ClientRequest::GetStatus).await {
                    Ok(ServerPush::Status(s)) => Some(s),
                    _ => std::fs::read_to_string(status_path())
                        .ok()
                        .and_then(|r| serde_json::from_str(&r).ok()),
                };
                if let Some(s) = snap {
                    if let Ok(mut g) = last_status2.lock() {
                        *g = Some(s);
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    });

    let menu_channel = MenuEvent::receiver();
    let mut prev_disk_hard = false;
    let mut prev_mem_hard = false;
    let mut prev_tone = TrayIconTone::Calm;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + Duration::from_millis(500),
        );

        if let Ok(ev) = menu_channel.try_recv() {
            let id = ev.id;
            if id == pause_id {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok();
                if let Some(rt) = rt {
                    let _ = rt.block_on(client::request(ClientRequest::Pause { minutes: 15 }));
                }
            } else if id == resume_id {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok();
                if let Some(rt) = rt {
                    let _ = rt.block_on(client::request(ClientRequest::Resume));
                }
            } else if id == check_update_id {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok();
                if let Some(rt) = rt {
                    let _ = rt.block_on(client::request(ClientRequest::CheckForUpdate));
                }
            } else if id == install_update_id {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok();
                if let Some(rt) = rt {
                    let _ = rt.block_on(client::request(ClientRequest::StartUpdate));
                }
            } else if id == open_id {
                let _ = open::that(guardian_core::config_dir());
            } else if id == quit_id {
                *control_flow = ControlFlow::Exit;
            } else if id == status_item_id {
                // no-op display
            }
        }

        let snap = last_status.lock().ok().and_then(|g| g.clone());
        let tone = tray_icon_tone(snap.as_ref());
        if tone != prev_tone {
            if let Ok(icon) = Icon::from_rgba(make_icon_rgba(32, tone), 32, 32) {
                let _ = tray.set_icon(Some(icon));
            }
            prev_tone = tone;
        }

        if let Some(s) = snap {
            let band = s.pressure_band.as_str();
            let ctrl = tray_control_summary(&s);
            let tip = if s.update_available {
                format!(
                    "Unstick [{band}] · {ctrl}\nUpdate v{} available · Check tray menu",
                    s.update_version
                )
            } else {
                format!(
                    "Unstick [{band}] · {ctrl}\nscore={:.2} · cpu={:.0}% · disk lock={} · mem lock={}",
                    s.pressure_score,
                    s.cpu_percent,
                    s.disk_lock.as_str(),
                    s.mem_lock.as_str()
                )
            };
            let _ = tray.set_tooltip(Some(tip));
            status_item.set_text(format!("[{band}] {ctrl} · cpu={:.0}%", s.cpu_percent));
            install_update_item.set_text(if s.update_available {
                format!("Install update v{}", s.update_version)
            } else {
                "Install update".into()
            });
            // P3-1: toast when Disk/Mem Lock enters HARD.
            use guardian_core::{DiskLockMode, MemLockMode};
            let disk_hard = s.disk_lock == DiskLockMode::Hard;
            let mem_hard = s.mem_lock == MemLockMode::Hard;
            if disk_hard && !prev_disk_hard {
                let _ = notify_rust::Notification::new()
                    .summary("Unstick — Disk Lock HARD")
                    .body("OS drive under heavy load; background I/O limited.")
                    .timeout(notify_rust::Timeout::Milliseconds(6000))
                    .show();
            }
            if mem_hard && !prev_mem_hard {
                let _ = notify_rust::Notification::new()
                    .summary("Unstick — Mem Lock HARD")
                    .body("RAM pressure with paging; trimming background working sets.")
                    .timeout(notify_rust::Timeout::Milliseconds(6000))
                    .show();
            }
            prev_disk_hard = disk_hard;
            prev_mem_hard = mem_hard;
        } else {
            let _ = tray.set_tooltip(Some("Unstick · waiting for service".to_string()));
            status_item.set_text("Status: waiting for service…");
        }

        if let Event::NewEvents(_) = &event {
            // poll menu/status on wake
        }
    });
}

#[cfg(windows)]
fn make_icon_rgba(size: u32, tone: TrayIconTone) -> Vec<u8> {
    let (r, g, b) = match tone {
        TrayIconTone::Calm => (20, 160, 140),    // teal
        TrayIconTone::Warn => (230, 141, 80),    // amber
        TrayIconTone::Cap => (220, 80, 80),      // coral — actively capping
        TrayIconTone::Idle => (180, 60, 120),    // magenta — Efficiency Idle
        TrayIconTone::Paused => (110, 120, 130), // slate
        TrayIconTone::Offline => (70, 75, 80),   // dim
    };
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let cx = size as f32 / 2.0;
            let cy = size as f32 / 2.0;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let inside = dx.abs() < cx * 0.55 && dy.abs() < cy * 0.7;
            if inside {
                rgba[i] = r;
                rgba[i + 1] = g;
                rgba[i + 2] = b;
                rgba[i + 3] = 255;
            } else {
                rgba[i + 3] = 0;
            }
        }
    }
    // Small corner pip for capping / idle so badge reads without opening UI.
    if matches!(tone, TrayIconTone::Cap | TrayIconTone::Idle) {
        let pip = (size * 3 / 4)..size;
        for y in pip.clone() {
            for x in pip.clone() {
                let i = ((y * size + x) * 4) as usize;
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use guardian_core::DiskControlMode;

    fn blank_status() -> StatusSnapshot {
        serde_json::from_str(
            r#"{"paused":false,"pressure_score":0.1,"pressure_band":"normal","cpu_percent":1.0,"memory_available_bytes":1,"memory_total_bytes":2,"disk_busy_percent":1.0,"disk_queue_length":0.0,"top_processes":[],"recent_throttles":[],"recent_abuse":[],"service_uptime_secs":1}"#,
        )
        .expect("minimal status")
    }

    #[test]
    fn control_summary_capping_and_idle() {
        let mut s = blank_status();
        assert_eq!(tray_control_summary(&s), "monitoring");
        s.disk_control_mode = DiskControlMode::Capping;
        s.disk_control_intensity = 2;
        assert_eq!(tray_control_summary(&s), "disk cap i2");
        s.disk_control_intensity = 3;
        assert_eq!(tray_control_summary(&s), "disk idle i3");
        s.paused = true;
        assert_eq!(tray_control_summary(&s), "paused");
    }

    #[test]
    fn icon_tone_follows_control() {
        let mut s = blank_status();
        assert_eq!(tray_icon_tone(Some(&s)), TrayIconTone::Calm);
        s.disk_control_mode = DiskControlMode::Capping;
        s.disk_control_intensity = 2;
        assert_eq!(tray_icon_tone(Some(&s)), TrayIconTone::Cap);
        s.disk_control_intensity = 3;
        assert_eq!(tray_icon_tone(Some(&s)), TrayIconTone::Idle);
        assert_eq!(tray_icon_tone(None), TrayIconTone::Offline);
    }
}
