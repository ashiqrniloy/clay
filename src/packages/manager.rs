/// Package manager backend abstraction.
///
/// Clay delegates package download, dependency resolution, lockfile management,
/// integrity verification, caching, and registry access to an npm-compatible
/// package manager.  This module defines the typed boundary that isolates
/// Clay's package-management logic from the concrete package-manager process.
///
/// The pnpm-first implementation is provided by [`PnpmBackend`].  Tests and
/// future alternative managers can substitute a [`FakeBackend`].
///
/// **Hot-path rule**: no method on any backend may be called from typing, paint,
/// layout, scroll, or text-event handlers.  All backend calls are explicit
/// user/agent operations issued off the editing hot path.
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

const PACKAGE_MANAGER_DIAGNOSTIC_LIMIT: usize = 4 * 1024;

// ── Backend trait ────────────────────────────────────────────────────────────

/// Source family for a package spec delegated to the package manager.
///
/// This is the family *claimed by the requested specifier*; it is not a trust
/// decision. Trusted runtime placement comes only from the bundled inventory
/// verification in `crate::packages::bundled`, never from
/// `PackageSourceKind::ClayShipped` or `@clay/*` naming alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceKind {
    ClayShipped,
    NpmRegistry,
    GitHub,
    GitUrl,
    Tarball,
    LocalPath,
}

impl PackageSourceKind {
    pub fn from_spec(spec: &str) -> Self {
        let lower = spec.to_ascii_lowercase();
        if spec.starts_with("@clay/") {
            Self::ClayShipped
        } else if lower.starts_with("github:") {
            Self::GitHub
        } else if lower.ends_with(".tgz") || lower.ends_with(".tar.gz") {
            Self::Tarball
        } else if lower.starts_with("git+")
            || lower.ends_with(".git")
            || lower.contains("github.com/")
        {
            Self::GitUrl
        } else if lower.starts_with("file:")
            || spec.starts_with("./")
            || spec.starts_with("../")
            || spec.starts_with('/')
            || spec.starts_with('~')
        {
            Self::LocalPath
        } else {
            Self::NpmRegistry
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClayShipped => "clay-shipped",
            Self::NpmRegistry => "npm",
            Self::GitHub => "github",
            Self::GitUrl => "git",
            Self::Tarball => "tarball",
            Self::LocalPath => "local-path",
        }
    }
}

/// Source/provenance captured at install or refresh time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageProvenance {
    pub requested_spec: String,
    pub source_kind: PackageSourceKind,
    pub resolved_name: String,
    pub resolved_version: String,
    pub package_root: PathBuf,
    pub lockfile_path: Option<PathBuf>,
    pub integrity: Option<String>,
    pub diagnostics: String,
}

impl PackageProvenance {
    pub fn from_package_json(
        requested_spec: &str,
        package_json: &Value,
        package_root: PathBuf,
        diagnostics: impl AsRef<str>,
    ) -> Self {
        Self {
            requested_spec: requested_spec.to_string(),
            source_kind: PackageSourceKind::from_spec(requested_spec),
            resolved_name: package_json
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(requested_spec)
                .to_string(),
            resolved_version: package_json
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            package_root,
            lockfile_path: None,
            integrity: None,
            diagnostics: sanitize_package_manager_diagnostics(diagnostics.as_ref()),
        }
    }
}

pub fn sanitize_package_manager_diagnostics(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("token")
            || lower.contains("password")
            || lower.contains("authorization")
            || lower.contains("auth=")
            || lower.contains("_auth")
        {
            if output.len() + "[redacted package-manager diagnostic]\n".len()
                > PACKAGE_MANAGER_DIAGNOSTIC_LIMIT
            {
                break;
            }
            output.push_str("[redacted package-manager diagnostic]\n");
            continue;
        }
        let remaining = PACKAGE_MANAGER_DIAGNOSTIC_LIMIT.saturating_sub(output.len());
        if remaining == 0 {
            break;
        }
        if line.len() + 1 > remaining {
            for ch in line.chars() {
                if output.len() + ch.len_utf8() > remaining {
                    break;
                }
                output.push(ch);
            }
            break;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim_end().to_string()
}

/// Result of delegating a `pnpm add` / `npm install`-equivalent operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    /// The package identifier that was installed (e.g. `@clay/markdown@0.1.0`).
    pub package_spec: String,
    /// Whether the underlying package manager reported success.
    pub success: bool,
    /// Captured stdout from the package-manager process (machine-readable where possible).
    pub stdout: String,
    /// Captured stderr from the package-manager process.
    pub stderr: String,
    /// The exit code returned by the package-manager process.
    pub exit_code: Option<i32>,
}

/// Result of discovering installed packages from the package-manager store.
#[derive(Debug, Clone)]
pub struct DiscoveredPackage {
    /// The raw `package.json`-shaped JSON value for the installed package.
    /// Clay will feed this value into the enable/load validator.
    pub package_json: Value,
    /// Resolved path to the package root on disk.
    pub package_root: PathBuf,
    /// Source/provenance captured during install or refresh discovery.
    pub provenance: PackageProvenance,
}

impl DiscoveredPackage {
    pub fn new(requested_spec: &str, package_json: Value, package_root: PathBuf) -> Self {
        let provenance = PackageProvenance::from_package_json(
            requested_spec,
            &package_json,
            package_root.clone(),
            "package discovery",
        );
        Self {
            package_json,
            package_root,
            provenance,
        }
    }
}

/// Typed error from a backend operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    pub kind: BackendErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendErrorKind {
    /// The package-manager process could not be spawned (e.g. not installed).
    ProcessSpawnFailed,
    /// The package-manager process exited with a non-zero code.
    ProcessFailed,
    /// The package-manager produced output that could not be parsed.
    OutputParseFailed,
    /// An I/O error occurred reading or writing to the package store.
    IoError,
}

impl BackendError {
    fn spawn_failed(message: impl Into<String>) -> Self {
        Self {
            kind: BackendErrorKind::ProcessSpawnFailed,
            message: message.into(),
        }
    }

    fn process_failed(message: impl Into<String>) -> Self {
        Self {
            kind: BackendErrorKind::ProcessFailed,
            message: message.into(),
        }
    }

    fn parse_failed(message: impl Into<String>) -> Self {
        Self {
            kind: BackendErrorKind::OutputParseFailed,
            message: message.into(),
        }
    }
}

/// Options controlling package installation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackageInstallOptions {
    /// Allow the package manager to run third-party lifecycle scripts
    /// (`postinstall`, `preinstall`, etc.). Defaults to `false`; enabling this
    /// is dangerous because remote code can execute before Clay validates the
    /// package metadata.
    pub allow_lifecycle_scripts: bool,
}

/// Sealed boundary for npm-compatible package-manager backends.
///
/// Implementors must:
/// - Only be called off the editing hot path (install/enable/disable are user
///   operations, not keypress handlers).
/// - Not grant packages any filesystem/network/shell/AI authority by virtue of
///   running the underlying manager.  Clay validates Clay-owned metadata
///   separately before any package is enabled.
pub trait PackageManagerBackend: Send + Sync {
    /// Install (or update) a package by spec into the Clay-managed package
    /// store.  Does **not** enable the package or execute its runtime.
    fn install(
        &self,
        package_spec: &str,
        store: &PackageStore,
        options: PackageInstallOptions,
    ) -> Result<InstallResult, BackendError>;

    /// Remove a package from the Clay-managed package store.
    fn remove(&self, package_name: &str, store: &PackageStore) -> Result<(), BackendError>;

    /// List all packages currently installed in the Clay-managed package store,
    /// returning their raw `package.json` values for later Clay validation.
    fn list_installed(&self, store: &PackageStore) -> Result<Vec<DiscoveredPackage>, BackendError>;
}

/// Path to the Clay-managed package store (directory containing installed packages).
#[derive(Debug, Clone)]
pub struct PackageStore {
    pub root: PathBuf,
}

impl PackageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

// ── pnpm-first implementation ─────────────────────────────────────────────────

/// Package manager backend that delegates to pnpm.
///
/// Uses `pnpm add <spec>` for installation and `pnpm list --json` for
/// discovery.  All process invocations use [`std::process::Command`] with
/// captured stdout/stderr; the exit code and machine-readable JSON output
/// are parsed into Clay's typed result types.
///
/// Security: pnpm runs with the user's own environment and file permissions.
/// Clay never reads the package contents until `enable` is called, which
/// triggers the Clay-owned validator separately.
pub struct PnpmBackend {
    /// Name or path to the pnpm binary (defaults to `"pnpm"`).
    pub pnpm_bin: String,
}

impl PnpmBackend {
    pub fn new() -> Self {
        Self {
            pnpm_bin: "pnpm".to_string(),
        }
    }

    /// Build the `pnpm add` argument list for the given spec and options.
    /// Exposed for tests so the command shape can be verified without
    /// requiring pnpm to be installed.
    pub fn install_command_args(
        &self,
        package_spec: &str,
        options: PackageInstallOptions,
    ) -> Vec<String> {
        let mut args = vec!["add".to_string(), package_spec.to_string()];
        if !options.allow_lifecycle_scripts {
            // Suppress lifecycle scripts by default. Remote package code must
            // not execute before Clay validates package metadata.
            args.push("--ignore-scripts".to_string());
        }
        args
    }

    fn run(&self, args: &[&str], cwd: &PathBuf) -> Result<Output, BackendError> {
        Command::new(&self.pnpm_bin)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|err| {
                BackendError::spawn_failed(format!("failed to spawn `{}`: {err}", self.pnpm_bin))
            })
    }
}

impl Default for PnpmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerBackend for PnpmBackend {
    fn install(
        &self,
        package_spec: &str,
        store: &PackageStore,
        options: PackageInstallOptions,
    ) -> Result<InstallResult, BackendError> {
        let command_args = self.install_command_args(package_spec, options);
        let args = command_args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        let output = self.run(&args, &store.root)?;
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !success {
            return Err(BackendError::process_failed(format!(
                "`pnpm add {package_spec}` failed (exit {:?}):\n{stderr}",
                output.status.code()
            )));
        }
        Ok(InstallResult {
            package_spec: package_spec.to_string(),
            success: true,
            stdout,
            stderr,
            exit_code: output.status.code(),
        })
    }

    fn remove(&self, package_name: &str, store: &PackageStore) -> Result<(), BackendError> {
        let output = self.run(&["remove", package_name], &store.root)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(BackendError::process_failed(format!(
                "`pnpm remove {package_name}` failed (exit {:?}):\n{stderr}",
                output.status.code()
            )));
        }
        Ok(())
    }

    fn list_installed(&self, store: &PackageStore) -> Result<Vec<DiscoveredPackage>, BackendError> {
        let output = self.run(&["list", "--json", "--long"], &store.root)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(BackendError::process_failed(format!(
                "`pnpm list --json` failed (exit {:?}):\n{stderr}",
                output.status.code()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let list: Vec<Value> = serde_json::from_str(&stdout).map_err(|err| {
            BackendError::parse_failed(format!("failed to parse `pnpm list --json` output: {err}"))
        })?;

        let mut packages = Vec::new();
        for entry in &list {
            // pnpm list --json returns an array; each item has a "dependencies" map.
            if let Some(dependencies) = entry.get("dependencies").and_then(Value::as_object) {
                for (_, dep) in dependencies {
                    if let Some(path) = dep.get("path").and_then(Value::as_str) {
                        let package_root = PathBuf::from(path);
                        let package_json_path = package_root.join("package.json");
                        if let Ok(text) = std::fs::read_to_string(&package_json_path)
                            && let Ok(value) = serde_json::from_str::<Value>(&text)
                        {
                            let requested_spec = dep
                                .get("from")
                                .or_else(|| dep.get("resolved"))
                                .or_else(|| dep.get("name"))
                                .and_then(Value::as_str)
                                .or_else(|| value.get("name").and_then(Value::as_str))
                                .unwrap_or(path);
                            let diagnostics = dep
                                .get("resolved")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let provenance = PackageProvenance::from_package_json(
                                requested_spec,
                                &value,
                                package_root.clone(),
                                diagnostics,
                            );
                            packages.push(DiscoveredPackage {
                                package_json: value,
                                package_root,
                                provenance,
                            });
                        }
                    }
                }
            }
        }

        Ok(packages)
    }
}

// ── Fake backend for testing ─────────────────────────────────────────────────

/// Fake backend for unit and integration tests.
///
/// Stores configured install results and discovered packages in memory without
/// spawning any process.  Tests inject this backend into [`crate::packages::service::PackageService`].
pub struct FakeBackend {
    /// Canned install results keyed by package spec.
    pub install_results: std::collections::HashMap<String, Result<InstallResult, BackendError>>,
    /// Canned list results.
    pub list_results: Vec<DiscoveredPackage>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self {
            install_results: std::collections::HashMap::new(),
            list_results: Vec::new(),
        }
    }

    /// Configure a successful install for a package spec with a given `package.json` value.
    pub fn will_install(mut self, package_spec: &str, package_json: Value) -> Self {
        let package_root = PathBuf::from(format!("/fake/store/{package_spec}"));
        let provenance = PackageProvenance::from_package_json(
            package_spec,
            &package_json,
            package_root.clone(),
            format!("Packages: +1\n{package_spec}\n"),
        );
        self.list_results.push(DiscoveredPackage {
            package_json: package_json.clone(),
            package_root,
            provenance,
        });
        self.install_results.insert(
            package_spec.to_string(),
            Ok(InstallResult {
                package_spec: package_spec.to_string(),
                success: true,
                stdout: format!("Packages: +1\n{package_spec}\n"),
                stderr: String::new(),
                exit_code: Some(0),
            }),
        );
        self
    }

    /// Configure a failed install for a package spec.
    pub fn will_fail_install(mut self, package_spec: &str, reason: &str) -> Self {
        self.install_results.insert(
            package_spec.to_string(),
            Err(BackendError::process_failed(reason.to_string())),
        );
        self
    }
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerBackend for FakeBackend {
    fn install(
        &self,
        package_spec: &str,
        _store: &PackageStore,
        _options: PackageInstallOptions,
    ) -> Result<InstallResult, BackendError> {
        // Fake backend never spawns a process, so lifecycle scripts are never
        // executed regardless of options.
        self.install_results
            .get(package_spec)
            .cloned()
            .unwrap_or_else(|| {
                Err(BackendError::process_failed(format!(
                    "fake backend has no configured result for `{package_spec}`"
                )))
            })
    }

    fn remove(&self, _package_name: &str, _store: &PackageStore) -> Result<(), BackendError> {
        // Fake remove always succeeds.
        Ok(())
    }

    fn list_installed(
        &self,
        _store: &PackageStore,
    ) -> Result<Vec<DiscoveredPackage>, BackendError> {
        Ok(self.list_results.clone())
    }
}
