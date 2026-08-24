//! Static security guards for the Tauri shell configuration.
//!
//! These fail the build if the strict CSP, minimal capability set, or
//! dependency surface regresses. They complement (never replace) runtime
//! capability enforcement by Tauri.

use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn csp_is_deny_by_default_with_no_remote_origins() {
    let conf = read("tauri.conf.json");
    let value: serde_json::Value = serde_json::from_str(&conf).expect("valid tauri.conf.json");
    let csp = value["app"]["security"]["csp"]
        .as_str()
        .expect("production CSP configured");
    for directive in ["default-src 'none'", "script-src 'self'"] {
        assert!(csp.contains(directive), "CSP missing `{directive}`: {csp}");
    }
    for forbidden in [
        "unsafe-eval",
        "unsafe-inline",
        "http://",
        "https://",
        "ws://",
    ] {
        // connect-src's http://ipc.localhost is Tauri v2's IPC origin shim on
        // Linux/Windows and is the single sanctioned exception.
        if forbidden == "http://" && csp.contains("connect-src ipc: http://ipc.localhost") {
            continue;
        }
        assert!(
            !csp.contains(forbidden),
            "CSP contains forbidden `{forbidden}`: {csp}"
        );
    }
}

#[test]
fn main_capability_grants_core_defaults_only() {
    let caps_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut seen_main = false;
    for entry in fs::read_dir(&caps_dir).expect("capabilities dir") {
        let path = entry.expect("capability file").path();
        let text = fs::read_to_string(&path).expect("capability json");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid capability json");
        if value["identifier"] == "main" {
            seen_main = true;
            assert_eq!(value["windows"], serde_json::json!(["main"]));
            for permission in value["permissions"].as_array().expect("permissions") {
                let permission = permission.as_str().expect("permission string");
                assert!(
                    permission.starts_with("core:") || permission == "core:default",
                    "non-core permission in main webview: {permission}"
                );
            }
        }
    }
    assert!(seen_main, "main capability definition went missing");
}

#[test]
fn no_privileged_tauri_plugins_are_compiled_or_configured() {
    let manifest = read("Cargo.toml");
    for plugin in [
        "tauri-plugin-fs",
        "tauri-plugin-shell",
        "tauri-plugin-process",
        "tauri-plugin-http",
        "tauri-plugin-dialog",
        "tauri-plugin-opener",
        "tauri-plugin-global-shortcut",
    ] {
        assert!(
            !manifest.contains(plugin),
            "privileged plugin {plugin} must not be compiled into the desktop shell"
        );
    }
    let conf = read("tauri.conf.json");
    assert!(
        !conf.contains("\"plugins\""),
        "tauri.conf.json must not register plugin configuration"
    );
    assert!(
        !conf.contains("createUpdaterArtifacts"),
        "updater artifacts require a signed updater; do not enable them unsigned"
    );
    assert!(
        !manifest.contains("tauri-plugin-updater"),
        "updater plugin must stay out until signing keys exist outside the repo"
    );
}

#[test]
fn release_identity_and_icon_are_present() {
    let conf: serde_json::Value =
        serde_json::from_str(&read("tauri.conf.json")).expect("valid tauri.conf.json");
    assert_eq!(
        conf["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION")),
        "tauri.conf.json version must match clay-desktop crate version"
    );
    let icons = conf["bundle"]["icon"].as_array().expect("bundle.icon list");
    assert!(!icons.is_empty(), "bundle.icon must name at least one file");
    for icon in icons {
        let rel = icon.as_str().expect("icon path string");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        assert!(path.is_file(), "bundled icon missing: {rel}");
    }
    assert_eq!(
        conf["bundle"]["targets"],
        serde_json::json!(["deb", "rpm", "appimage"]),
        "Linux bundle targets are the release surface"
    );
}
