//! Plan 115 task 14: roadmap Phase 3 exit-gate drills (Linux).
//!
//! Each drill drives the REAL `clay` binary (`CARGO_BIN_EXE_clay`) through
//! the REAL npm backend against a LOCAL static registry fixture (in-test
//! HTTP server + tarballs built with `tar` + digests via `python3`). No
//! network dependency: everything binds to 127.0.0.1 and lives in a scratch
//! `HOME`. Fails with a clear message when `npm`/`tar`/`python3` are
//! unavailable (Linux CI provides all three).

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_NAME: &str = "clay-fixture-pkg";
const FIXTURE_VERSIONS: [&str; 2] = ["0.1.0", "0.2.0"];

struct Registry {
    base_url: String,
}

/// Minimal static npm registry: GET /<name> and /<name>/<version> serve the
/// packument (from `_meta/`); GET /<name>/-/<name>-<version>.tgz streams the
/// tarball.
fn spawn_registry(dir: &Path) -> Registry {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind localhost");
    let port = listener.local_addr().expect("addr").port();
    let root = dir.to_path_buf();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let root = root.clone();
            std::thread::spawn(move || handle_connection(stream, &root));
        }
    });
    Registry {
        base_url: format!("http://127.0.0.1:{port}"),
    }
}

fn handle_connection(mut stream: TcpStream, root: &Path) {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                buf.push(byte[0]);
                if buf.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    let request = String::from_utf8_lossy(&buf);
    let path = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    let path = path.trim_start_matches('/');
    let body = if path.is_empty() {
        None
    } else {
        std::fs::read(root.join(path))
            .or_else(|_| std::fs::read(root.join("_meta").join(path)))
            .ok()
    };
    let Some(body) = body else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    };
    let content_type = if path.ends_with(".tgz") {
        "application/octet-stream"
    } else {
        "application/json"
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

fn tools_available() -> bool {
    ["npm", "tar", "python3"].iter().all(|tool| {
        Command::new(tool)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    })
}

fn scratch_home(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "clay-exit-gate-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join(".clay")).expect("scratch config dir");
    std::fs::write(
        root.join(".clay/init.js"),
        "// user line one\nimport { loadPackage } from \"clay:packages\";\n",
    )
    .expect("seed init.js");
    root
}

fn run_cli(home: &Path, registry: &Registry, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_clay"))
        .args(args)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("CLAY_PACKAGE_MANAGER", "npm")
        .env("npm_config_registry", format!("{}/", registry.base_url))
        .env("npm_config_cache", home.join("npm-cache"))
        .env("npm_config_update_notifier", "false")
        .env("npm_config_fund", "false")
        .env("npm_config_audit", "false")
        .env("npm_config_loglevel", "error")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("spawn clay CLI")
}

fn cli_text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed ({}):\n{}",
        output.status,
        cli_text(output)
    );
}

/// Build and "publish" one fixture version into the registry dir: staged
/// `package/` tarball (gzip via `tar czf`) plus an npm packument at
/// `_meta/<name>` and per-version docs, with sha1 shasum + sha512 SRI
/// integrity computed via `python3`.
fn publish_version(registry_dir: &Path, base_url: &str, version: &str, with_script: bool) {
    let staging = registry_dir.join(format!("staging-{version}"));
    std::fs::create_dir_all(staging.join("package")).expect("staging package dir");
    let mut manifest = serde_json::json!({
        "name": FIXTURE_NAME,
        "version": version,
        "type": "module",
        "exports": { ".": "./index.js" },
        "clay": {
            "apiPrefix": "gate",
            "entry": "./index.js",
            "loadEntry": "./index.js",
            "permissions": [],
            "modes": ["cli"],
            "docs": "./docs.md"
        }
    });
    if with_script {
        manifest["scripts"] = serde_json::json!({
            // Sentinel file lands inside the installed package dir; presence
            // proves lifecycle scripts ran.
            "postinstall": "node -e \"require('fs').writeFileSync('allow-scripts-ran','1')\""
        });
    }
    std::fs::write(
        staging.join("package/package.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest"),
    )
    .expect("write package.json");
    std::fs::write(
        staging.join("package/index.js"),
        "export default function load() {}\n",
    )
    .expect("write index.js");
    std::fs::write(staging.join("package/docs.md"), "# fixture\n").expect("write docs.md");

    let tarball_rel = format!("{FIXTURE_NAME}/-/{FIXTURE_NAME}-{version}.tgz");
    let tarball_path = registry_dir.join(&tarball_rel);
    std::fs::create_dir_all(tarball_path.parent().expect("tarball dir")).expect("tarball dir");
    let status = Command::new("tar")
        .args([
            "czf",
            tarball_path.to_str().expect("tarball path"),
            "package",
        ])
        .current_dir(&staging)
        .status()
        .expect("spawn tar");
    assert!(status.success(), "tar czf failed");

    let digest = Command::new("python3")
        .args([
            "-c",
            "import hashlib,base64,sys; d=open(sys.argv[1],'rb').read(); print(hashlib.sha1(d).hexdigest()); print(base64.b64encode(hashlib.sha512(d).digest()).decode())",
            tarball_path.to_str().expect("digest path"),
        ])
        .output()
        .expect("spawn python3");
    assert!(digest.status.success(), "python3 digests failed");
    let digest_text = String::from_utf8_lossy(&digest.stdout).to_string();
    let mut parts = digest_text.split_whitespace();
    let shasum = parts.next().expect("sha1").to_string();
    let integrity = parts.next().expect("sha512").to_string();

    let packument_path = registry_dir.join("_meta").join(FIXTURE_NAME);
    std::fs::create_dir_all(packument_path.parent().expect("meta dir")).expect("meta dir");
    let mut doc: Value = match std::fs::read(&packument_path) {
        Ok(existing) => serde_json::from_slice(&existing).expect("existing packument"),
        Err(_) => serde_json::json!({
            "_id": FIXTURE_NAME,
            "name": FIXTURE_NAME,
            "dist-tags": {},
            "versions": {}
        }),
    };
    doc["versions"][version] = serde_json::json!({
        "_id": format!("{FIXTURE_NAME}@{version}"),
        "name": FIXTURE_NAME,
        "version": version,
        "dist": {
            "tarball": format!("{base_url}/{tarball_rel}"),
            "shasum": shasum,
            "integrity": format!("sha512-{integrity}")
        }
    });
    let mut versions: Vec<String> = doc["versions"]
        .as_object()
        .expect("versions object")
        .keys()
        .cloned()
        .collect();
    versions.sort();
    doc["dist-tags"]["latest"] =
        serde_json::json!(versions.last().expect("at least one version").clone());
    std::fs::write(
        &packument_path,
        serde_json::to_vec(&doc).expect("packument"),
    )
    .expect("write packument");
}

/// Full fixture context: registry + both fixture versions published.
struct Fixture {
    home: PathBuf,
    registry: Registry,
}

fn fixture(label: &str, with_lifecycle_script: bool) -> Fixture {
    assert!(
        tools_available(),
        "exit-gate drills need npm, tar, and python3 on PATH (Linux CI provides them)"
    );
    let home = scratch_home(label);
    let registry_dir = home.join("registry");
    std::fs::create_dir_all(&registry_dir).expect("registry dir");
    let registry = spawn_registry(&registry_dir);
    for version in FIXTURE_VERSIONS {
        publish_version(
            &registry_dir,
            &registry.base_url,
            version,
            with_lifecycle_script,
        );
    }
    Fixture { home, registry }
}

/// Resolve an executable's absolute path via `which`.
fn which(program: &str) -> PathBuf {
    let out = Command::new("which")
        .arg(program)
        .output()
        .expect("spawn which");
    assert!(out.status.success(), "{program} not found on PATH");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Put a recording `npm` shim first on PATH: it appends its argv to
/// `$NPM_ARG_LOG`, then execs the real npm.
fn npm_shim_dir(home: &Path) -> PathBuf {
    let shim_dir = home.join("npm-shim");
    std::fs::create_dir_all(&shim_dir).expect("shim dir");
    let real_npm = which("npm");
    let shim = shim_dir.join("npm");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$NPM_ARG_LOG\"\nexec {} \"$@\"\n",
            real_npm.display()
        ),
    )
    .expect("write npm shim");
    let status = Command::new("chmod")
        .args(["+x", shim.to_str().expect("shim path")])
        .status()
        .expect("chmod shim");
    assert!(status.success(), "chmod +x shim failed");
    shim_dir
}

fn config_root(home: &Path) -> PathBuf {
    home.join(".clay")
}

fn init_js(home: &Path) -> PathBuf {
    config_root(home).join("init.js")
}

fn store_root(home: &Path) -> PathBuf {
    config_root(home).join("packages")
}

fn init_block_count(home: &Path) -> usize {
    String::from_utf8_lossy(&std::fs::read(init_js(home)).expect("init.js"))
        .lines()
        .filter(|line| line.contains(&format!("// clay install npm:{FIXTURE_NAME}")))
        .count()
}

/// The single ledger record for the fixture package.
fn ledger_record(home: &Path) -> Value {
    let path = store_root(home).join("installs.json");
    let doc: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("installs.json")).expect("ledger json");
    doc["packages"]
        .as_array()
        .expect("packages array")
        .iter()
        .find(|record| record["name"] == FIXTURE_NAME)
        .expect("fixture ledger record")
        .clone()
}

fn marker_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_clay"))
        .parent()
        .expect("binary dir")
        .join("channel.json")
}

/// Drop guard: removes the channel marker written beside the test binary so
/// subsequent drills/dev runs see the dev-checkout no-op again.
struct MarkerGuard;
impl Drop for MarkerGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(marker_path());
    }
}

// ─── Drill 1: install → store round-trip → remove ───────────────────────────

#[test]
fn exit_gate_install_round_trip_and_remove() {
    let fx = fixture("roundtrip", false);
    let out = run_cli(
        &fx.home,
        &fx.registry,
        &["install", &format!("npm:{FIXTURE_NAME}")],
    );
    assert_success(&out, "clay install");
    let text = cli_text(&out);
    assert!(
        text.contains(&format!(
            "Installed {FIXTURE_NAME}@0.2.0 from npm:{FIXTURE_NAME}"
        )),
        "{text}"
    );
    assert!(text.contains("Not enabled, not adopted"), "{text}");
    assert!(
        text.contains(&format!("Appended loadPackage(\"{FIXTURE_NAME}\")")),
        "{text}"
    );

    // Store round-trip: the real npm backend materialized the package.
    let installed_manifest = store_root(&fx.home)
        .join("node_modules")
        .join(FIXTURE_NAME)
        .join("package.json");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&installed_manifest).expect("installed manifest"))
            .expect("manifest json");
    assert_eq!(manifest["name"], FIXTURE_NAME);
    assert_eq!(manifest["version"], "0.2.0");

    // Ledger recorded the floating spec.
    let record = ledger_record(&fx.home);
    assert_eq!(record["version"], "0.2.0");
    assert_eq!(record["pinned"], serde_json::json!(false));

    // Exactly one appended load line.
    assert_eq!(init_block_count(&fx.home), 1);

    // Remove: store + ledger + appended block all cleaned, user lines kept.
    let out = run_cli(
        &fx.home,
        &fx.registry,
        &["remove", &format!("npm:{FIXTURE_NAME}")],
    );
    assert_success(&out, "clay remove");
    assert!(cli_text(&out).contains(&format!("Removed {FIXTURE_NAME}")));
    assert!(
        !store_root(&fx.home)
            .join("node_modules")
            .join(FIXTURE_NAME)
            .exists(),
        "package must be gone from the store"
    );
    let doc: Value = serde_json::from_slice(
        &std::fs::read(store_root(&fx.home).join("installs.json")).expect("ledger"),
    )
    .expect("ledger json");
    assert!(doc["packages"].as_array().expect("packages").is_empty());
    assert_eq!(init_block_count(&fx.home), 0);
    let init_bytes = std::fs::read(init_js(&fx.home)).expect("init.js");
    let init = String::from_utf8_lossy(&init_bytes);
    assert!(
        init.contains("// user line one"),
        "user lines must survive: {init}"
    );
}

// ─── Drill 2: update --extensions floating vs pinned ────────────────────────

#[test]
fn exit_gate_update_extensions_updates_floating() {
    let fx = fixture("update-extensions", false);
    let out = run_cli(
        &fx.home,
        &fx.registry,
        &["install", &format!("npm:{FIXTURE_NAME}@0.1.0")],
    );
    assert_success(&out, "pinned install");
    assert_eq!(
        ledger_record(&fx.home)["version"],
        serde_json::json!("0.1.0")
    );

    // Replace with a floating install so --extensions has a floating record
    // to act on (idempotent append keeps the line count at one).
    let out = run_cli(
        &fx.home,
        &fx.registry,
        &["install", &format!("npm:{FIXTURE_NAME}")],
    );
    assert_success(&out, "floating install");
    let record = ledger_record(&fx.home);
    assert_eq!(record["pinned"], serde_json::json!(false));
    assert_eq!(record["version"], "0.2.0");

    let out = run_cli(&fx.home, &fx.registry, &["update", "--extensions"]);
    assert_success(&out, "clay update --extensions");
    let text = cli_text(&out);
    assert!(
        text.contains(&format!("Updated {FIXTURE_NAME}@0.2.0")),
        "floating package must be updated: {text}"
    );
    assert_eq!(ledger_record(&fx.home)["version"], "0.2.0");
    assert_eq!(
        init_block_count(&fx.home),
        1,
        "update must not duplicate lines"
    );
}

#[test]
fn exit_gate_update_single_spec_skips_pinned_and_unmanaged() {
    let fx = fixture("update-single", false);
    let pinned = format!("npm:{FIXTURE_NAME}@0.1.0");
    let out = run_cli(&fx.home, &fx.registry, &["install", &pinned]);
    assert_success(&out, "pinned install");

    let out = run_cli(&fx.home, &fx.registry, &["update", &pinned]);
    assert_success(&out, "pinned single update");
    let text = cli_text(&out);
    assert!(
        text.contains("pinned; reinstall with a new version to move it"),
        "{text}"
    );

    let out = run_cli(&fx.home, &fx.registry, &["update", "npm:@vendor/nope"]);
    assert_success(&out, "unmanaged single update");
    assert!(cli_text(&out).contains("not a Clay-managed install"));
}

// ─── Drill 3: lifecycle scripts off unless --allow-scripts ──────────────────

#[test]
fn exit_gate_lifecycle_scripts_off_by_default_on_with_flag() {
    let fx = fixture("scripts", true);
    let spec = format!("npm:{FIXTURE_NAME}");
    let sentinel = store_root(&fx.home)
        .join("node_modules")
        .join(FIXTURE_NAME)
        .join("allow-scripts-ran");

    let out = run_cli(&fx.home, &fx.registry, &["install", &spec]);
    assert_success(&out, "default install");
    assert!(
        !sentinel.exists(),
        "lifecycle scripts must NOT run without --allow-scripts"
    );
    assert!(!cli_text(&out).contains("lifecycle scripts are ENABLED"));

    // Arg plumbing through the real CLI → service → backend → manager
    // process: an npm shim on PATH records every invocation, so we can prove
    // Clay suppresses (default) or passes through (--allow-scripts) the
    // script policy regardless of npm's own environment-dependent gating.
    let shim_dir = npm_shim_dir(&fx.home);
    let arg_log = fx.home.join("npm-args.log");
    let run_with_shim = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_clay"))
            .args(args)
            .env("HOME", &fx.home)
            .env("XDG_DATA_HOME", fx.home.join(".local/share"))
            .env("XDG_CACHE_HOME", fx.home.join(".cache"))
            .env("XDG_STATE_HOME", fx.home.join(".local/state"))
            .env("CLAY_PACKAGE_MANAGER", "npm")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    shim_dir.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("NPM_ARG_LOG", &arg_log)
            .env("npm_config_registry", format!("{}/", fx.registry.base_url))
            .env("npm_config_cache", fx.home.join("npm-cache"))
            .env("npm_config_update_notifier", "false")
            .env("npm_config_fund", "false")
            .env("npm_config_audit", "false")
            .env("npm_config_loglevel", "error")
            .output()
            .expect("spawn clay CLI with shim")
    };
    let out = run_with_shim(&["install", &spec]);
    assert_success(&out, "shim default install");
    let logged = std::fs::read_to_string(&arg_log).expect("npm arg log");
    assert!(
        logged.contains("--ignore-scripts"),
        "default install must pass --ignore-scripts to the manager: {logged}"
    );

    // --allow-scripts leg: clean store, re-install, flag must be absent.
    let out = run_with_shim(&["remove", &spec]);
    assert_success(&out, "shim remove");
    let out = run_with_shim(&["install", &spec, "--allow-scripts"]);
    assert_success(&out, "shim allow-scripts install");
    assert!(cli_text(&out).contains("lifecycle scripts are ENABLED"));
    let logged = std::fs::read_to_string(&arg_log).expect("npm arg log");
    let last_install = logged
        .lines()
        .rfind(|line| line.starts_with("install "))
        .expect("install invocation logged");
    assert!(
        !last_install.contains("--ignore-scripts"),
        "--allow-scripts must not suppress scripts at the manager boundary: {last_install}"
    );
    // ponytail ceiling: whether the manager then RUNS the scripts is npm's
    // own allowScripts policy (npm ≥ 11.17 gates registry deps regardless
    // of flags), so only Clay's suppression boundary is asserted here.
}

// ─── Drill 4: append once, never enable/adopt/execute ───────────────────────

#[test]
fn exit_gate_install_appends_once_never_enables_or_adopts() {
    let fx = fixture("append-once", false);
    let spec = format!("npm:{FIXTURE_NAME}");
    let out = run_cli(&fx.home, &fx.registry, &["install", &spec]);
    assert_success(&out, "first install");
    assert_eq!(init_block_count(&fx.home), 1);

    // Idempotent append.
    let out = run_cli(&fx.home, &fx.registry, &["install", &spec]);
    assert_success(&out, "second install");
    assert!(cli_text(&out).contains("Load line already present"));
    assert_eq!(init_block_count(&fx.home), 1, "append must stay idempotent");

    // Never enabled, never adopted: list shows installed + pending.
    let out = run_cli(&fx.home, &fx.registry, &["list"]);
    assert_success(&out, "clay list");
    let text = cli_text(&out);
    assert!(text.contains(FIXTURE_NAME), "{text}");
    assert!(text.contains("[pending]"), "must not be adopted: {text}");
    assert!(!text.contains("[enabled]"), "must not be enabled: {text}");
    assert!(
        !store_root(&fx.home)
            .join("clay-package-approvals.json")
            .exists(),
        "no approval record may exist before adopt"
    );
}

// ─── Drill 5: self-update no-op on dev checkout; marker drives fake command ─

#[test]
fn exit_gate_update_self_dev_noop_then_marker_command() {
    let _guard = MarkerGuard;
    let _ = std::fs::remove_file(marker_path());
    let fx = fixture("self-update", false);

    // Dev checkout (no marker beside the binary): documented no-op.
    let out = run_cli(&fx.home, &fx.registry, &["update"]);
    assert_success(&out, "dev no-op self update");
    let text = cli_text(&out);
    assert!(
        text.contains("not managed by an install channel") && text.contains("no channel marker"),
        "{text}"
    );

    // Marker-present checkout: the recorded command runs verbatim (fake
    // command — /bin/sh touches a sentinel; nothing downloads or replaces
    // the binary).
    let touch_target = fx.home.join("selfupdate-ran");
    let _ = std::fs::remove_file(&touch_target);
    clay::packages::self_update::write_channel_marker(
        &marker_path(),
        clay::packages::self_update::ChannelKind::Curl,
        &[
            "sh".to_string(),
            "-c".to_string(),
            format!("touch \"{}\"", touch_target.display()),
        ],
    )
    .expect("write channel marker");
    let out = run_cli(&fx.home, &fx.registry, &["update"]);
    assert_success(&out, "marker-driven self update");
    let text = cli_text(&out);
    assert!(text.contains("Updating Clay via curl channel"), "{text}");
    assert!(text.contains("Clay self-update finished."), "{text}");
    assert!(touch_target.exists(), "recorded command must have run");
}

// ─── Drill 6: publish dry-run docs for @arnilo/clay exist ───────────────────

#[test]
fn exit_gate_publish_dry_run_docs_exist() {
    let manifest_raw = std::fs::read_to_string("distribution/npm/package.json")
        .expect("distribution/npm/package.json must exist");
    let manifest: Value = serde_json::from_str(&manifest_raw).expect("npm wrapper manifest");
    assert_eq!(manifest["name"], "@arnilo/clay");
    assert!(
        manifest["scripts"]
            .as_object()
            .is_none_or(|scripts| scripts.is_empty()),
        "no lifecycle scripts on the npm wrapper"
    );
    for script in ["preinstall", "postinstall", "prepare"] {
        assert!(
            manifest["scripts"][script].is_null(),
            "{script} must not exist on the npm wrapper"
        );
    }
    assert!(
        Path::new("distribution/npm/bin/clay.js").exists(),
        "bin shim must exist"
    );
    let installer = std::fs::read_to_string("distribution/install.sh").expect("curl installer");
    assert!(
        installer.contains("channel.json"),
        "installer must write the channel marker"
    );
    let docs = std::fs::read_to_string("docs/development/distribution.md")
        .expect("distribution docs must exist");
    assert!(
        docs.contains("@arnilo/clay"),
        "docs must cover the npm channel"
    );
    assert!(
        docs.contains("publish"),
        "docs must cover the publish dry-run contract"
    );
}
