//! Portable apply helper: stop Unstick processes, extract allowlisted zip members, restart.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use guardian_core::is_allowed_update_member;
use zip::ZipArchive;

fn main() {
    if let Err(e) = run() {
        let _ = write_log(&format!("FATAL: {e}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let install_dir = arg_value(&args, "--install-dir")
        .ok_or_else(|| "missing --install-dir".to_string())?;
    let zip_path =
        arg_value(&args, "--zip").ok_or_else(|| "missing --zip".to_string())?;
    let restart_service = args.iter().any(|a| a == "--restart-service");
    let restart_ui = args.iter().any(|a| a == "--restart-ui");
    let restart_tray = args.iter().any(|a| a == "--restart-tray");

    let install_dir = PathBuf::from(install_dir);
    let zip_path = PathBuf::from(zip_path);
    if !zip_path.is_file() {
        return Err(format!("zip not found: {}", zip_path.display()));
    }
    if !install_dir.is_dir() {
        return Err(format!("install dir missing: {}", install_dir.display()));
    }

    write_log(&format!(
        "waiting for processes to exit; install={} zip={}",
        install_dir.display(),
        zip_path.display()
    ))?;

    // Give service/UI time to exit after spawning us.
    thread::sleep(Duration::from_secs(2));
    stop_unstick_processes();
    thread::sleep(Duration::from_millis(800));

    extract_allowlisted(&zip_path, &install_dir)?;
    write_log("extract ok")?;

    if restart_service {
        start_exe(&install_dir.join("guardian-service.exe"))?;
        thread::sleep(Duration::from_millis(600));
    }
    if restart_ui {
        start_exe(&install_dir.join("guardian-ui.exe"))?;
    }
    if restart_tray {
        start_exe(&install_dir.join("guardian-tray.exe"))?;
    }
    write_log("restart done")?;
    Ok(())
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn updates_log_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base)
        .join("Unstick")
        .join("updates")
        .join("last-apply.log")
}

fn write_log(msg: &str) -> Result<(), String> {
    let path = updates_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut f = OpenOptionsAppend::open(&path)?;
    writeln!(f, "{}", msg).map_err(|e| e.to_string())?;
    Ok(())
}

struct OpenOptionsAppend;
impl OpenOptionsAppend {
    fn open(path: &Path) -> Result<File, String> {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())
    }
}

fn stop_unstick_processes() {
    for name in ["guardian-ui", "guardian-tray", "guardian-service"] {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &format!("{name}.exe")])
            .output();
    }
}

fn extract_allowlisted(zip_path: &Path, install_dir: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        let name = entry.name().to_string();
        let base = name.rsplit('/').next().unwrap_or(&name).to_string();
        if entry.is_dir() {
            continue;
        }
        if !is_allowed_update_member(&base) {
            write_log(&format!("skip non-allowlisted: {name}"))?;
            continue;
        }
        let dest = install_dir.join(&base);
        let tmp = install_dir.join(format!("{base}.unstick-new"));
        {
            let mut out = File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
            io::copy(&mut entry, &mut out).map_err(|e| format!("copy {base}: {e}"))?;
        }
        fs::rename(&tmp, &dest).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("replace {base}: {e}")
        })?;
        write_log(&format!("replaced {base}"))?;
    }
    Ok(())
}

fn start_exe(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("missing {}", path.display()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        Command::new(path)
            .current_dir(path.parent().unwrap_or(Path::new(".")))
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|e| format!("start {}: {e}", path.display()))?;
    }
    #[cfg(not(windows))]
    {
        Command::new(path)
            .spawn()
            .map_err(|e| format!("start {}: {e}", path.display()))?;
    }
    Ok(())
}
