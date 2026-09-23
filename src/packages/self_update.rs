//! Install-channel self-update (plan 115 task 6).
//!
//! Channel identity is a marker file next to the running binary, written by
//! the npm package or curl installer (task 8). Missing, corrupt, unknown, or
//! non-owner-only markers are unmanaged: skip, exit success.
//!
//! This module never downloads URLs and never applies a release payload.
//! `accept_update` in `src-tauri/src/release.rs` remains the only payload-apply
//! gate; v1 self-update only execs the recorded argv.

use crate::str_enum::string_enum_impl;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::packages::approvals::atomic_write_owner_only;

pub const CHANNEL_MARKER_FILE_NAME: &str = "channel.json";
const CHANNEL_VERSION: u64 = 1;
const MAX_MARKER_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    Npm,
    Curl,
}

string_enum_impl! {
    pub ChannelKind, parse_private {
        Npm => "npm",
        Curl => "curl",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    Unmanaged {
        reason: String,
    },
    Command {
        kind: ChannelKind,
        argv: Vec<String>,
    },
}

pub fn default_marker_path() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => {
            let resolved = fs::canonicalize(&exe).unwrap_or(exe);
            match resolved.parent() {
                Some(dir) => dir.join(CHANNEL_MARKER_FILE_NAME),
                None => PathBuf::from(CHANNEL_MARKER_FILE_NAME),
            }
        }
        Err(_) => PathBuf::from(CHANNEL_MARKER_FILE_NAME),
    }
}

pub fn resolve_channel(path: &Path) -> Channel {
    match read_channel(path) {
        Ok(channel) => channel,
        Err(reason) => Channel::Unmanaged { reason },
    }
}

pub fn write_channel_marker(path: &Path, kind: ChannelKind, argv: &[String]) -> Result<(), String> {
    if !valid_argv(argv) {
        return Err("channel argv must be a non-empty array of non-empty strings".into());
    }
    let document = serde_json::json!({
        "version": CHANNEL_VERSION,
        "channel": kind.as_str(),
        "argv": argv,
    });
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    atomic_write_owner_only(path, &bytes).map_err(|error| error.to_string())
}

pub fn run_self_update(
    channel: &Channel,
    mut spawn: impl FnMut(&[String]) -> Result<i32, String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    match channel {
        Channel::Unmanaged { reason } => {
            writeln!(
                out,
                "clay: this checkout is not managed by an install channel ({reason}); nothing to update."
            )
            .map_err(|error| error.to_string())?;
            Ok(())
        }
        Channel::Command { kind, argv } => {
            writeln!(
                out,
                "Updating Clay via {} channel: {}",
                kind.as_str(),
                argv.join(" ")
            )
            .map_err(|error| error.to_string())?;
            let code = spawn(argv)?;
            if code == 0 {
                writeln!(out, "Clay self-update finished.").map_err(|error| error.to_string())?;
                Ok(())
            } else {
                Err(format!("Clay self-update command exited {code}"))
            }
        }
    }
}

fn read_channel(path: &Path) -> Result<Channel, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("no channel marker".into());
        }
        Err(error) => return Err(format!("channel marker unreadable: {error}")),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("channel marker is not owner-only".into());
        }
    }
    if metadata.len() as usize > MAX_MARKER_BYTES {
        return Err("channel marker exceeds size limit".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("channel marker unreadable: {error}"))?;
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "channel marker is not valid JSON".to_string())?;
    let version = document.get("version").and_then(Value::as_u64);
    if version != Some(CHANNEL_VERSION) {
        return Err(format!(
            "unknown channel marker version {version:?} (expected {CHANNEL_VERSION})"
        ));
    }
    let kind = document
        .get("channel")
        .and_then(Value::as_str)
        .and_then(ChannelKind::parse)
        .ok_or_else(|| "channel marker missing a known channel (npm|curl)".to_string())?;
    let argv_value = document
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(|| "channel marker `argv` must be an array of strings".to_string())?;
    let mut argv = Vec::with_capacity(argv_value.len());
    for value in argv_value {
        let Some(part) = value.as_str() else {
            return Err("channel marker `argv` must be an array of strings".into());
        };
        argv.push(part.to_string());
    }
    if !valid_argv(&argv) {
        return Err("channel argv must be a non-empty array of non-empty strings".into());
    }
    Ok(Channel::Command { kind, argv })
}

fn valid_argv(argv: &[String]) -> bool {
    !argv.is_empty() && argv.iter().all(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_marker(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-channel-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root.join(CHANNEL_MARKER_FILE_NAME)
    }

    #[test]
    fn missing_marker_is_unmanaged_skip() {
        let path = temp_marker("missing");
        let channel = resolve_channel(&path);
        assert!(matches!(channel, Channel::Unmanaged { .. }));
        let mut spawned = false;
        let mut out = Vec::new();
        run_self_update(
            &channel,
            |_| {
                spawned = true;
                Ok(0)
            },
            &mut out,
        )
        .unwrap();
        assert!(!spawned);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("not managed by an install channel"));
        assert!(text.contains("nothing to update"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn npm_and_curl_markers_spawn_recorded_argv() {
        for (kind, argv) in [
            (
                ChannelKind::Npm,
                vec![
                    "npm".into(),
                    "update".into(),
                    "-g".into(),
                    "@arnilo/clay".into(),
                ],
            ),
            (
                ChannelKind::Curl,
                vec!["curl-clay-update".into(), "--recorded".into()],
            ),
        ] {
            let path = temp_marker(kind.as_str());
            write_channel_marker(&path, kind, &argv).unwrap();
            let channel = resolve_channel(&path);
            let mut seen = None;
            let mut out = Vec::new();
            run_self_update(
                &channel,
                |got| {
                    seen = Some(got.to_vec());
                    Ok(0)
                },
                &mut out,
            )
            .unwrap();
            assert_eq!(seen.as_deref(), Some(argv.as_slice()));
            let text = String::from_utf8(out).unwrap();
            assert!(text.contains(kind.as_str()));
            assert!(text.contains("Clay self-update finished."));
            let _ = fs::remove_dir_all(path.parent().unwrap());
        }
    }

    #[test]
    fn corrupt_unknown_empty_and_unsafe_markers_skip() {
        let path = temp_marker("corrupt");
        fs::write(&path, b"{not json").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(matches!(
            resolve_channel(&path),
            Channel::Unmanaged { reason } if reason.contains("JSON")
        ));

        fs::write(
            &path,
            b"{\"version\":99,\"channel\":\"npm\",\"argv\":[\"npm\"]}",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(matches!(
            resolve_channel(&path),
            Channel::Unmanaged { reason } if reason.contains("version")
        ));

        write_channel_marker(&path, ChannelKind::Npm, &["npm".into()]).unwrap();
        fs::write(&path, b"{\"version\":1,\"channel\":\"npm\",\"argv\":[]}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(matches!(
            resolve_channel(&path),
            Channel::Unmanaged { reason } if reason.contains("argv")
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            write_channel_marker(
                &path,
                ChannelKind::Npm,
                &[
                    "npm".into(),
                    "update".into(),
                    "-g".into(),
                    "@arnilo/clay".into(),
                ],
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            let mut spawned = false;
            let channel = resolve_channel(&path);
            assert!(matches!(
                &channel,
                Channel::Unmanaged { reason } if reason.contains("owner-only")
            ));
            run_self_update(
                &channel,
                |_| {
                    spawned = true;
                    Ok(0)
                },
                &mut Vec::new(),
            )
            .unwrap();
            assert!(!spawned);
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn failed_channel_command_is_an_error() {
        let path = temp_marker("fail");
        write_channel_marker(&path, ChannelKind::Npm, &["npm".into()]).unwrap();
        let channel = resolve_channel(&path);
        let error = run_self_update(&channel, |_| Ok(2), &mut Vec::new()).unwrap_err();
        assert!(error.contains("exited 2"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn default_marker_is_channel_json_beside_the_binary() {
        assert_eq!(
            default_marker_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(CHANNEL_MARKER_FILE_NAME)
        );
    }
}
