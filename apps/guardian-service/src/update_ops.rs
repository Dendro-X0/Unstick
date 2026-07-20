//! GitHub Latest fetch, download, verify, and spawn `unstick-updater`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use guardian_core::{
    digest_for_file, is_newer, parse_latest_release_json, updates_dir, verify_sha256,
    zip_asset_name, ReleaseInfo, UpdateState, UPDATE_API_LATEST, VERSION,
};

#[derive(Debug, Clone)]
pub struct UpdateRuntime {
    pub state: UpdateState,
    pub available: bool,
    pub version: String,
    pub notes_url: String,
    pub error: String,
    pub unsigned_warning: bool,
    pub pending: Option<ReleaseInfo>,
    pub last_check: Option<std::time::Instant>,
}

impl UpdateRuntime {
    pub fn new() -> Self {
        Self {
            state: UpdateState::Idle,
            available: false,
            version: String::new(),
            notes_url: String::new(),
            error: String::new(),
            unsigned_warning: true,
            pending: None,
            last_check: None,
        }
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.state = UpdateState::Error;
        self.error = msg.into();
    }

    pub fn clear_error(&mut self) {
        if self.state == UpdateState::Error {
            self.state = if self.available {
                UpdateState::Available
            } else {
                UpdateState::Idle
            };
        }
        self.error.clear();
    }
}

impl Default for UpdateRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .user_agent(format!(
            "Unstick/{VERSION} (+https://github.com/Dendro-X0/Unstick)"
        ).as_str())
        .build();
    let resp = agent
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| format!("HTTP GET failed: {e}"))?;
    let mut body = Vec::new();
    resp.into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut body)
        .map_err(|e| format!("read body: {e}"))?;
    Ok(body)
}

fn http_get_text(url: &str) -> Result<String, String> {
    let bytes = http_get(url)?;
    String::from_utf8(bytes).map_err(|e| format!("response not utf8: {e}"))
}

/// Fetch Latest and update runtime fields. `force` ignores interval.
pub fn check_for_update(rt: &mut UpdateRuntime, enabled: bool) -> Result<String, String> {
    if !enabled {
        return Err("update check disabled in config".into());
    }
    rt.state = UpdateState::Checking;
    rt.clear_error();
    let body = http_get_text(UPDATE_API_LATEST)?;
    let info = parse_latest_release_json(&body)?;
    rt.last_check = Some(std::time::Instant::now());
    rt.notes_url = info.notes_url.clone();
    rt.version = info.version.clone();
    if is_newer(&info.version, VERSION) {
        rt.available = true;
        rt.state = UpdateState::Available;
        rt.pending = Some(info);
        Ok(format!("update available: {}", rt.version))
    } else {
        rt.available = false;
        rt.state = UpdateState::Idle;
        rt.pending = None;
        Ok(format!("up to date (running {VERSION}, latest {})", rt.version))
    }
}

pub fn download_and_verify(rt: &mut UpdateRuntime) -> Result<PathBuf, String> {
    let info = rt
        .pending
        .clone()
        .ok_or_else(|| "no pending update — Check for updates first".to_string())?;
    let sums_url = info
        .sha256sums_url
        .clone()
        .ok_or_else(|| "release has no SHA256SUMS asset — refuse to install".to_string())?;

    rt.state = UpdateState::Downloading;
    rt.error.clear();
    let dir = updates_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("updates dir: {e}"))?;

    let sums_text = http_get_text(&sums_url)?;
    let expected = digest_for_file(&sums_text, &info.zip_name)?;

    let zip_bytes = http_get(&info.zip_url)?;
    verify_sha256(&zip_bytes, &expected)?;

    let zip_path = dir.join(&info.zip_name);
    fs::write(&zip_path, &zip_bytes).map_err(|e| format!("write zip: {e}"))?;
    let _ = fs::write(dir.join("SHA256SUMS"), sums_text);

    rt.state = UpdateState::Verified;
    Ok(zip_path)
}

pub fn install_dir_from_service_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "service exe has no parent dir".into())
}

pub fn updater_path(install_dir: &Path) -> PathBuf {
    install_dir.join("unstick-updater.exe")
}

/// Spawn updater then caller should exit the service process.
pub fn spawn_updater(
    install_dir: &Path,
    zip_path: &Path,
    restart_ui: bool,
    restart_tray: bool,
) -> Result<(), String> {
    let updater = updater_path(install_dir);
    if !updater.exists() {
        return Err(format!(
            "missing {} — reinstall from zip or use manual update",
            updater.display()
        ));
    }
    let mut cmd = Command::new(&updater);
    cmd.arg("--install-dir")
        .arg(install_dir)
        .arg("--zip")
        .arg(zip_path)
        .arg("--restart-service");
    if restart_ui {
        cmd.arg("--restart-ui");
    }
    if restart_tray {
        cmd.arg("--restart-tray");
    }
    // Detach: updater waits for us to exit.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
    cmd.spawn()
        .map_err(|e| format!("spawn updater: {e}"))?;
    Ok(())
}

#[allow(dead_code)]
pub fn expected_zip_name_for_version(v: &str) -> String {
    zip_asset_name(v)
}
