//! GitHub Latest check + portable update helpers (parse / semver / hash / allowlist).

use serde::{Deserialize, Serialize};

/// Official release channel (pinned).
pub const UPDATE_OWNER: &str = "Dendro-X0";
pub const UPDATE_REPO: &str = "Unstick";
pub const UPDATE_API_LATEST: &str =
    "https://api.github.com/repos/Dendro-X0/Unstick/releases/latest";

/// Operator-visible update machine state on status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    Available,
    Downloading,
    Verified,
    Applying,
    Error,
}

impl UpdateState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::Available => "available",
            Self::Downloading => "downloading",
            Self::Verified => "verified",
            Self::Applying => "applying",
            Self::Error => "error",
        }
    }
}

/// Parsed Latest release suitable for apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub notes_url: String,
    pub zip_name: String,
    pub zip_url: String,
    pub sha256sums_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Strip optional leading `v` / `V`.
pub fn normalize_version(raw: &str) -> String {
    let t = raw.trim();
    t.strip_prefix('v')
        .or_else(|| t.strip_prefix('V'))
        .unwrap_or(t)
        .to_string()
}

/// Expected portable zip name for a version.
pub fn zip_asset_name(version: &str) -> String {
    format!("Unstick-{}-windows-x64.zip", normalize_version(version))
}

/// Compare dotted semver-ish versions (major.minor.patch[+extra ignored for numeric tuple]).
/// Returns `Some(Ordering)` when both parse; `None` if either is unparsable.
pub fn cmp_semver(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let pa = parse_semver_tuple(a)?;
    let pb = parse_semver_tuple(b)?;
    Some(pa.cmp(&pb))
}

fn parse_semver_tuple(raw: &str) -> Option<(u64, u64, u64)> {
    let v = normalize_version(raw);
    let core = v.split(['-', '+']).next().unwrap_or(&v);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    matches!(cmp_semver(remote, local), Some(std::cmp::Ordering::Greater))
}

/// Parse GitHub Releases API JSON for Latest into apply metadata.
pub fn parse_latest_release_json(body: &str) -> Result<ReleaseInfo, String> {
    let rel: GhRelease =
        serde_json::from_str(body).map_err(|e| format!("invalid GitHub release JSON: {e}"))?;
    let version = normalize_version(&rel.tag_name);
    if version.is_empty() {
        return Err("empty tag_name".into());
    }
    let want_zip = zip_asset_name(&version);
    let zip = rel
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&want_zip))
        .ok_or_else(|| format!("release missing asset {want_zip}"))?;
    let sha = rel.assets.iter().find(|a| {
        let n = a.name.to_ascii_lowercase();
        n == "sha256sums"
            || n == format!("{want_zip}.sha256").to_ascii_lowercase()
            || n.ends_with(".sha256sums")
    });
    Ok(ReleaseInfo {
        version,
        notes_url: rel.html_url,
        zip_name: zip.name.clone(),
        zip_url: zip.browser_download_url.clone(),
        sha256sums_url: sha.map(|a| a.browser_download_url.clone()),
    })
}

/// Look up hex digest for `file_name` in GNU `SHA256SUMS` text (or single-line `hash  name`).
pub fn digest_for_file(sums_text: &str, file_name: &str) -> Result<String, String> {
    let want = file_name.to_ascii_lowercase();
    for line in sums_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next().ok_or_else(|| "empty SHA256 line".to_string())?;
        let name = parts
            .next()
            .map(|n| n.trim_start_matches('*'))
            .unwrap_or("");
        if name.is_empty() && hash.len() == 64 && parts.next().is_none() {
            // bare hash for a known single file
            return Ok(hash.to_ascii_lowercase());
        }
        if name.to_ascii_lowercase() == want
            || std::path::Path::new(name)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(file_name))
                .unwrap_or(false)
        {
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("invalid sha256 for {file_name}"));
            }
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(format!("SHA256SUMS has no entry for {file_name}"))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), String> {
    let got = sha256_hex(bytes);
    let exp = expected_hex.trim().to_ascii_lowercase();
    if got != exp {
        return Err(format!("sha256 mismatch (got {got}, expected {exp})"));
    }
    Ok(())
}

/// Safe extract member names for portable zip apply.
pub fn is_allowed_update_member(name: &str) -> bool {
    if name.contains("..") || name.contains('\\') || name.starts_with('/') {
        return false;
    }
    let base = name.rsplit('/').next().unwrap_or(name);
    if base != name && name.contains('/') {
        // only flat zip members (no nested dirs)
        return false;
    }
    matches!(
        base,
        "guardian-service.exe"
            | "guardian-ui.exe"
            | "guardian-tray.exe"
            | "unstick-updater.exe"
            | "Install-Autostart.ps1"
            | "Uninstall-Autostart.ps1"
            | "USER-GUIDE.md"
            | "RELEASE-NOTES.md"
            | "SIGNING.txt"
            | "README.txt"
            | "packaging-and-soak.md"
            | "frontend-spec.md"
            | "SHA256SUMS"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_newer() {
        assert!(is_newer("0.8.0", "0.7.0"));
        assert!(is_newer("v0.8.1", "0.8.0"));
        assert!(!is_newer("0.7.0", "0.8.0"));
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn parse_release_finds_zip_and_sums() {
        let body = r#"{
          "tag_name": "v0.8.1",
          "html_url": "https://github.com/Dendro-X0/Unstick/releases/tag/v0.8.1",
          "assets": [
            {
              "name": "Unstick-0.8.1-windows-x64.zip",
              "browser_download_url": "https://example.com/z.zip"
            },
            {
              "name": "SHA256SUMS",
              "browser_download_url": "https://example.com/SHA256SUMS"
            }
          ]
        }"#;
        let info = parse_latest_release_json(body).unwrap();
        assert_eq!(info.version, "0.8.1");
        assert!(info.zip_url.contains("z.zip"));
        assert!(info.sha256sums_url.unwrap().contains("SHA256SUMS"));
    }

    #[test]
    fn parse_release_missing_zip_errors() {
        let body = r#"{"tag_name":"v0.8.0","html_url":"https://x","assets":[]}"#;
        assert!(parse_latest_release_json(body).is_err());
    }

    #[test]
    fn digest_lookup() {
        let sums = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899  Unstick-0.8.0-windows-x64.zip\n";
        let d = digest_for_file(sums, "Unstick-0.8.0-windows-x64.zip").unwrap();
        assert_eq!(d.len(), 64);
    }

    #[test]
    fn verify_hash_roundtrip() {
        let bytes = b"hello-unstick";
        let h = sha256_hex(bytes);
        verify_sha256(bytes, &h).unwrap();
        assert!(verify_sha256(bytes, "00").is_err());
    }

    #[test]
    fn allowlist_rejects_traversal() {
        assert!(is_allowed_update_member("guardian-service.exe"));
        assert!(!is_allowed_update_member("../evil.exe"));
        assert!(!is_allowed_update_member("nested/guardian-service.exe"));
        assert!(!is_allowed_update_member("malware.exe"));
    }
}
