/// Package management service.
///
/// [`PackageService`] is Clay's single authoritative service for installing,
/// enabling, disabling, listing, and inspecting packages.  The CLI and any
/// future in-app package UI both route through the same service instance so
/// that enabled-package state is never duplicated.
///
/// Separation of concerns:
/// - **Install**: delegates download/resolution/lockfile/integrity/caching to
///   the [`PackageManagerBackend`].  Installation records a package without
///   executing its runtime entry point.
/// - **Enable/load**: calls the Clay-owned [`assemble_package_record`] validator
///   before any contribution is activated.  A package whose Clay metadata is
///   invalid cannot be enabled.
/// - **Disable**: removes the package from the enabled set and frees its
///   prefix, mode, command IDs, configuration keys, and SDUI regions.
/// - **List / inspect**: return typed metadata without executing package code.
///
/// **Hot-path rule**: none of these operations may be called from typing,
/// paint, layout, scroll, or text-event handlers.  Every call is an explicit
/// user or agent operation off the editing hot path.
use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::packages::conflict::{PackageConflictDiagnostic, check_enabled_packages};
use crate::packages::manager::{
    BackendError, BackendErrorKind, PackageManagerBackend, PackageStore,
};
use crate::packages::record::{PackageRecord, PackageRecordError, assemble_package_record};

// ── Installed package state ───────────────────────────────────────────────────

/// A package that has been installed (via the delegated manager) but not
/// necessarily enabled.  Installation does not execute the runtime entry.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    /// The raw `package.json`-shaped value, retained for re-enabling.
    pub package_json: Value,
    /// Resolved path to the package root on disk.
    pub package_root: PathBuf,
}

// ── Service error types ───────────────────────────────────────────────────────

/// Errors returned by [`PackageService`] operations.
#[derive(Debug, Clone)]
pub enum PackageServiceError {
    /// The underlying package-manager backend failed.
    BackendError(BackendError),
    /// Clay's enable/load validator rejected the package.
    InvalidClayMetadata(PackageRecordError),
    /// Enabled package contribution conflicts would result from the operation.
    ContributionConflict(PackageConflictDiagnostic),
    /// The package is not installed.
    NotInstalled { package_name: String },
    /// The package is already enabled.
    AlreadyEnabled { package_name: String },
    /// The package is not currently enabled.
    NotEnabled { package_name: String },
    /// The raw package.json could not be found after install.
    MissingPackageJson { package_spec: String },
}

impl std::error::Error for PackageServiceError {}

impl std::fmt::Display for PackageServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendError(err) => write!(f, "package manager error: {}", err.message),
            Self::InvalidClayMetadata(err) => {
                write!(f, "invalid Clay package metadata: {}", err.message)
            }
            Self::ContributionConflict(err) => {
                write!(f, "package contribution conflict: {}", err.message)
            }
            Self::NotInstalled { package_name } => {
                write!(f, "package `{package_name}` is not installed")
            }
            Self::AlreadyEnabled { package_name } => {
                write!(f, "package `{package_name}` is already enabled")
            }
            Self::NotEnabled { package_name } => {
                write!(f, "package `{package_name}` is not enabled")
            }
            Self::MissingPackageJson { package_spec } => {
                write!(
                    f,
                    "package.json not found after installing `{package_spec}`"
                )
            }
        }
    }
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Result of an `inspect` operation.
#[derive(Debug, Clone)]
pub struct PackageInspection {
    pub name: String,
    pub version: String,
    pub api_prefix: String,
    pub is_enabled: bool,
    pub modes: Vec<String>,
    pub permissions: Vec<String>,
    pub docs_path: Option<String>,
    pub command_count: usize,
    pub configuration_count: usize,
}

/// One shared package-management service.
///
/// The `backend` field is boxed so that tests can inject a [`crate::packages::manager::FakeBackend`]
/// and production code uses a [`crate::packages::manager::PnpmBackend`].
pub struct PackageService {
    store: PackageStore,
    backend: Box<dyn PackageManagerBackend>,
    /// Packages that have been installed (but may not be enabled).
    installed: HashMap<String, InstalledPackage>,
    /// Packages that have passed Clay enable/load validation.
    enabled: HashMap<String, PackageRecord>,
}

impl PackageService {
    /// Create a new service with the given store root and backend.
    pub fn new(store_root: impl Into<PathBuf>, backend: Box<dyn PackageManagerBackend>) -> Self {
        Self {
            store: PackageStore::new(store_root),
            backend,
            installed: HashMap::new(),
            enabled: HashMap::new(),
        }
    }

    /// Install a package by spec.
    ///
    /// Delegates the actual download/resolution/lockfile/integrity/caching to
    /// the backend.  Does not execute the package runtime or enable the
    /// package.  The installed `package.json` is cached for later `enable`.
    pub fn install(
        &mut self,
        package_spec: &str,
        options: crate::packages::manager::PackageInstallOptions,
    ) -> Result<(), PackageServiceError> {
        // Ensure the store directory exists before invoking the backend; pnpm
        // needs a valid current working directory.
        std::fs::create_dir_all(&self.store.root).map_err(|error| {
            PackageServiceError::BackendError(BackendError {
                kind: BackendErrorKind::IoError,
                message: format!(
                    "could not create package store {}: {error}",
                    self.store.root.display()
                ),
            })
        })?;

        // Delegate to the backend.
        let result = self
            .backend
            .install(package_spec, &self.store, options)
            .map_err(PackageServiceError::BackendError)?;

        // Discover the installed package.json from the updated store.
        let discovered = self
            .backend
            .list_installed(&self.store)
            .map_err(PackageServiceError::BackendError)?;

        // Find the newly installed package in the discovery list.
        // Match by the package spec prefix or name field.
        let base_name = package_spec.split('@').next().unwrap_or(package_spec);
        let found = discovered.into_iter().find(|d| {
            d.package_json
                .get("name")
                .and_then(Value::as_str)
                .map(|name| {
                    name == package_spec || name == base_name || package_spec.starts_with(name)
                })
                .unwrap_or(false)
        });

        if let Some(pkg) = found {
            let name = pkg
                .package_json
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(package_spec)
                .to_string();
            self.installed.insert(
                name,
                InstalledPackage {
                    package_json: pkg.package_json,
                    package_root: pkg.package_root,
                },
            );
        } else if result.success {
            // Backend reported success but we could not re-discover the package.
            // This can happen with some fake backends that don't populate list results
            // on install; store under the spec name so subsequent enable can try.
            return Err(PackageServiceError::MissingPackageJson {
                package_spec: package_spec.to_string(),
            });
        }

        Ok(())
    }

    /// Install a package from a raw `package.json`-shaped value (used by fake
    /// backends and future local-path installs where the manifest is already
    /// available).
    ///
    /// This does **not** spawn any process; it simply records the package as
    /// installed without enabling it.
    pub fn install_from_value(&mut self, package_json: Value) -> Result<(), PackageServiceError> {
        let name = package_json
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| PackageServiceError::MissingPackageJson {
                package_spec: "<unknown>".to_string(),
            })?
            .to_string();
        self.installed.insert(
            name,
            InstalledPackage {
                package_json,
                package_root: PathBuf::from("<in-memory>"),
            },
        );
        Ok(())
    }

    /// Enable a previously installed package.
    ///
    /// Runs the Clay-owned [`assemble_package_record`] validator before activating
    /// any contribution.  Returns an error with actionable diagnostics if the
    /// Clay metadata is invalid.  Does not execute the package runtime.
    pub fn enable(&mut self, package_name: &str) -> Result<&PackageRecord, PackageServiceError> {
        if self.enabled.contains_key(package_name) {
            return Err(PackageServiceError::AlreadyEnabled {
                package_name: package_name.to_string(),
            });
        }
        let installed = self
            .installed
            .get(package_name)
            .ok_or_else(|| PackageServiceError::NotInstalled {
                package_name: package_name.to_string(),
            })?
            .clone();

        // Clay-owned validation: must pass before any contribution is activated.
        let record = assemble_package_record(&installed.package_json)
            .map_err(PackageServiceError::InvalidClayMetadata)?;

        self.enabled.insert(package_name.to_string(), record);
        if let Err(err) = check_enabled_packages(self.enabled.values()) {
            self.enabled.remove(package_name);
            return Err(PackageServiceError::ContributionConflict(err));
        }
        Ok(self.enabled.get(package_name).unwrap())
    }

    /// Disable a currently enabled package.
    ///
    /// Removes the package from the enabled set, freeing its prefix, modes,
    /// command IDs, configuration keys, and SDUI regions.
    pub fn disable(&mut self, package_name: &str) -> Result<PackageRecord, PackageServiceError> {
        self.enabled
            .remove(package_name)
            .ok_or_else(|| PackageServiceError::NotEnabled {
                package_name: package_name.to_string(),
            })
    }

    /// Remove (uninstall) a package.
    ///
    /// Disables the package first if it is currently enabled, then delegates
    /// the removal to the backend.
    pub fn remove(&mut self, package_name: &str) -> Result<(), PackageServiceError> {
        // Disable silently if enabled.
        let _ = self.disable(package_name);

        self.backend
            .remove(package_name, &self.store)
            .map_err(PackageServiceError::BackendError)?;
        self.installed.remove(package_name);
        Ok(())
    }

    /// List all installed packages with their enabled status.
    pub fn list(&self) -> Vec<PackageInspection> {
        let mut inspections: Vec<PackageInspection> = self
            .installed
            .iter()
            .map(|(name, installed)| {
                let is_enabled = self.enabled.contains_key(name.as_str());
                self.inspection_from_installed(name, installed, is_enabled)
            })
            .collect();
        inspections.sort_by(|a, b| a.name.cmp(&b.name));
        inspections
    }

    /// Inspect a specific package by name.
    pub fn inspect(&self, package_name: &str) -> Option<PackageInspection> {
        // If enabled, use the richer record.
        if let Some(record) = self.enabled.get(package_name) {
            return Some(self.inspection_from_record(record));
        }
        // Fall back to installed metadata.
        self.installed
            .get(package_name)
            .map(|installed| self.inspection_from_installed(package_name, installed, false))
    }

    /// Return all currently enabled package records.
    pub fn enabled_records(&self) -> impl Iterator<Item = &PackageRecord> {
        self.enabled.values()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn inspection_from_record(&self, record: &PackageRecord) -> PackageInspection {
        PackageInspection {
            name: record.manifest.name.clone(),
            version: record.manifest.version.clone(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
            is_enabled: true,
            modes: record.manifest.clay.modes.clone(),
            permissions: record
                .manifest
                .clay
                .permissions
                .iter()
                .map(|p| p.as_str().to_string())
                .collect(),
            docs_path: Some(record.docs.docs_path.clone()),
            command_count: record.contributions.commands.len(),
            configuration_count: record.contributions.configuration.len(),
        }
    }

    fn inspection_from_installed(
        &self,
        name: &str,
        installed: &InstalledPackage,
        is_enabled: bool,
    ) -> PackageInspection {
        let json = &installed.package_json;
        let version = json
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let api_prefix = json
            .pointer("/clay/apiPrefix")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        PackageInspection {
            name: name.to_string(),
            version,
            api_prefix,
            is_enabled,
            modes: Vec::new(),
            permissions: Vec::new(),
            docs_path: None,
            command_count: 0,
            configuration_count: 0,
        }
    }
}
