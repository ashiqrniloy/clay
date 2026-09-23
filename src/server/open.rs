//! OS-integration helpers for the agent OAuth journey (plan 116).
//!
//! [`open_url`] hands an http(s) authorization URL to the platform default
//! browser, and [`copy_to_clipboard`] writes text to the OS clipboard. Both
//! are best-effort: the caller is a server-owned menu session that keeps the
//! OAuth stage open either way, surfacing any failure as a bounded runtime
//! diagnostic rather than aborting the flow.
//!
//! Security posture:
//! - Only `http://` / `https://` URLs are ever opened, so a host-provided
//!   value can never launch a custom scheme, local file, or shell target.
//! - The URL/text is passed to a fixed program as a single argv element; no
//!   command-line string is built from user/host-controlled input, so there
//!   is no shell parsing or quoting-injection surface.

use std::io::Write;

/// True when `url` is a plain http(s) URL that may be handed to the OS
/// browser opener. Custom schemes (`file:`, `mailto:`, `x-...:` and friends)
/// are rejected so an untrusted provider credential can never drive the
/// desktop into launching arbitrary targets.
pub(crate) fn is_safe_http_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("https://") || trimmed.starts_with("http://")
}

/// Returns the trimmed URL when it passes the http(s) guard.
fn guarded(url: &str) -> Result<&str, String> {
    let trimmed = url.trim();
    if is_safe_http_url(trimmed) {
        Ok(trimmed)
    } else {
        Err("refusing to open a non-http(s) URL from the OAuth flow".to_string())
    }
}

/// Ask the OS to open `url` in the default browser. Best-effort: returns
/// `Ok` once the opener process has been spawned; does not wait for the
/// browser to finish or to actually display the page (headless servers
/// legitimately fail later at the desktop layer).
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    let url = guarded(url)?;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("/usr/bin/open");
        command.arg(url);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    #[cfg(windows)]
    let mut command = {
        // rundll32 url.dll,FileProtocolHandler <url> is the classic
        // explorer-free default-browser dispatch; the URL travels as one
        // argv element so `&`/`?`/spaces need no shell quoting.
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler").arg(url);
        command
    };
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("could not launch the OS browser: {error}"))?;
    // Detached: the browser outlives this call. Wait briefly only to observe
    // an immediate startup failure (missing display, unknown handler...).
    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("the OS browser opener exited early: {status}")),
        Err(error) => Err(format!("the OS browser opener failed: {error}")),
    }
}

/// Write `text` to the OS clipboard so the user can paste it into any
/// browser. Tries the platform clipboard facilities in a fixed order and
/// returns the first success; every failure is folded into one message.
pub(crate) fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let candidates: &[(&str, &[&str])] = &[("/usr/bin/pbcopy", &[])];
    #[cfg(all(unix, not(target_os = "macos")))]
    let candidates: &[(&str, &[&str])] = &[
        // Wayland-first, then X11; whichever tool exists wins.
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];
    #[cfg(windows)]
    let candidates: &[(&str, &[&str])] = &[("clip", &[])];
    let mut last_error = String::new();
    for (program, args) in candidates {
        match spawn_clipboard(program, args, text) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(if last_error.is_empty() {
        "no clipboard utility is available on this system".to_string()
    } else {
        last_error
    })
}

#[cfg(any(unix, windows))]
fn spawn_clipboard(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("could not run `{program}` for the clipboard: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("could not open `{program}` stdin"))?;
    stdin
        .write_all(text.as_bytes())
        .map_err(|error| format!("could not write to `{program}`: {error}"))?;
    drop(stdin);
    let status = child
        .wait()
        .map_err(|error| format!("could not wait on `{program}`: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program}` exited with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_urls_pass_the_guard() {
        for url in [
            "https://auth.x.ai/device",
            "https://example.test/authorize?client_id=abc&scope=1",
            "http://localhost:8080/device",
        ] {
            assert!(is_safe_http_url(url), "{url}");
            assert_eq!(guarded(url).unwrap(), url);
        }
    }

    #[test]
    fn non_http_urls_are_rejected() {
        for url in [
            "file:///etc/passwd",
            "ftp://example.test/device",
            "mailto:user@example.test",
            "javascript:alert(1)",
            "x-oauth-flow:launch",
            "not a url",
            "",
        ] {
            assert!(!is_safe_http_url(url), "{url:?}");
            assert!(guarded(url).is_err(), "{url:?}");
        }
    }

    #[test]
    fn guard_trims_whitespace() {
        assert!(is_safe_http_url("  https://example.test/device  "));
        assert_eq!(
            guarded("  https://example.test/device  ").unwrap(),
            "https://example.test/device"
        );
    }
}
