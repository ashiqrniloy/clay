//! Binary provisioning table (plan 115 task 7, decision
//! `2026-09-08-2141-binary-provisioning-deny-by-default-table.md`).
//!
//! One row per external binary a Clay feature references. Presence checks
//! reuse each feature's fail-closed resolution rules (explicit configured
//! path → PATH → documented absolute fallback); installs route through the
//! shared npm-compatible manager backend into the Clay-owned package store
//! (binaries land in `<store>/node_modules/.bin`) and never touch package
//! records, grants, or the install ledger. Features keep their own
//! resolution unchanged — provisioning only places files.
//!
//! Deny-by-default: nothing is installed without `--yes` on the same
//! invocation, and lifecycle scripts stay off unless `--allow-scripts`.
//! Presence-only entries print the manual command and are never spawned.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::packages::manager::{
    ManagerKind, PackageInstallOptions, PackageStore, resolve_manager_backend,
};

#[derive(Clone)]
pub struct BinaryEntry {
    /// Binary name features resolve on PATH.
    pub name: &'static str,
    /// Feature that uses the binary (what breaks without it).
    pub feature: &'static str,
    /// Explicit configured-path environment variable checked before PATH
    /// (mirrors `clay-agent/src/resolve-obscura.ts`).
    pub env_override: Option<&'static str>,
    /// Documented absolute fallback when PATH has no entry (obscura only).
    pub fallback_path: Option<&'static str>,
    /// npm-registry spec Clay can install with permission; `None` means
    /// presence-only: Clay prints the manual command and never spawns.
    pub install_spec: Option<&'static str>,
    /// Exact manual install command shown for absent binaries.
    pub manual_install: &'static str,
}

/// Binaries Clay features reference today (task 1 inventory + roadmap).
/// Adding a binary is a one-line table change.
pub const BINARY_INVENTORY: &[BinaryEntry] = &[
    BinaryEntry {
        name: "obscura",
        feature: "web engine (obscura_fetch/obscura_scrape, CDP browser automation)",
        env_override: Some("CLAY_OBSCURA_BIN"),
        fallback_path: Some("/usr/local/bin/obscura"),
        install_spec: None,
        manual_install: "host-installed binary — no npm binary package; install per docs/development (obscura)",
    },
    BinaryEntry {
        name: "graft",
        feature: "context graph (graft repo-context MCP tools)",
        env_override: None,
        fallback_path: None,
        install_spec: Some("npm:@nanonets/graft"),
        manual_install: "npm install -g @nanonets/graft",
    },
    BinaryEntry {
        name: "qmd",
        feature: "wiki hybrid search (opt-in, host-owned binary)",
        env_override: None,
        fallback_path: None,
        install_spec: None,
        manual_install: "host-installed binary — no npm binary package; install per docs/development (qmd)",
    },
    BinaryEntry {
        name: "ripgrep",
        feature: "(no Clay feature references it today; roadmap mention only)",
        env_override: None,
        fallback_path: None,
        install_spec: None,
        manual_install: "install via your system package manager",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// Resolved absolutely: explicit configured path, a PATH hit, or the
    /// documented fallback.
    Present {
        path: PathBuf,
        via: &'static str,
    },
    Absent,
}

/// Resolve a binary exactly the way its feature does: explicit configured
/// path (absolute only) → PATH → documented fallback. Never guesses.
pub fn resolve(entry: &BinaryEntry) -> Presence {
    resolve_with(
        entry,
        |env_var| std::env::var_os(env_var),
        std::env::var_os("PATH"),
    )
}

fn resolve_with(
    entry: &BinaryEntry,
    env_lookup: impl Fn(&str) -> Option<std::ffi::OsString>,
    path_value: Option<std::ffi::OsString>,
) -> Presence {
    if let Some(env_var) = entry.env_override
        && let Some(value) = env_lookup(env_var)
    {
        let candidate = PathBuf::from(&value);
        if candidate.is_absolute() {
            return if candidate.is_file() {
                Presence::Present {
                    path: candidate,
                    via: "explicit configured path",
                }
            } else {
                // Absolute override set but missing: fail closed to
                // absent, matching resolve-obscura semantics.
                Presence::Absent
            };
        }
        // Non-absolute override: ignored, fall through to PATH.
    }
    if let Some(hit) = path_hit(&path_value, entry.name) {
        return Presence::Present {
            path: hit,
            via: "PATH",
        };
    }
    if let Some(fallback) = entry.fallback_path {
        let candidate = PathBuf::from(fallback);
        if candidate.is_file() {
            return Presence::Present {
                path: candidate,
                via: "documented fallback path",
            };
        }
    }
    Presence::Absent
}

fn path_hit(path_value: &Option<std::ffi::OsString>, name: &str) -> Option<PathBuf> {
    let path = path_value.as_ref()?;
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Where npm-provisioned binaries land (same store as packages).
pub fn bin_dir(store_root: &Path) -> PathBuf {
    store_root.join("node_modules").join(".bin")
}

fn kind_for_display() -> Result<ManagerKind, String> {
    resolve_manager_backend()
        .map(|(kind, _)| kind)
        .map_err(|error| error.message)
}

/// Full-table status report. Never spawns a process.
pub fn check(out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "Binary check (features resolve these fail-closed; Clay installs only with explicit approval):").map_err(io_err)?;
    for entry in BINARY_INVENTORY {
        writeln!(out, "  {} — {}", entry.name, entry.feature).map_err(io_err)?;
        match resolve(entry) {
            Presence::Present { path, via } => {
                writeln!(out, "    present ({via}): {}", path.display()).map_err(io_err)?;
            }
            Presence::Absent => {
                writeln!(out, "    absent").map_err(io_err)?;
                match entry.install_spec {
                    Some(spec) => {
                        let kind = kind_for_display()?;
                        let command = kind.install_argv_display(
                            &crate::packages::service::default_store_root(),
                            spec,
                            PackageInstallOptions::default(),
                        );
                        writeln!(
                            out,
                            "    install with permission: clay install --bin {tname} --yes",
                            tname = entry.name
                        )
                        .map_err(io_err)?;
                        writeln!(out, "    would run: {command}").map_err(io_err)?;
                    }
                    None => {
                        writeln!(out, "    manual install: {}", entry.manual_install)
                            .map_err(io_err)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Permissioned install of one binary. `approved` must come only from an
/// explicit `--yes` on the same invocation; without it this prints the
/// command and changes nothing.
pub fn provision(
    entry: &BinaryEntry,
    approved: bool,
    allow_scripts: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let Some(spec) = entry.install_spec else {
        writeln!(
            out,
            "{} is presence-only; Clay never installs it. Manual install: {}",
            entry.name, entry.manual_install
        )
        .map_err(io_err)?;
        return Ok(());
    };
    let (kind, backend) = resolve_manager_backend().map_err(|error| error.message)?;
    let store = crate::packages::service::default_store_root();
    let options = PackageInstallOptions {
        allow_lifecycle_scripts: allow_scripts,
    };
    let command = kind.install_argv_display(&store, spec, options);
    if !approved {
        writeln!(
            out,
            "Refusing to install {} without explicit approval.",
            entry.name
        )
        .map_err(io_err)?;
        writeln!(out, "Would run: {command}").map_err(io_err)?;
        writeln!(
            out,
            "Re-run with `clay install --bin {} --yes` to approve this exact command.",
            entry.name
        )
        .map_err(io_err)?;
        return Ok(());
    }
    writeln!(out, "Running: {command}").map_err(io_err)?;
    let result = backend
        .install(spec, &PackageStore::new(&store), options)
        .map_err(|error| error.message)?;
    if !result.success {
        return Err(format!(
            "provisioning {} failed: {}{}",
            entry.name,
            sanitize_result(&result.stdout),
            sanitize_result(&result.stderr)
        ));
    }
    writeln!(
        out,
        "Installed {spec} via {}; binary (if shipped) at {}",
        kind.as_str(),
        bin_dir(&store).display()
    )
    .map_err(io_err)?;
    writeln!(
        out,
        "Scripts were {}.",
        if allow_scripts {
            "allowed (--allow-scripts)"
        } else {
            "suppressed (--ignore-scripts)"
        }
    )
    .map_err(io_err)?;
    writeln!(
        out,
        "Remove with: npm remove --prefix {} {}",
        store.display(),
        crate::packages::manager::PackageSpec::parse(spec)
            .map(|parsed| parsed.name)
            .unwrap_or_else(|_| spec.to_string())
    )
    .map_err(io_err)?;
    writeln!(
        out,
        "Feature resolution is unchanged: features keep their own fail-closed lookup (provisioning places files only)."
    )
    .map_err(io_err)?;
    Ok(())
}

fn sanitize_result(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> &BinaryEntry {
        BINARY_INVENTORY
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("table entry {name}"))
    }

    #[test]
    fn table_covers_inventory_and_stays_presence_only_where_unverified() {
        let names: Vec<&str> = BINARY_INVENTORY.iter().map(|entry| entry.name).collect();
        assert_eq!(names, vec!["obscura", "graft", "qmd", "ripgrep"]);
        for entry in BINARY_INVENTORY {
            assert!(!entry.feature.is_empty());
            assert!(!entry.manual_install.is_empty());
            if entry.install_spec.is_none() {
                // Presence-only entries must name the manual step.
                assert!(!entry.manual_install.starts_with("npm install -g"));
            }
        }
        assert!(entry("graft").install_spec == Some("npm:@nanonets/graft"));
    }

    #[test]
    fn presence_resolves_explicit_path_then_fallback_fail_closed() {
        let probe = BinaryEntry {
            name: "clay-probe-absent-binary",
            feature: "test",
            env_override: Some("CLAY_PROBE_BIN"),
            fallback_path: None,
            install_spec: None,
            manual_install: "manual",
        };
        let no_env = |_var: &str| None;
        let empty_path: Option<std::ffi::OsString> = None;
        assert_eq!(
            resolve_with(&probe, no_env, empty_path.clone()),
            Presence::Absent
        );

        // Absolute override set but missing: fail closed to absent.
        let missing = std::env::temp_dir().join("clay-probe-missing-binary");
        let _ = std::fs::remove_file(&missing);
        let missing_env = |_var: &str| Some(missing.clone().into_os_string());
        assert_eq!(
            resolve_with(&probe, missing_env, empty_path.clone()),
            Presence::Absent
        );

        // Absolute override present: explicit configured path.
        std::fs::write(&missing, b"").unwrap();
        assert!(matches!(
            resolve_with(&probe, missing_env, empty_path.clone()),
            Presence::Present {
                via: "explicit configured path",
                ..
            }
        ));
        let _ = std::fs::remove_file(&missing);

        // PATH hit beats fallback.
        let path_dir = std::env::temp_dir().join(format!("clay-probe-path-{}", std::process::id()));
        std::fs::create_dir_all(&path_dir).unwrap();
        let path_bin = path_dir.join(probe.name);
        std::fs::write(&path_bin, b"").unwrap();
        let path_value = std::env::join_paths([&path_dir]).unwrap();
        assert!(matches!(
            resolve_with(&probe, no_env, Some(path_value.clone())),
            Presence::Present { via: "PATH", .. }
        ));

        // Documented fallback stands in when PATH misses (obscura rule).
        let fallback_dir =
            std::env::temp_dir().join(format!("clay-probe-fb-{}", std::process::id()));
        std::fs::create_dir_all(&fallback_dir).unwrap();
        let fallback_bin = fallback_dir.join("clay-probe-fallback-binary");
        std::fs::write(&fallback_bin, b"").unwrap();
        let leaked: &'static str =
            Box::leak(fallback_bin.to_string_lossy().into_owned().into_boxed_str());
        let with_fallback = BinaryEntry {
            fallback_path: Some(leaked),
            ..probe.clone()
        };
        assert!(matches!(
            resolve_with(&with_fallback, no_env, empty_path.clone()),
            Presence::Present {
                via: "documented fallback path",
                ..
            }
        ));

        let _ = std::fs::remove_file(&path_bin);
        let _ = std::fs::remove_dir_all(&path_dir);
        let _ = std::fs::remove_file(&fallback_bin);
        let _ = std::fs::remove_dir_all(&fallback_dir);
    }

    #[test]
    fn check_reports_absent_and_never_spawns() {
        let mut out = Vec::new();
        check(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // Every table row renders with its feature line; present/absent is
        // host-state dependent (graft may legitimately be installed here).
        for entry in BINARY_INVENTORY {
            assert!(text.contains(entry.name), "missing row: {}", entry.name);
            assert!(text.contains(entry.feature));
        }
        assert!(text.contains("manual install:"));
    }

    #[test]
    fn provision_without_approval_prints_command_and_changes_nothing() {
        let mut out = Vec::new();
        provision(entry("graft"), false, false, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Refusing to install graft"));
        assert!(text.contains("Would run:"));
        assert!(text.contains("@nanonets/graft"));
        assert!(text.contains("--ignore-scripts"));
        assert!(text.contains("--yes"));
    }

    #[test]
    fn provision_presence_only_never_spawns() {
        let mut out = Vec::new();
        provision(entry("obscura"), true, true, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("presence-only"));
        assert!(text.contains("Manual install:"));
    }
}
