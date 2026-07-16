//! Environment-gated real language-server smoke for Phase 18.21.
//!
//! Ordinary `cargo test` must not require host language servers. When
//! `CLAY_LSP_REAL_SMOKE=1`, this suite runs the four first-party package real
//! smokes and skips individual servers with an explicit reason when a binary is
//! missing. Fake-server correctness stays in `tests/lsp_bridge.rs` and
//! `tests/fixtures/lsp/fake-server/`.

use std::path::Path;
use std::process::Command;

fn real_smoke_enabled() -> bool {
    std::env::var_os("CLAY_LSP_REAL_SMOKE").is_some_and(|value| value == "1")
}

fn command_available(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn marksman_available() -> bool {
    if let Ok(path) = std::env::var("MARKSMAN_PATH") {
        return Path::new(&path).is_file();
    }
    Command::new("marksman")
        .arg("--help")
        .output()
        .map(|output| output.status.success() || output.status.code().is_some())
        .unwrap_or(false)
}

fn run_node_tests(files: &[&str]) {
    let mut command = Command::new("node");
    command.arg("--test");
    for file in files {
        command.arg(file);
    }
    let output = command
        .env("CLAY_LSP_REAL_SMOKE", "1")
        .output()
        .expect("Node.js is required for real-server smoke");
    assert!(
        output.status.success(),
        "node --test {} failed\nstdout:\n{}\nstderr:\n{}",
        files.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn real_server_smoke_is_environment_gated_and_documents_skip_contract() {
    if !real_smoke_enabled() {
        // Keep ordinary CI green without host language servers.
        eprintln!(
            "skipping real language-server smoke: set CLAY_LSP_REAL_SMOKE=1 after installing rust-analyzer, typescript-language-server, and marksman"
        );
        return;
    }

    let mut ran = 0usize;

    if command_available("rustup", &["run", "stable", "rust-analyzer", "--version"]) {
        run_node_tests(&["packages/lsp-rust/rust-real-smoke.test.mjs"]);
        ran += 1;
    } else {
        eprintln!("skipping rust-analyzer real smoke: rustup run stable rust-analyzer unavailable");
    }

    if command_available("typescript-language-server", &["--version"]) {
        run_node_tests(&[
            "packages/lsp-typescript/typescript-real-smoke.test.mjs",
            "packages/lsp-javascript/javascript-real-smoke.test.mjs",
        ]);
        ran += 1;
    } else {
        eprintln!(
            "skipping typescript-language-server real smoke: typescript-language-server unavailable on PATH"
        );
    }

    if marksman_available() {
        run_node_tests(&["packages/lsp-markdown/markdown-real-smoke.test.mjs"]);
        ran += 1;
    } else {
        eprintln!("skipping marksman real smoke: marksman unavailable on PATH");
    }

    assert!(
        ran > 0,
        "CLAY_LSP_REAL_SMOKE=1 was set but no supported language server was available"
    );
}

#[test]
fn real_server_workspace_fixtures_exist_for_manual_and_automated_smoke() {
    for path in [
        "tests/fixtures/lsp/rust/Cargo.toml",
        "tests/fixtures/lsp/rust/src/main.rs",
        "tests/fixtures/lsp/typescript/tsconfig.json",
        "tests/fixtures/lsp/typescript/src/main.ts",
        "tests/fixtures/lsp/javascript/jsconfig.json",
        "tests/fixtures/lsp/javascript/src/main.js",
        "tests/fixtures/lsp/markdown/.marksman.toml",
        "tests/fixtures/lsp/markdown/README.md",
        "tests/fixtures/lsp/markdown/other.md",
        "tests/fixtures/lsp/markdown/broken.md",
        "tests/fixtures/lsp/fake-server/server.mjs",
        "tests/fixtures/lsp/fake-server/profiles.mjs",
        "tests/fixtures/lsp/fake-server/session.mjs",
        "tests/fixtures/lsp/workspaces/README.md",
    ] {
        assert!(
            Path::new(path).is_file(),
            "missing LSP workspace/fake-server fixture {path}"
        );
    }
}
