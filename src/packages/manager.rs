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
            let redacted = sanitize_package_manager_diagnostics(&stderr);
            return Err(BackendError::process_failed(format!(
                "`pnpm add {package_spec}` failed (exit {:?}):\n{redacted}",
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
            let redacted =
                sanitize_package_manager_diagnostics(&String::from_utf8_lossy(&output.stderr));
            return Err(BackendError::process_failed(format!(
                "`pnpm remove {package_name}` failed (exit {:?}):\n{redacted}",
                output.status.code()
            )));
        }
        Ok(())
    }

    fn list_installed(&self, store: &PackageStore) -> Result<Vec<DiscoveredPackage>, BackendError> {
        let output = self.run(&["list", "--json", "--long"], &store.root)?;
        if !output.status.success() {
            let redacted =
                sanitize_package_manager_diagnostics(&String::from_utf8_lossy(&output.stderr));
            return Err(BackendError::process_failed(format!(
                "`pnpm list --json` failed (exit {:?}):\n{redacted}",
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

// ── npm spec model (v1 install source) ───────────────────────────────────────

/// Error parsing a package specifier.
///
/// v1 (plan 115) accepts exactly one install source: the npm registry, in
/// the `npm:`-prefixed form. Everything else is rejected at parse time with
/// a message naming the accepted form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSpecError {
    /// The specifier is not an `npm:` registry specifier (or is empty).
    MissingNpmPrefix { spec: String },
    /// The package name part is not a valid npm package name.
    InvalidName { spec: String },
    /// The version part is not an exact version (ranges are unsupported in v1).
    InvalidVersion { spec: String, version: String },
}

impl std::fmt::Display for PackageSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNpmPrefix { spec } => write!(
                f,
                "`{spec}` is not an npm registry specifier: v1 accepts only `npm:<name>`, `npm:@scope/name`, `npm:<name>@<version>`, `npm:@scope/name@<version>`",
            ),
            Self::InvalidName { spec } => {
                write!(f, "invalid package name in specifier `{spec}`")
            }
            Self::InvalidVersion { spec, version } => write!(
                f,
                "invalid pinned version `{version}` in specifier `{spec}`: v1 pins are exact versions like `1.2.3` (ranges such as `^1.2.3` are unsupported)",
            ),
        }
    }
}

impl std::error::Error for PackageSpecError {}

/// A parsed `npm:` registry specifier — the only install source v1 accepts.
///
/// The original spec string is what the delegated manager receives; this
/// model exists for parse-time validation, exact-name matching after install,
/// and the pinned/floating distinction the update family needs (task 6).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PackageSpec {
    /// Bare resolved package name (`@arnilo/st` or `markdown`).
    pub name: String,
    /// Exact version pin (`1.2.3`); `None` = floating spec.
    pub version: Option<String>,
}

impl PackageSpec {
    /// Parse the v1 `npm:` form. Rejects every other source family with a
    /// typed error naming the accepted form; no install path accepts a
    /// non-npm source.
    pub fn parse(raw: &str) -> Result<Self, PackageSpecError> {
        let Some(body) = raw.strip_prefix("npm:") else {
            return Err(PackageSpecError::MissingNpmPrefix {
                spec: raw.to_string(),
            });
        };
        if body.is_empty() {
            return Err(PackageSpecError::MissingNpmPrefix {
                spec: raw.to_string(),
            });
        }
        let (name, version) = split_name_version(body);
        if !is_valid_name(name) {
            return Err(PackageSpecError::InvalidName {
                spec: raw.to_string(),
            });
        }
        let version = match version {
            None => None,
            Some(version) => {
                if !is_exact_version(version) {
                    return Err(PackageSpecError::InvalidVersion {
                        spec: raw.to_string(),
                        version: version.to_string(),
                    });
                }
                Some(version.to_string())
            }
        };
        Ok(Self {
            name: name.to_string(),
            version,
        })
    }

    /// Canonical spec string (`npm:<name>[@<version>]`).
    pub fn to_spec(&self) -> String {
        match &self.version {
            Some(version) => format!("npm:{}@{version}", self.name),
            None => format!("npm:{}", self.name),
        }
    }
}

/// Split `npm:`-body into (name, optional version). The version separator is
/// the `@` after the scope part for scoped names — `"@arnilo/st@1.2.3"` must
/// not split at the leading `@`.
fn split_name_version(body: &str) -> (&str, Option<&str>) {
    let at = if body.starts_with('@') {
        body.find('/')
            .and_then(|slash| body[slash + 1..].find('@').map(|offset| slash + 1 + offset))
    } else {
        body.find('@')
    };
    match at {
        None => (body, None),
        Some(index) => (&body[..index], Some(&body[index + 1..])),
    }
}

fn is_valid_name(name: &str) -> bool {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_ascii_uppercase())
    {
        return false;
    }
    let valid = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | '/')
    };
    // Scoped names carry exactly one leading `@`; nothing after it may.
    let (scoped, body) = match name.strip_prefix('@') {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    if !body.chars().all(valid) || body.contains('@') {
        return false;
    }
    if scoped {
        // Scoped: exactly one `/`, non-empty scope and package parts.
        let mut parts = body.split('/');
        let scope = parts.next().unwrap_or("");
        let package = parts.next();
        !scope.is_empty()
            && package.is_some_and(|package| !package.is_empty())
            && parts.next().is_none()
    } else {
        !name.contains('/')
    }
}

/// v1 pin: an exact dotted version (`1.2.3`, optional `-pre`/`+build` suffix).
/// Ranges (`^1.2.3`, `~1.2.3`, `1.2`, `1.x`, `latest`) are rejected so a
/// pinned spec is a genuine identity, which is what the update family's
/// skip-pinned semantics rely on.
fn is_exact_version(version: &str) -> bool {
    if version.is_empty() {
        return false;
    }
    let head = version.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = head.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

// ── Manager selection ────────────────────────────────────────────────────────

/// Which npm-compatible manager a verb runs against (observable by tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerKind {
    Pnpm,
    Npm,
}

impl ManagerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm",
            Self::Npm => "npm",
        }
    }

    /// Human-readable form of the argv [`PackageManagerBackend::install`]
    /// spawns, mirroring each backend's `install_command_args` shape
    /// (npm installs with `--prefix <store>`; pnpm runs with the store as
    /// cwd). Used to name exactly what a provisioning command will run.
    pub fn install_argv_display(
        self,
        store_root: &std::path::Path,
        package_spec: &str,
        options: PackageInstallOptions,
    ) -> String {
        let scripts = if options.allow_lifecycle_scripts {
            String::new()
        } else {
            " --ignore-scripts".to_string()
        };
        match self {
            Self::Npm => format!(
                "npm install --prefix {} {package_spec}{scripts}",
                store_root.display()
            ),
            Self::Pnpm => format!(
                "pnpm add {package_spec}{scripts}  # cwd {}",
                store_root.display()
            ),
        }
    }
}

/// Select the package-manager backend for a CLI verb.
///
/// v1 policy: pnpm when present on `PATH`, otherwise npm (npm ships with the
/// Node ≥ 20 prerequisite clay-agent already requires). An explicit
/// `CLAY_PACKAGE_MANAGER=pnpm|npm` override wins; an unknown value is a
/// typed fail-closed error. When neither binary is present the resolution
/// fails closed with a typed spawn error before any package work starts.
///
/// The probe is a `PATH` scan — it spawns no process — so each verb still
/// runs exactly one package-manager process.
pub fn resolve_manager_backend()
-> Result<(ManagerKind, Box<dyn PackageManagerBackend>), BackendError> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let manager_override = std::env::var("CLAY_PACKAGE_MANAGER").ok();
    resolve_manager_from_path(&path, manager_override.as_deref())
}

fn resolve_manager_from_path(
    path: &std::ffi::OsStr,
    manager_override: Option<&str>,
) -> Result<(ManagerKind, Box<dyn PackageManagerBackend>), BackendError> {
    if let Some(choice) = manager_override {
        return match choice {
            "pnpm" => Ok((ManagerKind::Pnpm, Box::new(PnpmBackend::new()))),
            "npm" => Ok((ManagerKind::Npm, Box::new(NpmBackend::new()))),
            other => Err(BackendError::spawn_failed(format!(
                "CLAY_PACKAGE_MANAGER={other} is unknown; expected `pnpm` or `npm`"
            ))),
        };
    }
    if path_contains_executable(path, "pnpm") {
        Ok((ManagerKind::Pnpm, Box::new(PnpmBackend::new())))
    } else if path_contains_executable(path, "npm") {
        Ok((ManagerKind::Npm, Box::new(NpmBackend::new())))
    } else {
        Err(BackendError::spawn_failed(
            "no npm-compatible package manager found on PATH (pnpm or npm); install one and retry",
        ))
    }
}

fn path_contains_executable(path: &std::ffi::OsStr, binary: &str) -> bool {
    std::env::split_paths(path).any(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file() && is_executable(&candidate)
    })
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &std::path::Path) -> bool {
    true
}

// ── npm implementation ───────────────────────────────────────────────────────

/// Package manager backend that delegates to npm.
///
/// npm ships with Node, which clay-agent already requires, making it the
/// fallback when pnpm is absent. Uses `npm install --prefix <store>` so the
/// store stays the install target regardless of the caller's working
/// directory, `npm remove --prefix <store>`, and `npm list --prefix <store>
/// --json` discovery. All process invocations capture stdout/stderr;
/// diagnostics embedded in errors pass through the shared sanitizer.
///
/// Security: same posture as [`PnpmBackend`] — lifecycle scripts suppressed
/// by default, package code never read or executed by the backend.
pub struct NpmBackend {
    /// Name or path to the npm binary (defaults to `"npm"`).
    pub npm_bin: String,
}

impl NpmBackend {
    pub fn new() -> Self {
        Self {
            npm_bin: "npm".to_string(),
        }
    }

    /// Build the `npm install` argument list for the given spec and options.
    /// Exposed for tests so the command shape can be verified without
    /// requiring npm to be installed.
    pub fn install_command_args(
        &self,
        package_spec: &str,
        store_root: &std::path::Path,
        options: PackageInstallOptions,
    ) -> Vec<String> {
        let mut args = vec![
            "install".to_string(),
            "--prefix".to_string(),
            store_root.display().to_string(),
            package_spec.to_string(),
        ];
        if !options.allow_lifecycle_scripts {
            // Suppress lifecycle scripts by default. Remote package code must
            // not execute before Clay validates package metadata.
            args.push("--ignore-scripts".to_string());
        }
        args
    }

    fn run(&self, args: &[&str], cwd: &std::path::Path) -> Result<Output, BackendError> {
        Command::new(&self.npm_bin)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|err| {
                BackendError::spawn_failed(format!("failed to spawn `{}`: {err}", self.npm_bin))
            })
    }
}

impl Default for NpmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerBackend for NpmBackend {
    fn install(
        &self,
        package_spec: &str,
        store: &PackageStore,
        options: PackageInstallOptions,
    ) -> Result<InstallResult, BackendError> {
        let command_args = self.install_command_args(package_spec, &store.root, options);
        let args = command_args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        let output = self.run(&args, &store.root)?;
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !success {
            let redacted =
                sanitize_package_manager_diagnostics(&String::from_utf8_lossy(&output.stderr));
            return Err(BackendError::process_failed(format!(
                "`npm install {package_spec}` failed (exit {:?}):\n{redacted}",
                output.status.code()
            )));
        }
        Ok(InstallResult {
            package_spec: package_spec.to_string(),
            success: true,
            stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
        })
    }

    fn remove(&self, package_name: &str, store: &PackageStore) -> Result<(), BackendError> {
        let output = self.run(
            &[
                "remove",
                "--prefix",
                &store.root.display().to_string(),
                package_name,
            ],
            &store.root,
        )?;
        if !output.status.success() {
            let redacted =
                sanitize_package_manager_diagnostics(&String::from_utf8_lossy(&output.stderr));
            return Err(BackendError::process_failed(format!(
                "`npm remove {package_name}` failed (exit {:?}):\n{redacted}",
                output.status.code()
            )));
        }
        Ok(())
    }

    fn list_installed(&self, store: &PackageStore) -> Result<Vec<DiscoveredPackage>, BackendError> {
        let output = self.run(
            &[
                "list",
                "--prefix",
                &store.root.display().to_string(),
                "--json",
            ],
            &store.root,
        )?;
        if !output.status.success() {
            let redacted =
                sanitize_package_manager_diagnostics(&String::from_utf8_lossy(&output.stderr));
            return Err(BackendError::process_failed(format!(
                "`npm list --json` failed (exit {:?}):\n{redacted}",
                output.status.code()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let value: Value = serde_json::from_str(&stdout).map_err(|err| {
            BackendError::parse_failed(format!("failed to parse `npm list --json` output: {err}"))
        })?;
        // A fresh (or package.json-less) store prints no `dependencies`
        // object at all: that is "nothing installed", not a discovery failure.
        let empty = serde_json::Map::new();
        let dependencies = value
            .get("dependencies")
            .and_then(Value::as_object)
            .unwrap_or(&empty);

        let mut packages = Vec::new();
        for (dep_name, dep) in dependencies {
            // npm installs flattened under node_modules/<name> (scoped:
            // node_modules/@scope/name), so the package root is deterministic.
            let package_root = store.root.join("node_modules").join(dep_name);
            let package_json_path = package_root.join("package.json");
            if let Ok(text) = std::fs::read_to_string(&package_json_path)
                && let Ok(value) = serde_json::from_str::<Value>(&text)
            {
                // Discovery reports the bare resolved name, never the `npm:`
                // install spec; the resolved registry URL is the last resort
                // (it would mislabel local-registry tarballs as "tarball"
                // source, so the package name wins over it).
                let requested_spec = dep
                    .get("from")
                    .or_else(|| dep.get("name"))
                    .and_then(Value::as_str)
                    .or_else(|| value.get("name").and_then(Value::as_str))
                    .or_else(|| dep.get("resolved").and_then(Value::as_str))
                    .unwrap_or(dep_name)
                    .to_string();
                let diagnostics = dep
                    .get("resolved")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let provenance = PackageProvenance::from_package_json(
                    &requested_spec,
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

    fn fake_bin(label: &str, binary: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clay-manager-resolver-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(binary);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&bin, body).unwrap();
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(not(unix))]
        {
            let _ = body;
            std::fs::write(&bin, "").unwrap();
        }
        dir
    }

    fn path_of(dirs: &[&Path]) -> OsString {
        std::env::join_paths(dirs.iter().copied()).unwrap()
    }

    #[test]
    fn npm_spec_parse_accepts_all_v1_forms() {
        for (raw, expected_name, expected_version) in [
            ("npm:markdown", "markdown", None),
            ("npm:@arnilo/st", "@arnilo/st", None),
            ("npm:markdown@1.2.3", "markdown", Some("1.2.3")),
            ("npm:@arnilo/st@1.2.3", "@arnilo/st", Some("1.2.3")),
            (
                "npm:@arnilo/st@0.1.0-beta.1",
                "@arnilo/st",
                Some("0.1.0-beta.1"),
            ),
        ] {
            let spec = PackageSpec::parse(raw).unwrap_or_else(|error| panic!("{raw}: {error}"));
            assert_eq!(spec.name, expected_name, "{raw}");
            assert_eq!(spec.version.as_deref(), expected_version, "{raw}");
            assert_eq!(spec.to_spec(), raw, "{raw} round-trips");
        }
    }

    #[test]
    fn npm_spec_parse_rejects_all_non_npm_sources() {
        for raw in [
            "markdown",
            "@arnilo/st",
            "github:user/st",
            "git+https://github.com/user/st.git",
            "https://example.test/st.tgz",
            "file:./st",
            "./local-st",
            "npm:",
        ] {
            let error = PackageSpec::parse(raw).unwrap_err();
            assert!(
                matches!(&error, PackageSpecError::MissingNpmPrefix { spec } if spec == raw),
                "{raw}: got {error:?}"
            );
            let message = error.to_string();
            assert!(
                message.contains("npm:"),
                "{raw}: message must name the v1 form"
            );
        }
    }

    #[test]
    fn npm_spec_parse_rejects_invalid_names() {
        for raw in [
            "npm:Name",
            "npm:name space",
            "npm:@scope/",
            "npm:@/name",
            "npm:@scope/name/extra",
            "npm:name/",
            "npm:.",
            "npm:..",
            "npm:@scope//name",
        ] {
            assert!(
                matches!(
                    &PackageSpec::parse(raw),
                    Err(PackageSpecError::InvalidName { .. })
                ),
                "{raw} must be an invalid name"
            );
        }
    }

    #[test]
    fn npm_spec_parse_rejects_ranges_and_non_exact_versions() {
        for (raw, version) in [
            ("npm:markdown@^1.2.3", "^1.2.3"),
            ("npm:markdown@~1.2.3", "~1.2.3"),
            ("npm:markdown@1.2", "1.2"),
            ("npm:markdown@1.x", "1.x"),
            ("npm:markdown@latest", "latest"),
            ("npm:markdown@", ""),
        ] {
            assert!(
                matches!(
                    &PackageSpec::parse(raw),
                    Err(PackageSpecError::InvalidVersion { version: got, .. }) if got == version
                ),
                "{raw} must be rejected as a non-exact version"
            );
        }
    }

    #[test]
    fn backends_suppress_lifecycle_scripts_by_default() {
        let store = Path::new("/tmp/clay-manager-test-store");
        let pnpm = PnpmBackend::new();
        assert_eq!(
            pnpm.install_command_args("npm:@arnilo/st", PackageInstallOptions::default()),
            vec!["add", "npm:@arnilo/st", "--ignore-scripts"]
        );
        assert_eq!(
            pnpm.install_command_args(
                "npm:@arnilo/st",
                PackageInstallOptions {
                    allow_lifecycle_scripts: true,
                }
            ),
            vec!["add", "npm:@arnilo/st"]
        );

        let npm = NpmBackend::new();
        let store_arg = store.display().to_string();
        assert_eq!(
            npm.install_command_args("npm:@arnilo/st", store, PackageInstallOptions::default()),
            vec![
                "install",
                "--prefix",
                store_arg.as_str(),
                "npm:@arnilo/st",
                "--ignore-scripts"
            ]
        );
        assert_eq!(
            npm.install_command_args(
                "npm:@arnilo/st",
                store,
                PackageInstallOptions {
                    allow_lifecycle_scripts: true,
                }
            ),
            vec!["install", "--prefix", store_arg.as_str(), "npm:@arnilo/st"]
        );
    }

    #[test]
    fn npm_backend_list_parses_npm_json_shape() {
        let store_root =
            std::env::temp_dir().join(format!("clay-npm-list-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store_root);
        let scoped_root = store_root.join("node_modules/@arnilo/st");
        std::fs::create_dir_all(&scoped_root).unwrap();
        std::fs::write(
            scoped_root.join("package.json"),
            r#"{"name":"@arnilo/st","version":"0.1.0","clay":{"apiPrefix":"st"}}"#,
        )
        .unwrap();

        let dir = fake_bin(
            "fake-npm-list",
            "npm",
            "#!/bin/sh\necho '{\"name\":\"store\",\"dependencies\":{\"@arnilo/st\":{\"version\":\"0.1.0\",\"from\":\"@arnilo/st\",\"resolved\":\"https://registry.example/@arnilo/st/-/st-0.1.0.tgz\"}}}'\nexit 0\n",
        );
        let backend = NpmBackend {
            npm_bin: dir.join("npm").display().to_string(),
        };
        let discovered = backend
            .list_installed(&PackageStore::new(&store_root))
            .expect("npm list discovery works against the npm JSON shape");
        assert_eq!(discovered.len(), 1);
        let pkg = &discovered[0];
        assert_eq!(pkg.provenance.resolved_name, "@arnilo/st");
        assert_eq!(pkg.provenance.resolved_version, "0.1.0");
        assert_eq!(pkg.provenance.source_kind, PackageSourceKind::NpmRegistry);
        assert_eq!(pkg.provenance.requested_spec, "@arnilo/st");
        assert!(
            pkg.package_root.ends_with("node_modules/@arnilo/st"),
            "npm packages resolve to their node_modules root"
        );
    }

    #[test]
    fn npm_backend_list_treats_a_dependencies_less_reply_as_an_empty_store() {
        // A fresh store (no package.json, or one without dependencies) makes
        // `npm list --json` print an object with no `dependencies` key at all.
        // That is "nothing installed" — reporting it as a discovery failure
        // made every first launch log a package-manager error.
        let store_root =
            std::env::temp_dir().join(format!("clay-npm-empty-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store_root);
        std::fs::create_dir_all(&store_root).unwrap();
        let dir = fake_bin("fake-npm-empty", "npm", "#!/bin/sh\necho '{}'\nexit 0\n");
        let backend = NpmBackend {
            npm_bin: dir.join("npm").display().to_string(),
        };
        let discovered = backend
            .list_installed(&PackageStore::new(&store_root))
            .expect("an empty store is not a discovery failure");
        assert!(discovered.is_empty(), "nothing installed: {discovered:?}");
        let _ = std::fs::remove_dir_all(&store_root);
    }

    #[test]
    fn discovery_creates_a_missing_store_root_before_spawning_the_manager() {
        // The manager runs *in* the store root (`current_dir(store.root)`), so
        // a fresh profile (no `~/.clay/packages` yet) must not hand the child
        // a nonexistent working directory: `Command::current_dir` on a missing
        // directory fails the spawn with ENOENT before npm ever starts.
        let dir = fake_bin(
            "fake-npm-missing-store",
            "npm",
            "#!/bin/sh\necho '{\"dependencies\":{}}'\nexit 0\n",
        );
        let store_root = dir.join("store-that-does-not-exist");
        let mut service = crate::packages::service::PackageService::open(
            &store_root,
            Box::new(NpmBackend {
                npm_bin: dir.join("npm").display().to_string(),
            }),
        )
        .expect("open the service on a fresh store root");
        service
            .refresh_installed()
            .expect("discovery on a store root that does not exist yet");
        assert!(
            store_root.is_dir(),
            "discovery creates the store root it runs the manager in"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolver_prefers_pnpm_then_npm_then_fails_closed() {
        let pnpm_dir = fake_bin("pnpm-only", "pnpm", "#!/bin/sh\nexit 0\n");
        let (kind, _) =
            resolve_manager_from_path(&path_of(&[&pnpm_dir]), None).expect("pnpm resolves");
        assert_eq!(kind, ManagerKind::Pnpm);

        let npm_dir = fake_bin("npm-only", "npm", "#!/bin/sh\nexit 0\n");
        let (kind, _) =
            resolve_manager_from_path(&path_of(&[&npm_dir]), None).expect("npm resolves");
        assert_eq!(kind, ManagerKind::Npm);

        // pnpm wins regardless of directory order when both are present.
        let (kind, _) =
            resolve_manager_from_path(&path_of(&[&npm_dir, &pnpm_dir]), None).expect("pnpm wins");
        assert_eq!(kind, ManagerKind::Pnpm);

        let empty_dir = fake_bin("neither", "none", "#!/bin/sh\nexit 0\n");
        let error = match resolve_manager_from_path(&path_of(&[&empty_dir]), None) {
            Ok(_) => panic!("missing managers must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.kind, BackendErrorKind::ProcessSpawnFailed);
        assert!(error.message.contains("pnpm or npm"), "{}", error.message);

        // Explicit override wins over PATH; unknown values fail closed.
        let (kind, _) =
            resolve_manager_from_path(&path_of(&[&pnpm_dir]), Some("npm")).expect("override wins");
        assert_eq!(kind, ManagerKind::Npm);
        let error = match resolve_manager_from_path(&path_of(&[&pnpm_dir]), Some("yarn")) {
            Ok(_) => panic!("unknown override must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.message.contains("CLAY_PACKAGE_MANAGER"),
            "{}",
            error.message
        );
    }
}
