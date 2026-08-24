//! Release identity, sidecar lookup, and fail-closed update policy.
//!
//! The packaged desktop shell does not ship `tauri-plugin-updater` (no
//! signing keys in-tree, no extra webview capability). Unsigned, wrong-target,
//! and non-newer payloads have no apply path; [`accept_update`] is the policy
//! any future updater must call.

use std::io;
use std::path::{Path, PathBuf};

use clay::ipc::{IpcEndpoint, default_endpoint};

/// Desktop crate version; must match `tauri.conf.json` and sibling packages.
pub const DESKTOP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compilation target triple (`x86_64-unknown-linux-gnu`, …).
pub fn host_triple() -> &'static str {
    option_env!("CLAY_HOST_TRIPLE").unwrap_or(std::env::consts::ARCH)
}

/// True when the argument names a network URL instead of a local IPC path.
pub fn is_networked_endpoint(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.contains("://") || trimmed.starts_with("tcp:") || trimmed.starts_with("udp:")
}

/// Endpoint used by the desktop process.
///
/// `CLAY_ENDPOINT` selects a local socket/pipe (container / already-running
/// `clay server`). Network URLs are rejected.
pub fn desktop_endpoint() -> Result<IpcEndpoint, String> {
    match std::env::var_os("CLAY_ENDPOINT") {
        None => Ok(default_endpoint()),
        Some(raw) => {
            let text = raw.to_string_lossy();
            if is_networked_endpoint(&text) {
                return Err(format!(
                    "network endpoints are not supported (got {text}); use a local socket or named pipe"
                ));
            }
            Ok(IpcEndpoint::from_argument(raw))
        }
    }
}

/// `CLAY_SERVER_BIN` → sibling `clay-server` → sibling Tauri sidecar name → `PATH`.
pub fn resolve_server_binary() -> PathBuf {
    if let Some(overridden) = std::env::var_os("CLAY_SERVER_BIN") {
        return PathBuf::from(overridden);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for candidate in sidecar_names(dir, "clay-server") {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("clay-server")
}

/// Tauri `externalBin` names the sidecar `name-<target-triple>[.exe]`.
pub fn sidecar_names(dir: &Path, stem: &str) -> [PathBuf; 2] {
    let triple = host_triple();
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    [
        dir.join(format!("{stem}{suffix}")),
        dir.join(format!("{stem}-{triple}{suffix}")),
    ]
}

/// Typed spawn failure for the status line (no path secrets beyond the
/// attempted binary name).
pub fn classify_spawn_error(binary: &Path, error: &io::Error) -> String {
    let name = binary
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("clay-server");
    if error.kind() == io::ErrorKind::NotFound {
        format!(
            "clay-server binary not found ({name}); set CLAY_SERVER_BIN or install the matching sidecar"
        )
    } else {
        format!("failed to launch clay-server: {error}")
    }
}

/// Desktop and sidecar versions must be identical (no silent skew).
pub fn versions_match(desktop: &str, other: &str) -> bool {
    desktop == other
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub version: String,
    pub target: String,
    #[serde(default)]
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateReject {
    Unsigned,
    WrongTarget,
    WrongVersion,
}

/// Fail-closed update gate. A future plugin must call this before apply.
pub fn accept_update(
    current: &str,
    host_target: &str,
    manifest: &UpdateManifest,
) -> Result<(), UpdateReject> {
    if manifest.signature.is_empty() {
        return Err(UpdateReject::Unsigned);
    }
    if manifest.target != host_target {
        return Err(UpdateReject::WrongTarget);
    }
    if !is_newer_version(&manifest.version, current) {
        return Err(UpdateReject::WrongVersion);
    }
    Ok(())
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    parse_semver(candidate) > parse_semver(current)
}

fn parse_semver(raw: &str) -> [u64; 3] {
    let mut parts = raw.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    });
    [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networked_endpoints_are_rejected() {
        for raw in [
            "tcp://127.0.0.1:7700",
            "https://example.invalid/clay",
            "ws://localhost:1",
            "tcp:1.2.3.4",
        ] {
            assert!(is_networked_endpoint(raw), "{raw}");
        }
        assert!(!is_networked_endpoint("/tmp/clay.sock"));
        assert!(!is_networked_endpoint(r"\\.\pipe\clay-user"));
    }

    #[test]
    fn missing_binary_diagnostic_is_typed() {
        let err = io::Error::new(io::ErrorKind::NotFound, "gone");
        let message = classify_spawn_error(Path::new("/opt/missing/clay-server"), &err);
        assert!(message.contains("not found"), "{message}");
        assert!(!message.contains("/opt/missing"), "{message}");
    }

    #[test]
    fn versions_must_be_identical() {
        assert!(versions_match(DESKTOP_VERSION, DESKTOP_VERSION));
        assert!(!versions_match(DESKTOP_VERSION, "0.0.0-not-this"));
    }

    #[test]
    fn update_rejects_unsigned_wrong_target_and_non_newer() {
        let host = "x86_64-unknown-linux-gnu";
        let unsigned = UpdateManifest {
            version: "0.2.0".into(),
            target: host.into(),
            signature: String::new(),
        };
        assert_eq!(
            accept_update("0.1.0", host, &unsigned),
            Err(UpdateReject::Unsigned)
        );

        let wrong_target = UpdateManifest {
            version: "0.2.0".into(),
            target: "aarch64-apple-darwin".into(),
            signature: "sig".into(),
        };
        assert_eq!(
            accept_update("0.1.0", host, &wrong_target),
            Err(UpdateReject::WrongTarget)
        );

        let same = UpdateManifest {
            version: "0.1.0".into(),
            target: host.into(),
            signature: "sig".into(),
        };
        assert_eq!(
            accept_update("0.1.0", host, &same),
            Err(UpdateReject::WrongVersion)
        );

        let older = UpdateManifest {
            version: "0.0.9".into(),
            target: host.into(),
            signature: "sig".into(),
        };
        assert_eq!(
            accept_update("0.1.0", host, &older),
            Err(UpdateReject::WrongVersion)
        );

        let ok = UpdateManifest {
            version: "0.2.0".into(),
            target: host.into(),
            signature: "sig".into(),
        };
        assert_eq!(accept_update("0.1.0", host, &ok), Ok(()));
    }

    #[test]
    fn sidecar_names_include_host_triple() {
        let names = sidecar_names(Path::new("/bundle"), "clay-server");
        assert!(names[0].ends_with("clay-server") || names[0].ends_with("clay-server.exe"));
        assert!(
            names[1]
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(host_triple()))
        );
    }
}
