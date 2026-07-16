//! System tray / status client for Unstick.

mod client;

use std::time::Duration;

use anyhow::Result;
use guardian_core::{status_path, ClientRequest, PressureBand, ServerPush, StatusSnapshot};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--cli" || a == "status") {
        return run_cli().await;
    }

    #[cfg(windows)]
    {
        if args.iter().any(|a| a == "--tray") || args.is_empty() {
            return run_tray().await;
        }
    }

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
    println!(
        "[{}] score={:.2} cpu={:.0}% mem_avail={:.1}GB disk={:.0}% queue={:.1} paused={} uptime={}s",
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
    let open_log_item = MenuItem::new("Open event log folder", true, None);
    let quit_item = MenuItem::new("Quit tray", true, None);
    menu.append(&status_item)?;
    menu.append(&pause_item)?;
    menu.append(&resume_item)?;
    menu.append(&open_log_item)?;
    menu.append(&quit_item)?;

    let icon = Icon::from_rgba(make_icon_rgba(32), 32, 32)?;
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
            } else if id == open_id {
                let _ = open::that(guardian_core::config_dir());
            } else if id == quit_id {
                *control_flow = ControlFlow::Exit;
            } else if id == status_item_id {
                // no-op display
            }
        }

        if let Some(s) = last_status.lock().ok().and_then(|g| g.clone()) {
            let band = s.pressure_band.as_str();
            let tip = format!(
                "Unstick [{band}] score={:.2} cpu={:.0}%",
                s.pressure_score, s.cpu_percent
            );
            let _ = tray.set_tooltip(Some(tip));
            status_item.set_text(format!(
                "[{band}] cpu={:.0}% score={:.2}",
                s.cpu_percent, s.pressure_score
            ));
        }

        if let Event::NewEvents(_) = &event {
            // poll menu/status on wake
        }
    });
}

#[cfg(windows)]
fn make_icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            // Simple teal shield glyph
            let cx = size as f32 / 2.0;
            let cy = size as f32 / 2.0;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let inside = dx.abs() < cx * 0.55 && dy.abs() < cy * 0.7;
            if inside {
                rgba[i] = 20;
                rgba[i + 1] = 160;
                rgba[i + 2] = 140;
                rgba[i + 3] = 255;
            } else {
                rgba[i + 3] = 0;
            }
        }
    }
    rgba
}
