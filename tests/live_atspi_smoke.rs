//! Environment-gated live AT-SPI accessibility smoke (plan 086 task 4).
//!
//! Ordinary `cargo test` never runs this test: it is `#[ignore]` AND gated
//! on `CLAY_LIVE_A11Y_SMOKE=1`. When enabled it must run on a Linux desktop
//! session with a live AT-SPI bus and Python 3 with the AT-SPI GI bindings
//! (`python3-gi` + `gir1.2-atspi-2.0`). Missing prerequisites print a skip
//! reason and return — never a false pass.
//!
//! The check launches the real `clay server` + `clay client` binaries with
//! a mode-700 temporary IPC/config home (never the ambient
//! `~/.config/clay` or default socket), restores a two-tab window (the
//! live two-tab tree shape that panicked before plan 086 task 3), and
//! requires the window to survive startup, expose its accessibility tree
//! over the real AT-SPI bus, keep the same node identities across a second
//! query, and stay alive until the deadline. Every failure path kills both
//! children and removes the temporary directory.

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn live_smoke_enabled() -> bool {
    std::env::var_os("CLAY_LIVE_A11Y_SMOKE").is_some_and(|value| value == "1")
}

/// Python probe: `prereq` mode verifies the GI AT-SPI bindings and a
/// reachable desktop bus; `dump` mode prints every node as
/// `depth|role|selected|name|object-path|application`.
const PROBE_SCRIPT: &str = r#"
import sys
try:
    import gi
    gi.require_version('Atspi', '2.0')
    from gi.repository import Atspi
except Exception as exc:
    print(f'PREREQ_MISSING: {exc}', file=sys.stderr)
    sys.exit(3)

if sys.argv[1] == 'prereq':
    desktop = Atspi.get_desktop(0)
    print(f'OK apps={desktop.get_child_count()}')
    sys.exit(0)

desktop = Atspi.get_desktop(0)
lines = []

def walk(node, depth):
    if node is None:
        return
    try:
        role = node.get_role_name()
        name = node.get_name()
        path = node.path
        app = node.get_application()
        app_name = app.get_name() if app is not None else ''
        selected = 'S' if node.get_state_set().contains(Atspi.StateType.SELECTED) else '-'
        lines.append(f'{depth}|{role}|{selected}|{name}|{path}|{app_name}')
    except Exception as exc:
        lines.append(f'{depth}|ERROR|{exc}')
        return
    count = node.get_child_count()
    for i in range(count):
        try:
            child = node.get_child_at_index(i)
        except Exception:
            continue
        walk(child, depth + 1)

for i in range(desktop.get_child_count()):
    walk(desktop.get_child_at_index(i), 0)
print('\n'.join(lines))
"#;

/// Kills every spawned child on any exit path.
struct KillGuard {
    children: Vec<Child>,
}

impl KillGuard {
    fn spawn(&mut self, mut command: Command) {
        self.children.push(
            command.spawn().unwrap_or_else(|error| {
                panic!("spawn {}: {error}", command.get_program().display())
            }),
        );
    }

    /// True when the child is still running.
    fn alive(&mut self, index: usize) -> bool {
        self.children[index].try_wait().ok().flatten().is_none()
    }

    fn exit_status(&mut self, index: usize) -> Option<std::process::ExitStatus> {
        self.children[index].try_wait().ok().flatten()
    }
}

impl Drop for KillGuard {
    fn drop(&mut self) {
        for child in self.children.iter_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Mode-700 user-owned temporary IPC/config directory.
fn make_private_temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "clay-live-a11y-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos()
    ));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .unwrap_or_else(|error| panic!("create {}: {error}", dir.display()));
    dir
}

fn run_probe(script: &Path, mode: &str) -> Result<String, String> {
    let output = Command::new("python3")
        .arg(script)
        .arg(mode)
        .output()
        .map_err(|error| format!("python3 unavailable: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "python probe {mode} failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn wait_until(
    mut check: impl FnMut() -> bool,
    timeout: Duration,
    what: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!("timed out waiting for {what} ({timeout:?})"))
}

fn socket_connectable(path: &Path) -> bool {
    use std::os::unix::net::UnixStream;
    UnixStream::connect(path).is_ok()
}

#[derive(Debug, Clone)]
struct ProbeNode {
    depth: usize,
    role: String,
    selected: bool,
    name: String,
    path: String,
    app: String,
}

fn parse_dump(dump: &str) -> Vec<ProbeNode> {
    dump.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(6, '|').collect();
            if fields.len() != 6 {
                return None;
            }
            Some(ProbeNode {
                depth: fields[0].parse().unwrap_or(0),
                role: fields[1].to_string(),
                selected: fields[2] == "S",
                name: fields[3].to_string(),
                path: fields[4].to_string(),
                app: fields[5].to_string(),
            })
        })
        .collect()
}

fn clay_nodes(nodes: &[ProbeNode]) -> Vec<&ProbeNode> {
    nodes.iter().filter(|node| node.app == "clay").collect()
}

fn find<'a>(nodes: &'a [&'a ProbeNode], role: &str, name_contains: &str) -> Option<&'a ProbeNode> {
    nodes.iter().copied().find(|node| {
        node.role == role && (name_contains.is_empty() || node.name.contains(name_contains))
    })
}

fn find_children<'a>(nodes: &'a [ProbeNode], parent: &ProbeNode) -> Vec<&'a ProbeNode> {
    nodes
        .iter()
        .filter(|node| node.app == "clay" && node.depth == parent.depth + 1)
        .collect()
}

fn expect_clay_shell(clay: &[&ProbeNode]) {
    if find(clay, "panel", "Clay working area shell").is_none() {
        panic!("clay accessibility tree missing shell panel; clay nodes: {clay:#?}");
    }
}

#[test]
#[ignore = "requires a live Linux desktop with an AT-SPI bus (CLAY_LIVE_A11Y_SMOKE=1)"]
fn live_atspi_accessibility_smoke() {
    if !live_smoke_enabled() {
        eprintln!(
            "skipping live AT-SPI smoke: set CLAY_LIVE_A11Y_SMOKE=1 on a desktop Linux session to run it"
        );
        return;
    }

    // Prerequisites: python3 + GI AT-SPI bindings + reachable desktop bus.
    let script_dir = make_private_temp_dir();
    let script = script_dir.join("atspi_probe.py");
    fs::write(&script, PROBE_SCRIPT).expect("write probe script");
    let prereq = run_probe(&script, "prereq");
    let prereq = match prereq {
        Ok(output) => output,
        Err(reason) => {
            eprintln!(
                "skipping live AT-SPI smoke (prerequisite missing, never a false pass): {reason}"
            );
            let _ = fs::remove_dir_all(&script_dir);
            return;
        }
    };
    eprintln!(
        "live AT-SPI smoke: prerequisites present ({})",
        prereq.trim()
    );

    // Isolated mode-700 IPC/config homes; never the ambient defaults.
    let config_home = script_dir.join("config");
    let data_home = script_dir.join("data");
    fs::create_dir_all(config_home.join("clay")).expect("create config home");
    fs::create_dir_all(&data_home).expect("create data home");
    let socket = script_dir.join("a11y-smoke.sock");

    let binary = env!("CARGO_BIN_EXE_clay");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configuration");

    let mut guard = KillGuard {
        children: Vec::new(),
    };
    let result = (|| -> Result<(), String> {
        // 1. Isolated server on the fixture config.
        guard.spawn({
            let mut command = Command::new(binary);
            command
                .arg("server")
                .arg(&socket)
                .arg("--config-fixture")
                .arg("runtime-sdui")
                .env("XDG_CONFIG_HOME", &config_home)
                .env("XDG_DATA_HOME", &data_home);
            command
        });
        wait_until(
            || socket_connectable(&socket),
            Duration::from_secs(20),
            "server socket",
        )?;

        // 2. Two-tab restore state: tab 0 = runtime-sdui fixture (bootstrap),
        //    tab 1 = syntax-grammars fixture. v2 layout document.
        let workspace_a = fixture_root.join("runtime-sdui");
        let workspace_b = fixture_root.join("syntax-grammars");
        let layout = format!(
            r#"{{
  "version": 2,
  "activeTab": 0,
  "tabs": [
    {{"workspaceRoot": "{}", "activePane": 1, "splitTree": null, "slots": [], "panes": {{}}}},
    {{"workspaceRoot": "{}", "activePane": 1, "splitTree": null, "slots": [], "panes": {{}}}}
  ]
}}"#,
            workspace_a.display(),
            workspace_b.display()
        );
        fs::write(config_home.join("clay/layout.json"), layout).expect("write layout.json");

        // 3. Client window: connects to the isolated server, restores two tabs.
        guard.spawn({
            let mut command = Command::new(binary);
            command
                .arg("client")
                .arg(&socket)
                .env("XDG_CONFIG_HOME", &config_home)
                .env("XDG_DATA_HOME", &data_home);
            command
        });

        // 4. The real AT-SPI tree must expose the window and it must survive.
        let mut latest_dump = String::new();
        wait_until(
            || {
                if !guard.alive(0) {
                    return true; // fail-fast below with a clear message
                }
                match run_probe(&script, "dump") {
                    Ok(dump) => {
                        latest_dump = dump.clone();
                        let nodes = parse_dump(&dump);
                        let clay = clay_nodes(&nodes);
                        find(&clay, "frame", "").is_some()
                            && find(&clay, "panel", "Clay working area shell").is_some()
                    }
                    Err(_) => false,
                }
            },
            Duration::from_secs(45),
            "clay window in the AT-SPI tree",
        )
        .map_err(|error| {
            let died = if guard.alive(0) {
                String::new()
            } else {
                format!(" (server exited with {:?})", guard.exit_status(0))
            };
            format!("{error}{died}; latest tree:\n{latest_dump}")
        })?;

        // 5. Restored two-tab shape: TabList with both cards, one selected,
        //    active pane, status line, attached server-driven region.
        //    Restore mounts tab 1 after registry confirmation, so poll for
        //    the two-card bar instead of asserting the first snapshot.
        wait_until(
            || {
                if !guard.alive(0) {
                    return true; // fail-fast below
                }
                match run_probe(&script, "dump") {
                    Ok(dump) => {
                        latest_dump = dump.clone();
                        let nodes = parse_dump(&dump);
                        let clay = clay_nodes(&nodes);
                        find(&clay, "page tab list", "Workspace tabs").is_some()
                    }
                    Err(_) => false,
                }
            },
            Duration::from_secs(30),
            "restored two-tab TabList",
        )
        .map_err(|error| format!("{error}; latest tree:\n{latest_dump}"))?;
        let nodes = parse_dump(&latest_dump);
        let clay = clay_nodes(&nodes);
        expect_clay_shell(&clay);
        let tab_list = find(&clay, "page tab list", "Workspace tabs").ok_or_else(|| {
            format!(
                "no TabList 'Workspace tabs' in tree (restore failed?): {:#?}",
                clay
            )
        })?;
        let tab_children: Vec<&ProbeNode> = find_children(&nodes, tab_list)
            .into_iter()
            .filter(|node| node.role == "page tab")
            .collect();
        assert_eq!(
            tab_children.len(),
            2,
            "expected 2 restored Tab cards, got {tab_children:#?}"
        );
        for expected in ["runtime-sdui", "syntax-grammars"] {
            assert!(
                tab_children.iter().any(|tab| tab.name == expected),
                "missing restored Tab card '{expected}': {tab_children:#?}"
            );
        }
        assert_eq!(
            tab_children.iter().filter(|tab| tab.selected).count(),
            1,
            "exactly one restored Tab must be selected: {tab_children:#?}"
        );
        let status = find(&clay, "status bar", "Clay —")
            .ok_or_else(|| format!("no status line in tree: {clay:#?}"))?;
        assert!(
            status.name.contains("Connected"),
            "status line must show a live connection: {:?}",
            status.name
        );
        // The P0 crash: the server-driven region must be attached and the
        // window alive after the two-tab restore.
        find(&clay, "panel", "Server-driven UI region")
            .ok_or_else(|| format!("region not attached: {clay:#?}"))?;
        let pane = find(&clay, "panel", "Pane 1 of 1")
            .ok_or_else(|| format!("no pane label in tree: {clay:#?}"))?;
        let _ = pane;

        // 6. Identity stability: a second query must expose the same object
        //    paths (stable virtual node identities; churn would renumber).
        std::thread::sleep(Duration::from_secs(2));
        let second = run_probe(&script, "dump").map_err(|error| error.to_string())?;
        let first_paths: Vec<String> = clay_nodes(&parse_dump(&latest_dump))
            .iter()
            .map(|node| node.path.clone())
            .collect();
        let second_paths: Vec<String> = clay_nodes(&parse_dump(&second))
            .iter()
            .map(|node| node.path.clone())
            .collect();
        assert_eq!(
            first_paths, second_paths,
            "clay node identities churned between queries"
        );

        // 7. Still alive at the deadline with the tree still queryable.
        assert!(guard.alive(0), "server exited during the smoke");
        eprintln!("live AT-SPI smoke: PASS (window alive, tree stable, 2 tabs restored)");
        Ok(())
    })();

    drop(guard); // kill client + server before removing the temp home
    let _ = fs::remove_dir_all(&script_dir);
    if let Err(error) = result {
        panic!("live AT-SPI smoke failed: {error}");
    }
}
