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

use crate::packages::authorization::{
    LanguageServerGrant, PackageAuthorizationRecord, RuntimeProfile,
    resolve_language_server_executable,
};
use crate::packages::conflict::{
    PackageConflictDiagnostic, PackageConflictProvenance, PackageConflictResolutionDiagnostic,
    PackageConflictResolutionPolicy, PackageConflictResolutionReason,
    check_enabled_packages_with_policy,
};
use crate::packages::graph::{PackageGraphPlan, cycle_from_stack};
use crate::packages::manager::{
    BackendError, BackendErrorKind, PackageManagerBackend, PackageProvenance, PackageStore,
};
use crate::packages::permissions::PackagePermission;
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
    /// Source/provenance recorded during install or refresh discovery.
    pub provenance: PackageProvenance,
}

// ── Service error types ───────────────────────────────────────────────────────

/// Errors returned by [`PackageService`] operations.
#[derive(Debug, Clone)]
pub enum PackageServiceError {
    /// The underlying package-manager backend failed.
    BackendError(BackendError),
    /// Clay's enable/load validator rejected the package.
    ///
    /// Boxed to keep `Result<T, PackageServiceError>` under clippy's
    /// `result_large_err` threshold (the inner diagnostic is ~120 bytes).
    InvalidClayMetadata(Box<PackageRecordError>),
    /// Enabled package contribution conflicts would result from the operation.
    ///
    /// Boxed for the same reason as `InvalidClayMetadata`.
    ContributionConflict(Box<PackageConflictDiagnostic>),
    /// The package is not installed.
    NotInstalled { package_name: String },
    /// The package is already enabled.
    AlreadyEnabled { package_name: String },
    /// The package is not currently enabled.
    NotEnabled { package_name: String },
    /// The raw package.json could not be found after install.
    MissingPackageJson { package_spec: String },
    /// A package requested a capability that has not been user-approved.
    MissingCapabilityGrant {
        package_name: String,
        capability: PackagePermission,
    },
    /// A language-server capability lacks a current contribution/root-scoped grant.
    MissingLanguageServerGrant { package_name: String },
    /// Bundled-default trust was requested for a non-Clay-shipped package.
    BundledTrustDenied { package_name: String },
    /// A graph relation referenced a package that is not installed.
    MissingGraphTarget {
        package_name: String,
        target: String,
    },
    /// Enabling graph relations would recurse through a package cycle.
    PackageGraphCycle { cycle: Vec<String> },
    /// A package attempted disables/replaces without an explicit package-control grant.
    MissingPackageControlGrant { package_name: String },
    /// A structured relation request failed owner/user consent verification.
    RelationDenied {
        package_name: String,
        code: &'static str,
        detail: String,
    },
    /// A third-party package cannot execute without an exact current durable
    /// user approval (Plan 061 task 10 pre-execution adoption gate).
    AdoptionRequired {
        package_name: String,
        code: &'static str,
        detail: String,
    },
    /// The durable package approval store failed to load or persist.
    ApprovalStore { message: String },
    /// Rollback requested for a package with no active replacement.
    NoActiveReplacement { target: String },
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
            Self::MissingCapabilityGrant {
                package_name,
                capability,
            } => write!(
                f,
                "package `{package_name}` requested capability `{}` without a user authorization grant",
                capability.as_str()
            ),
            Self::MissingLanguageServerGrant { package_name } => write!(
                f,
                "package `{package_name}` requested `language-server` without a current contribution/root grant"
            ),
            Self::BundledTrustDenied { package_name } => write!(
                f,
                "package `{package_name}` is not Clay-shipped and cannot receive bundled defaults"
            ),
            Self::MissingGraphTarget {
                package_name,
                target,
            } => write!(
                f,
                "package `{package_name}` declares graph target `{target}` that is not installed"
            ),
            Self::PackageGraphCycle { cycle } => {
                write!(f, "package graph cycle detected: {}", cycle.join(" -> "))
            }
            Self::MissingPackageControlGrant { package_name } => write!(
                f,
                "package `{package_name}` declares disables/replaces without a package-control authorization grant"
            ),
            Self::RelationDenied {
                package_name,
                code,
                detail,
            } => write!(
                f,
                "{code}: package `{package_name}` relation request denied ({detail})"
            ),
            Self::AdoptionRequired {
                package_name,
                code,
                detail,
            } => write!(
                f,
                "{code}: package `{package_name}` requires explicit user adoption before execution ({detail}); inspect with `clay package inspect {package_name}` and approve with `clay package adopt {package_name}`"
            ),
            Self::ApprovalStore { message } => write!(f, "{message}"),
            Self::NoActiveReplacement { target } => {
                write!(f, "no enabled package currently replaces `{target}`")
            }
        }
    }
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Package-owned contribution counts withdrawn during disable/revocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageContributionWithdrawalCounts {
    pub commands: usize,
    pub behavior_manifests: usize,
    pub sdui: usize,
    pub parse_handlers: usize,
    pub decorations: usize,
    pub syntax_grammars: usize,
    pub completions: usize,
    pub layout: usize,
    pub input: usize,
    pub state: usize,
    pub theme: usize,
    pub diagnostics: usize,
}

impl PackageContributionWithdrawalCounts {
    fn from_record(record: &PackageRecord) -> Self {
        Self {
            commands: record.contributions.commands.len(),
            behavior_manifests: record.contributions.key_routing.len()
                + record.contributions.text_transforms.len(),
            sdui: record.contributions.sdui.len(),
            parse_handlers: usize::from(
                record
                    .manifest
                    .clay
                    .permissions
                    .contains(&PackagePermission::ParseDocument),
            ),
            decorations: record.contributions.decorations.len(),
            syntax_grammars: record.contributions.syntax_grammars.len(),
            completions: record.contributions.completion_providers.len(),
            layout: record.contributions.ui_panels.len()
                + record.contributions.ui_components.len()
                + record.contributions.ui_overlays.len()
                + record.contributions.layout_overrides.len()
                + record.contributions.package_options.len(),
            input: record.contributions.input_contributions.len(),
            state: record.contributions.ui_state_scopes.len(),
            theme: record.contributions.theme_tokens.len(),
            diagnostics: 0,
        }
    }
}

/// Audit record for a package-scoped disable/revocation generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRevocationRecord {
    pub package_name: String,
    pub api_prefix: String,
    pub package_generation: u64,
    pub withdrawn: PackageContributionWithdrawalCounts,
    pub reason: String,
}

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
    pub provenance: PackageProvenance,
    pub requested_capabilities: Vec<String>,
    pub approved_capabilities: Vec<String>,
    pub runtime_profile: Option<String>,
}

/// Adoption state of an installed package for inspection surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoptionState {
    /// No durable approval exists; the package cannot execute.
    Pending,
    /// An exact current durable approval covers the installed package.
    Approved,
    /// An approval exists but no longer matches the installed
    /// identity/capabilities/processes/relations (stale or expanded).
    Stale,
    /// The approval was explicitly revoked.
    Revoked,
}

/// All contribution IDs declared by a record (approval withdrawn lists).
fn contribution_ids_of(record: &PackageRecord) -> Vec<String> {
    let contributions = &record.contributions;
    contributions
        .commands
        .iter()
        .map(|d| d.id.clone())
        .chain(
            contributions
                .completion_providers
                .iter()
                .map(|d| d.id.clone()),
        )
        .chain(contributions.language_servers.iter().map(|d| d.id.clone()))
        .chain(
            contributions
                .language_intelligence_providers
                .iter()
                .map(|d| d.id.clone()),
        )
        .chain(contributions.sdui.iter().map(|d| d.region_id.clone()))
        .chain(contributions.ui_components.iter().map(|d| d.id.clone()))
        .chain(contributions.ui_panels.iter().map(|d| d.id.clone()))
        .chain(contributions.syntax_grammars.iter().map(|d| d.id.clone()))
        .collect()
}

/// Default on-disk package store root shared by the CLI and the production
/// server runtime: `~/.config/clay/packages`.
pub fn default_store_root() -> std::path::PathBuf {
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from);
    match base {
        Some(home) => home.join(".config").join("clay").join("packages"),
        None => std::path::PathBuf::from(".clay-packages"),
    }
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
    /// User/admin grants keyed by package name.
    authorizations: HashMap<String, PackageAuthorizationRecord>,
    /// Exact language-server grants keyed by package and contribution ID.
    language_server_grants: HashMap<(String, String), LanguageServerGrant>,
    /// Explicit user-selected winners for contribution conflicts.
    conflict_policy: PackageConflictResolutionPolicy,
    /// Last conflict/package-control resolutions applied during enable/load.
    conflict_resolutions: Vec<PackageConflictResolutionDiagnostic>,
    /// Monotonic package-state generation for enable/disable/revoke audit records.
    package_generation: u64,
    /// Package-scoped revocation records keyed by package name.
    revocations: HashMap<String, PackageRevocationRecord>,
    /// Durable host-owned user approvals (`clay-package-approval-v1`).
    approvals: crate::packages::approvals::PackageApprovalStore,
}

impl PackageService {
    /// Create a new service with the given store root and backend.
    ///
    /// The service starts with an empty installed set; call
    /// [`PackageService::refresh_installed`] after construction to repopulate
    /// `installed` from the package-manager store so CLI invocations reflect
    /// packages installed by previous processes.
    pub fn new(store_root: impl Into<PathBuf>, backend: Box<dyn PackageManagerBackend>) -> Self {
        Self {
            store: PackageStore::new(store_root),
            backend,
            installed: HashMap::new(),
            enabled: HashMap::new(),
            authorizations: HashMap::new(),
            language_server_grants: HashMap::new(),
            conflict_policy: PackageConflictResolutionPolicy::default(),
            conflict_resolutions: Vec::new(),
            package_generation: 0,
            revocations: HashMap::new(),
            approvals: crate::packages::approvals::PackageApprovalStore::in_memory(),
        }
    }

    /// Create a service backed by the durable approval store under
    /// `store_root` (`clay-package-approvals.json`). Loading fails closed on
    /// corruption, truncation, unknown store version, or unsafe permissions:
    /// a store Clay cannot trust behaves as a hard error, never as "no
    /// approvals". Production wiring uses this constructor.
    pub fn open(
        store_root: impl Into<PathBuf>,
        backend: Box<dyn PackageManagerBackend>,
    ) -> Result<Self, PackageServiceError> {
        let store_root = store_root.into();
        let approvals = crate::packages::approvals::PackageApprovalStore::open(&store_root)
            .map_err(|error| PackageServiceError::ApprovalStore {
                message: error.to_string(),
            })?;
        let mut service = Self::new(store_root, backend);
        service.approvals = approvals;
        Ok(service)
    }

    /// The approval store (read-only) for host-side coverage checks.
    // Used by cross-domain envelope validation (handlers wired in task 8).
    #[allow(dead_code)]
    pub(crate) fn approval_store(&self) -> &crate::packages::approvals::PackageApprovalStore {
        &self.approvals
    }

    /// Host-authored durable approval records (read-only view).
    pub fn package_approvals(
        &self,
    ) -> impl Iterator<Item = &crate::packages::approvals::PackageApprovalRecord> {
        self.approvals.records()
    }

    /// Record one host-authored durable approval and persist the store.
    /// Package code has no path to this; only host approval flows call it.
    pub fn record_package_approval(
        &mut self,
        record: crate::packages::approvals::PackageApprovalRecord,
    ) -> Result<(), PackageServiceError> {
        self.approvals
            .upsert(record)
            .map_err(|error| PackageServiceError::ApprovalStore {
                message: error.to_string(),
            })
    }

    /// Build and persist an exact durable approval for an installed package
    /// from host-side facts (identity, capabilities, processes, relations,
    /// replacements). One authority path shared by CLI and future native UI
    /// (Plan 061 task 10).
    pub fn approve_package(
        &mut self,
        package_name: &str,
        approved_by: &str,
    ) -> Result<crate::packages::approvals::PackageApprovalRecord, PackageServiceError> {
        let installed =
            self.installed
                .get(package_name)
                .ok_or_else(|| PackageServiceError::NotInstalled {
                    package_name: package_name.to_string(),
                })?;
        let record = assemble_package_record(&installed.package_json)
            .map_err(|err| PackageServiceError::InvalidClayMetadata(Box::new(err)))?;
        let capabilities = record
            .manifest
            .clay
            .permissions
            .iter()
            .map(|permission| permission.as_str().to_string())
            .collect();
        let processes = record
            .contributions
            .language_servers
            .iter()
            .map(|descriptor| descriptor.id.clone())
            .collect();
        let relations = record
            .manifest
            .clay
            .graph
            .relation_requests
            .iter()
            .map(|request| crate::packages::approvals::ApprovedRelation {
                package: request.package.clone(),
                extension_point: request.extension_point.clone(),
                version: request.version,
                operation: request.operation.as_str().to_string(),
                scopes: request.scopes.clone(),
            })
            .collect();
        let replacements = record
            .manifest
            .clay
            .graph
            .replaces
            .iter()
            .map(|target| {
                let withdrawn_contributions = self
                    .installed
                    .get(target)
                    .and_then(|target_installed| {
                        assemble_package_record(&target_installed.package_json).ok()
                    })
                    .map(|target_record| contribution_ids_of(&target_record))
                    .unwrap_or_default();
                crate::packages::approvals::ApprovedReplacement {
                    target: target.clone(),
                    replacement_package: record.manifest.name.clone(),
                    replacement_version: record.manifest.version.clone(),
                    replacement_source: installed.provenance.requested_spec.clone(),
                    replacement_integrity: installed.provenance.integrity.clone(),
                    withdrawn_contributions,
                    compatibility_claims: Vec::new(),
                    rollback_restore_target: true,
                }
            })
            .collect();
        let approval = crate::packages::approvals::PackageApprovalRecord {
            package: installed.provenance.resolved_name.clone(),
            resolved_version: installed.provenance.resolved_version.clone(),
            source: installed.provenance.requested_spec.clone(),
            integrity: installed.provenance.integrity.clone(),
            package_root: installed
                .provenance
                .package_root
                .to_string_lossy()
                .into_owned(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
            capabilities,
            processes,
            relations,
            replacements,
            approved_by: approved_by.to_string(),
            approved_at: crate::packages::approvals::rfc3339_now(),
            revoked: false,
        };
        self.record_package_approval(approval.clone())?;
        Ok(approval)
    }

    /// Current adoption state of an installed package (None if not installed).
    pub fn adoption_state(&self, package_name: &str) -> Option<AdoptionState> {
        let installed = self.installed.get(package_name)?;
        let record = assemble_package_record(&installed.package_json).ok()?;
        let processes: Vec<String> = record
            .contributions
            .language_servers
            .iter()
            .map(|descriptor| descriptor.id.clone())
            .collect();
        match self.approvals.approval_covers(
            &installed.provenance,
            &record.manifest.clay.api_prefix,
            &record.manifest.clay.permissions,
            &processes,
            &record.manifest.clay.graph,
        ) {
            Ok(()) => Some(AdoptionState::Approved),
            Err(crate::packages::approvals::ApprovalMismatch::NotFound) => {
                Some(AdoptionState::Pending)
            }
            Err(crate::packages::approvals::ApprovalMismatch::Revoked) => {
                Some(AdoptionState::Revoked)
            }
            Err(_) => Some(AdoptionState::Stale),
        }
    }

    /// Revoke a durable approval (kept for diagnostics) and persist.
    pub fn revoke_package_approval(
        &mut self,
        package_name: &str,
    ) -> Result<bool, PackageServiceError> {
        self.approvals
            .revoke(package_name)
            .map_err(|error| PackageServiceError::ApprovalStore {
                message: error.to_string(),
            })
    }

    /// Verify owner-plus-user consent for structured relation requests before
    /// the requester is enabled (and therefore before any of its code runs).
    /// Owner consent: the target's enabled record declares the exact
    /// versioned extension point and operation. User consent: third-party
    /// requesters additionally need an exact durable approval covering
    /// identity, capabilities, processes, and relation edges; trusted-domain
    /// packages are pre-authorized by the bundled inventory and skip the
    /// durable-approval requirement.
    fn verify_relation_authority(&self, record: &PackageRecord) -> Result<(), PackageServiceError> {
        let requests = &record.manifest.clay.graph.relation_requests;
        for request in requests {
            let target = self.enabled.get(&request.package).ok_or_else(|| {
                PackageServiceError::RelationDenied {
                    package_name: record.manifest.name.clone(),
                    code: "package_relation.target_not_enabled",
                    detail: format!("relation target `{}` is not enabled", request.package),
                }
            })?;
            crate::packages::extension_points::verify_relation_request(
                &target.manifest.clay.extension_points,
                request,
            )
            .map_err(|error| PackageServiceError::RelationDenied {
                package_name: record.manifest.name.clone(),
                code: error.code(),
                detail: format!("{:?}", error),
            })?;
        }
        // Pre-execution adoption gate (Plan 061 task 10): NO third-party
        // package executes without an exact current durable user approval
        // covering identity, capabilities, processes, and relations —
        // relation-bearing or not. Trusted bundled packages are exempt (their
        // authority is the compiled inventory, not user adoption).
        if record.runtime_domain == crate::packages::bundled::RuntimeDomain::ThirdParty {
            let installed = self.installed.get(&record.manifest.name).ok_or_else(|| {
                PackageServiceError::NotInstalled {
                    package_name: record.manifest.name.clone(),
                }
            })?;
            let processes: Vec<String> = record
                .contributions
                .language_servers
                .iter()
                .map(|descriptor| descriptor.id.clone())
                .collect();
            self.approvals
                .approval_covers(
                    &installed.provenance,
                    &record.manifest.clay.api_prefix,
                    &record.manifest.clay.permissions,
                    &processes,
                    &record.manifest.clay.graph,
                )
                .map_err(|mismatch| PackageServiceError::AdoptionRequired {
                    package_name: record.manifest.name.clone(),
                    code: mismatch.code(),
                    detail: format!("{:?}", mismatch),
                })?;
        }
        Ok(())
    }

    /// Repopulate the `installed` map from the package-manager store.
    ///
    /// Each CLI invocation is a fresh process with a fresh [`PackageService`],
    /// so without this call `installed` is empty even though packages were
    /// installed by a previous `clay package add`. Discovery delegates to the
    /// backend's `list_installed` (e.g. `pnpm list --json`) and does **not**
    /// execute package code; it only reads `package.json` metadata. Enabled
    /// state is intentionally kept in memory per process.
    ///
    /// The store is the single source of truth: this replaces the entire
    /// `installed` map with the discovered set.
    pub fn refresh_installed(&mut self) -> Result<(), PackageServiceError> {
        let discovered = self
            .backend
            .list_installed(&self.store)
            .map_err(PackageServiceError::BackendError)?;
        self.installed.clear();
        for pkg in discovered {
            let Some(name) = pkg.package_json.get("name").and_then(Value::as_str) else {
                // Skip packages without a name field rather than failing the
                // whole discovery; a real package always has a name.
                continue;
            };
            self.installed.insert(
                name.to_string(),
                InstalledPackage {
                    package_json: pkg.package_json,
                    package_root: pkg.package_root,
                    provenance: pkg.provenance,
                },
            );
        }
        Ok(())
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
            d.provenance.requested_spec == package_spec
                || d.package_json
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
            let diagnostics = format!("{}\n{}", result.stdout, result.stderr);
            let provenance = PackageProvenance::from_package_json(
                package_spec,
                &pkg.package_json,
                pkg.package_root.clone(),
                diagnostics,
            );
            self.installed.insert(
                name,
                InstalledPackage {
                    package_json: pkg.package_json,
                    package_root: pkg.package_root,
                    provenance,
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
        self.install_from_value_at_root(package_json, PathBuf::from("<in-memory>"))
    }

    /// Install a package from raw metadata with an explicit package root.
    ///
    /// Runtime loading uses this root for `loadEntry` canonicalization and
    /// transitive import confinement; it still does not execute package code.
    pub fn install_from_value_at_root(
        &mut self,
        package_json: Value,
        package_root: PathBuf,
    ) -> Result<(), PackageServiceError> {
        let requested_spec = package_json
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string();
        self.install_from_value_at_root_with_spec(package_json, package_root, &requested_spec)
    }

    /// Install a package from raw metadata with an explicit package root and
    /// original source specifier.
    pub fn install_from_value_at_root_with_spec(
        &mut self,
        package_json: Value,
        package_root: PathBuf,
        requested_spec: &str,
    ) -> Result<(), PackageServiceError> {
        let name = package_json
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| PackageServiceError::MissingPackageJson {
                package_spec: "<unknown>".to_string(),
            })?
            .to_string();
        let provenance = PackageProvenance::from_package_json(
            requested_spec,
            &package_json,
            package_root.clone(),
            "in-memory install",
        );
        self.installed.insert(
            name,
            InstalledPackage {
                package_json,
                package_root,
                provenance,
            },
        );
        Ok(())
    }

    /// Resolve an installed package by package name or original requested specifier.
    pub fn installed_package_for_specifier(
        &self,
        specifier: &str,
    ) -> Option<(String, InstalledPackage)> {
        self.installed
            .iter()
            .find(|(name, installed)| {
                name.as_str() == specifier || installed.provenance.requested_spec == specifier
            })
            .map(|(name, installed)| (name.clone(), installed.clone()))
    }

    /// Select an explicit user-configured conflict winner for one contribution ID.
    pub fn set_conflict_override(
        &mut self,
        contribution_id: impl Into<String>,
        winner_package: impl Into<String>,
    ) {
        self.conflict_policy
            .set_user_override(contribution_id, winner_package);
    }

    /// Return conflict/package-control resolutions applied by recent enable operations.
    pub fn conflict_resolution_diagnostics(&self) -> &[PackageConflictResolutionDiagnostic] {
        &self.conflict_resolutions
    }

    /// Return package-scoped revocation audit records produced by disable/revoke.
    pub fn revocation_records(&self) -> impl Iterator<Item = &PackageRevocationRecord> {
        self.revocations.values()
    }

    /// Return the latest revocation audit record for one package, if any.
    pub fn revocation_record(&self, package_name: &str) -> Option<&PackageRevocationRecord> {
        self.revocations.get(package_name)
    }

    /// Record user/admin authorization for an installed package.
    pub fn authorize_package(
        &mut self,
        package_name: &str,
        approved_capabilities: Vec<PackagePermission>,
        runtime_profile: RuntimeProfile,
        approved_by: impl Into<String>,
    ) -> Result<(), PackageServiceError> {
        let installed =
            self.installed
                .get(package_name)
                .ok_or_else(|| PackageServiceError::NotInstalled {
                    package_name: package_name.to_string(),
                })?;
        let record = assemble_package_record(&installed.package_json)
            .map_err(|err| PackageServiceError::InvalidClayMetadata(Box::new(err)))?;
        let authorization = PackageAuthorizationRecord::new(
            &installed.provenance,
            record.manifest.clay.api_prefix,
            approved_capabilities,
            runtime_profile,
            approved_by,
        );
        self.authorizations
            .insert(package_name.to_string(), authorization);
        Ok(())
    }

    /// Record one exact user-approved language-server contribution/root grant.
    pub fn authorize_language_server(
        &mut self,
        package_name: &str,
        contribution_id: &str,
        canonical_executable: PathBuf,
        workspace_root_ids: Vec<crate::protocol::WorkspaceRootId>,
        approved_by: impl Into<String>,
    ) -> Result<&LanguageServerGrant, PackageServiceError> {
        let approved_by = approved_by.into();
        let installed = self.installed.get(package_name).cloned().ok_or_else(|| {
            PackageServiceError::NotInstalled {
                package_name: package_name.to_string(),
            }
        })?;
        let record = assemble_package_record(&installed.package_json)
            .map_err(|err| PackageServiceError::InvalidClayMetadata(Box::new(err)))?;
        let descriptor = record
            .contributions
            .language_servers
            .iter()
            .find(|descriptor| descriptor.id == contribution_id)
            .ok_or_else(|| PackageServiceError::MissingLanguageServerGrant {
                package_name: package_name.to_string(),
            })?;
        if resolve_language_server_executable(&descriptor.executable).as_ref()
            != Some(&canonical_executable)
        {
            return Err(PackageServiceError::MissingLanguageServerGrant {
                package_name: package_name.to_string(),
            });
        }
        let grant = LanguageServerGrant::new(
            &installed.provenance,
            &record.manifest.clay.api_prefix,
            descriptor,
            canonical_executable,
            workspace_root_ids,
            &approved_by,
        );
        if grant.workspace_root_ids.is_empty() {
            return Err(PackageServiceError::MissingLanguageServerGrant {
                package_name: package_name.to_string(),
            });
        }
        self.language_server_grants.insert(
            (package_name.to_string(), contribution_id.to_string()),
            grant,
        );

        let mut capabilities = self
            .authorizations
            .get(package_name)
            .filter(|authorization| authorization_matches(&installed.provenance, authorization))
            .map(|authorization| authorization.approved_capabilities.clone())
            .unwrap_or_default();
        if !capabilities.contains(&PackagePermission::LanguageServer) {
            capabilities.push(PackagePermission::LanguageServer);
        }
        let runtime_profile = self
            .authorizations
            .get(package_name)
            .filter(|authorization| authorization_matches(&installed.provenance, authorization))
            .map_or(RuntimeProfile::Restricted, |authorization| {
                authorization.runtime_profile
            });
        self.authorizations.insert(
            package_name.to_string(),
            PackageAuthorizationRecord::new(
                &installed.provenance,
                record.manifest.clay.api_prefix,
                capabilities,
                runtime_profile,
                approved_by,
            ),
        );
        Ok(self
            .language_server_grants
            .get(&(package_name.to_string(), contribution_id.to_string()))
            .expect("inserted language-server grant must be present"))
    }

    /// Grant bundled defaults except process authority, preserving only an
    /// already-current exact language-server grant.
    pub fn authorize_bundled_defaults(
        &mut self,
        package_name: &str,
        approved_by: impl Into<String>,
    ) -> Result<(), PackageServiceError> {
        let installed = self.installed.get(package_name).cloned().ok_or_else(|| {
            PackageServiceError::NotInstalled {
                package_name: package_name.to_string(),
            }
        })?;
        // Bundled defaults require exact inventory verification: name, version,
        // canonical shipped root, and manifest integrity. Requested source kind
        // or `@clay/*` naming alone never qualifies.
        if crate::packages::bundled::verify_bundled_trust(&installed.provenance).is_err() {
            return Err(PackageServiceError::BundledTrustDenied {
                package_name: package_name.to_string(),
            });
        }
        let record = assemble_package_record(&installed.package_json)
            .map_err(|err| PackageServiceError::InvalidClayMetadata(Box::new(err)))?;
        let mut capabilities: Vec<_> = record
            .manifest
            .clay
            .permissions
            .iter()
            .copied()
            .filter(|permission| *permission != PackagePermission::LanguageServer)
            .collect();
        if self.has_current_language_server_grant(package_name, &installed.provenance, &record) {
            capabilities.push(PackagePermission::LanguageServer);
        }
        self.authorize_package(
            package_name,
            capabilities,
            RuntimeProfile::NativeTrust,
            approved_by,
        )
    }

    pub fn language_server_grant(
        &self,
        package_name: &str,
        contribution_id: &str,
    ) -> Option<&LanguageServerGrant> {
        self.language_server_grants
            .get(&(package_name.to_string(), contribution_id.to_string()))
    }

    pub fn revoke_language_server_grants(&mut self, package_name: &str) -> usize {
        let before = self.language_server_grants.len();
        self.language_server_grants
            .retain(|(name, _), _| name != package_name);
        if let Some(authorization) = self.authorizations.get_mut(package_name) {
            authorization
                .approved_capabilities
                .retain(|permission| *permission != PackagePermission::LanguageServer);
        }
        before.saturating_sub(self.language_server_grants.len())
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

        let previous_enabled = self.enabled.clone();
        let previous_resolutions = self.conflict_resolutions.clone();
        let previous_revocations = self.revocations.clone();
        let previous_package_generation = self.package_generation;
        let previous_approvals = self.approvals.snapshot();
        let resolution_offset = previous_resolutions.len();
        let result = self
            .enable_graph(package_name, &mut Vec::new())
            .and_then(|_| {
                // Stale-on-replacement (Plan 061 task 11): a committed
                // replacement revokes the replaced target's durable approval,
                // so restoring the target always requires an explicit user
                // rollback/re-adoption — never a silent old-approval reuse.
                // Trusted targets hold no approval record; `revoke` no-ops.
                let replaced: Vec<String> = self.conflict_resolutions[resolution_offset..]
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.reason == PackageConflictResolutionReason::PackageReplaces
                            && diagnostic.winner.package_name == package_name
                    })
                    .map(|diagnostic| diagnostic.loser.package_name.clone())
                    .collect();
                for target in replaced {
                    self.approvals.revoke(&target).map_err(|error| {
                        PackageServiceError::ApprovalStore {
                            message: error.to_string(),
                        }
                    })?;
                }
                Ok(())
            });
        if let Err(error) = result {
            self.enabled = previous_enabled;
            self.conflict_resolutions = previous_resolutions;
            self.revocations = previous_revocations;
            self.package_generation = previous_package_generation;
            if let Err(restore_error) = self.approvals.restore(previous_approvals) {
                return Err(PackageServiceError::ApprovalStore {
                    message: format!("approval rollback failed: {restore_error}"),
                });
            }
            return Err(error);
        }
        Ok(self
            .enabled
            .get(package_name)
            .expect("root package must be enabled after successful graph evaluation"))
    }

    /// Explicit user rollback of a committed replacement (Plan 061 task 11):
    /// disable the active replacement of `target`, re-adopt the target when
    /// it is third-party (the replacement commit revoked its approval), and
    /// re-enable it. Returns the disabled replacement's name. Restoration is
    /// always this explicit user action — never an automatic reversal.
    pub fn rollback_replacement(&mut self, target: &str) -> Result<String, PackageServiceError> {
        let replacement = self
            .enabled
            .values()
            .find(|record| {
                record.manifest.clay.graph.replaces.iter().any(|spec| {
                    self.installed_package_for_specifier(spec)
                        .is_some_and(|(name, _)| name == target)
                })
            })
            .map(|record| record.manifest.name.clone())
            .ok_or_else(|| PackageServiceError::NoActiveReplacement {
                target: target.to_string(),
            })?;
        self.disable(&replacement)?;
        let installed =
            self.installed
                .get(target)
                .ok_or_else(|| PackageServiceError::NotInstalled {
                    package_name: target.to_string(),
                })?;
        let third_party =
            crate::packages::bundled::verify_bundled_trust(&installed.provenance).is_err();
        if third_party && !matches!(self.adoption_state(target), Some(AdoptionState::Approved)) {
            self.approve_package(target, "rollback")?;
        }
        self.enable(target)?;
        Ok(replacement)
    }

    /// Disable a currently enabled package.
    ///
    /// Removes the package from the enabled set, freeing all package-owned
    /// contribution categories and recording a package-scoped revocation
    /// generation for runtime/parse/UI withdrawal hooks.
    /// Test-only: stamp the runtime domain of an enabled record so synthetic
    /// packages can exercise trusted-domain dispatch paths.
    #[cfg(test)]
    pub(crate) fn force_enabled_runtime_domain_for_test(
        &mut self,
        package_name: &str,
        package_version: &str,
        domain: crate::packages::bundled::RuntimeDomain,
    ) {
        if let Some(record) = self
            .enabled
            .get_mut(package_name)
            .filter(|record| record.manifest.version == package_version)
        {
            record.runtime_domain = domain;
        }
    }

    pub fn disable(&mut self, package_name: &str) -> Result<PackageRecord, PackageServiceError> {
        self.revoke_enabled_package(package_name, "disable")
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
        self.authorizations.remove(package_name);
        self.revoke_language_server_grants(package_name);
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

    /// Host-owned enabled record for an exact name/version pair. Used at
    /// package-facing op ingress so the executing package context resolves
    /// against the enabled set (a disabled or stale-version package fails
    /// closed).
    pub(crate) fn enabled_record(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Option<&PackageRecord> {
        self.enabled
            .get(package_name)
            .filter(|record| record.manifest.version == package_version)
    }

    /// Whether the current authorization record approves `permission` for
    /// `package_name` (never caller-declared permissions).
    pub(crate) fn has_approved_capability(
        &self,
        package_name: &str,
        permission: crate::packages::permissions::PackagePermission,
    ) -> bool {
        self.authorization_for(package_name)
            .is_some_and(|authorization| authorization.approved_capabilities.contains(&permission))
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn enable_graph(
        &mut self,
        package_name: &str,
        stack: &mut Vec<String>,
    ) -> Result<(), PackageServiceError> {
        if self.enabled.contains_key(package_name) {
            return Ok(());
        }
        if stack.iter().any(|name| name == package_name) {
            return Err(PackageServiceError::PackageGraphCycle {
                cycle: cycle_from_stack(stack, package_name),
            });
        }
        // A package replaced by an enabled replacement must not be silently
        // re-enabled to satisfy another package's dependency edge; restoring
        // it is the explicit user rollback path. Compatibility claims in the
        // approval record are disclosure-only today: dependency substitution
        // through a replacement stays fail-closed.
        if !stack.is_empty()
            && !self.enabled.contains_key(package_name)
            && self.conflict_resolutions.iter().any(|diagnostic| {
                diagnostic.reason == PackageConflictResolutionReason::PackageReplaces
                    && diagnostic.loser.package_name == package_name
                    && self.enabled.contains_key(&diagnostic.winner.package_name)
            })
        {
            return Err(PackageServiceError::RelationDenied {
                package_name: package_name.to_string(),
                code: "package_replacement.target_replaced",
                detail: format!(
                    "package `{package_name}` was replaced by user-approved replacement; dependents require explicit rollback of the replacement"
                ),
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
        let mut record = assemble_package_record(&installed.package_json)
            .map_err(|err| PackageServiceError::InvalidClayMetadata(Box::new(err)))?;
        record.runtime_domain = crate::packages::bundled::runtime_domain(&installed.provenance);
        self.ensure_capability_grants(package_name, &record)?;
        let graph = PackageGraphPlan::from_relations(&record.manifest.clay.graph);
        if graph.requires_package_control() {
            self.ensure_package_control_grant(package_name)?;
        }

        let resolved_graph = self.resolve_graph_targets(package_name, &graph)?;
        stack.push(package_name.to_string());
        for target in resolved_graph.activation_targets() {
            self.enable_graph(target, stack)?;
        }
        stack.pop();

        self.verify_relation_authority(&record)?;

        for target in &resolved_graph.disables {
            self.record_package_control_resolution(
                package_name,
                target,
                PackageConflictResolutionReason::PackageDisables,
            );
            self.enabled.remove(target);
        }
        for target in &resolved_graph.replaces {
            self.record_package_control_resolution(
                package_name,
                target,
                PackageConflictResolutionReason::PackageReplaces,
            );
            self.enabled.remove(target);
        }

        self.enabled.insert(package_name.to_string(), record);
        self.package_generation = self.package_generation.saturating_add(1);
        self.revocations.remove(package_name);
        self.reconcile_enabled_conflicts(package_name)?;
        Ok(())
    }

    fn revoke_enabled_package(
        &mut self,
        package_name: &str,
        reason: impl Into<String>,
    ) -> Result<PackageRecord, PackageServiceError> {
        let record =
            self.enabled
                .remove(package_name)
                .ok_or_else(|| PackageServiceError::NotEnabled {
                    package_name: package_name.to_string(),
                })?;
        self.conflict_resolutions.retain(|diagnostic| {
            diagnostic.winner.package_name != package_name
                && diagnostic.loser.package_name != package_name
        });
        self.record_revocation_for_record(&record, reason);
        Ok(record)
    }

    fn record_revocation_for_record(&mut self, record: &PackageRecord, reason: impl Into<String>) {
        self.package_generation = self.package_generation.saturating_add(1);
        let revocation = PackageRevocationRecord {
            package_name: record.manifest.name.clone(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
            package_generation: self.package_generation,
            withdrawn: PackageContributionWithdrawalCounts::from_record(record),
            reason: reason.into(),
        };
        self.revocations
            .insert(record.manifest.name.clone(), revocation);
    }

    fn reconcile_enabled_conflicts(
        &mut self,
        package_name: &str,
    ) -> Result<(), PackageServiceError> {
        match check_enabled_packages_with_policy(self.enabled.values(), &self.conflict_policy) {
            Ok(resolutions) => {
                for resolution in resolutions {
                    if let Some(loser) = self.enabled.remove(&resolution.loser.package_name) {
                        self.record_revocation_for_record(
                            &loser,
                            format!("conflict-resolution:{}", resolution.reason.as_str()),
                        );
                    }
                    self.conflict_resolutions.push(resolution);
                }
                Ok(())
            }
            Err(err) => {
                self.enabled.remove(package_name);
                Err(PackageServiceError::ContributionConflict(Box::new(err)))
            }
        }
    }

    fn record_package_control_resolution(
        &mut self,
        controller: &str,
        target: &str,
        reason: PackageConflictResolutionReason,
    ) {
        let (Some(winner), Some(loser)) =
            (self.installed.get(controller), self.enabled.get(target))
        else {
            return;
        };
        let Ok(winner_record) = assemble_package_record(&winner.package_json) else {
            return;
        };
        let winner_provenance = PackageConflictProvenance::from_record(&winner_record);
        let loser_provenance = PackageConflictProvenance::from_record(loser);
        let action = match reason {
            PackageConflictResolutionReason::PackageReplaces => "replaced",
            PackageConflictResolutionReason::PackageDisables => "disabled",
            PackageConflictResolutionReason::UserOverride => "overrode",
        };
        self.conflict_resolutions
            .push(PackageConflictResolutionDiagnostic {
                contribution_id: target.to_string().into_boxed_str(),
                winner: winner_provenance,
                loser: loser_provenance,
                reason,
                message: format!(
                    "package-control {action} `{target}` while enabling `{controller}`"
                )
                .into_boxed_str(),
            });
    }

    fn resolve_graph_targets(
        &self,
        package_name: &str,
        graph: &PackageGraphPlan,
    ) -> Result<PackageGraphPlan, PackageServiceError> {
        let mut resolved = PackageGraphPlan {
            depends_on: Vec::with_capacity(graph.depends_on.len()),
            extends: Vec::with_capacity(graph.extends.len()),
            disables: Vec::with_capacity(graph.disables.len()),
            replaces: Vec::with_capacity(graph.replaces.len()),
        };
        for target in &graph.depends_on {
            resolved
                .depends_on
                .push(self.resolve_graph_target(package_name, target)?);
        }
        for target in &graph.extends {
            resolved
                .extends
                .push(self.resolve_graph_target(package_name, target)?);
        }
        for target in &graph.disables {
            resolved
                .disables
                .push(self.resolve_graph_target(package_name, target)?);
        }
        for target in &graph.replaces {
            resolved
                .replaces
                .push(self.resolve_graph_target(package_name, target)?);
        }
        Ok(resolved)
    }

    fn resolve_graph_target(
        &self,
        package_name: &str,
        target: &str,
    ) -> Result<String, PackageServiceError> {
        if target == package_name {
            return Err(PackageServiceError::PackageGraphCycle {
                cycle: vec![package_name.to_string(), target.to_string()],
            });
        }
        self.installed_package_for_specifier(target)
            .map(|(name, _)| name)
            .ok_or_else(|| PackageServiceError::MissingGraphTarget {
                package_name: package_name.to_string(),
                target: target.to_string(),
            })
    }

    fn ensure_package_control_grant(&self, package_name: &str) -> Result<(), PackageServiceError> {
        if self
            .authorization_for(package_name)
            .is_some_and(|authorization| authorization.grants(PackagePermission::PackageControl))
        {
            return Ok(());
        }
        Err(PackageServiceError::MissingPackageControlGrant {
            package_name: package_name.to_string(),
        })
    }

    fn ensure_capability_grants(
        &self,
        package_name: &str,
        record: &PackageRecord,
    ) -> Result<(), PackageServiceError> {
        // Phase 24.5 decision (2026-08-13-2223 decision log): a missing
        // `language-server` grant no longer blocks loadPackage —
        // grantLanguageServer degrades independently per the examples
        // contract, and the capability stays inert because session start is
        // strictly grant-gated in authorize_language_server. All other
        // capabilities keep their hard load-time requirement.
        let required: Vec<&PackagePermission> = record
            .manifest
            .clay
            .permissions
            .iter()
            .filter(|capability| **capability != PackagePermission::LanguageServer)
            .collect();
        let Some(authorization) = self.authorizations.get(package_name) else {
            let Some(capability) = required.first().copied() else {
                return Ok(());
            };
            return Err(PackageServiceError::MissingCapabilityGrant {
                package_name: package_name.to_string(),
                capability: *capability,
            });
        };
        for capability in required {
            if !authorization.grants(*capability) {
                return Err(PackageServiceError::MissingCapabilityGrant {
                    package_name: package_name.to_string(),
                    capability: *capability,
                });
            }
        }
        Ok(())
    }

    fn has_current_language_server_grant(
        &self,
        package_name: &str,
        provenance: &PackageProvenance,
        record: &PackageRecord,
    ) -> bool {
        record
            .contributions
            .language_servers
            .iter()
            .any(|descriptor| {
                self.language_server_grants
                    .get(&(package_name.to_string(), descriptor.id.clone()))
                    .is_some_and(|grant| {
                        grant.matches(provenance, &record.manifest.clay.api_prefix, descriptor)
                            && resolve_language_server_executable(&descriptor.executable).as_ref()
                                == Some(&grant.canonical_executable)
                    })
            })
    }

    fn authorization_for(&self, package_name: &str) -> Option<&PackageAuthorizationRecord> {
        self.authorizations.get(package_name)
    }

    fn inspection_from_record(&self, record: &PackageRecord) -> PackageInspection {
        let provenance = self
            .installed
            .get(&record.manifest.name)
            .map(|installed| installed.provenance.clone())
            .unwrap_or_else(|| {
                let package_json = serde_json::json!({
                    "name": record.manifest.name.clone(),
                    "version": record.manifest.version.clone(),
                });
                PackageProvenance::from_package_json(
                    &record.manifest.name,
                    &package_json,
                    PathBuf::from("<enabled>"),
                    "enabled package record",
                )
            });
        let authorization = self.authorization_for(&record.manifest.name);
        let requested_capabilities: Vec<String> = record
            .manifest
            .clay
            .permissions
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        PackageInspection {
            name: record.manifest.name.clone(),
            version: record.manifest.version.clone(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
            is_enabled: true,
            modes: record.manifest.clay.modes.clone(),
            permissions: requested_capabilities.clone(),
            docs_path: Some(record.docs.docs_path.clone()),
            command_count: record.contributions.commands.len(),
            configuration_count: record.contributions.configuration.len(),
            provenance,
            requested_capabilities,
            approved_capabilities: authorization
                .map(PackageAuthorizationRecord::approved_capability_names)
                .unwrap_or_default(),
            runtime_profile: authorization
                .map(|record| record.runtime_profile.as_str().to_string()),
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
        let requested_capabilities = requested_capability_names(json);
        let authorization = self.authorization_for(name);
        PackageInspection {
            name: name.to_string(),
            version,
            api_prefix,
            is_enabled,
            modes: Vec::new(),
            permissions: requested_capabilities.clone(),
            docs_path: None,
            command_count: 0,
            configuration_count: 0,
            provenance: installed.provenance.clone(),
            requested_capabilities,
            approved_capabilities: authorization
                .map(PackageAuthorizationRecord::approved_capability_names)
                .unwrap_or_default(),
            runtime_profile: authorization
                .map(|record| record.runtime_profile.as_str().to_string()),
        }
    }
}

fn authorization_matches(
    provenance: &PackageProvenance,
    authorization: &PackageAuthorizationRecord,
) -> bool {
    authorization.package_name == provenance.resolved_name
        && authorization.requested_spec == provenance.requested_spec
        && authorization.source_kind == provenance.source_kind
        && authorization.resolved_version == provenance.resolved_version
}

fn requested_capability_names(package_json: &Value) -> Vec<String> {
    let mut names = Vec::new();
    for key in ["permissions", "capabilities"] {
        if let Some(values) = package_json
            .get("clay")
            .and_then(|clay| clay.get(key))
            .and_then(Value::as_array)
        {
            for value in values {
                if let Some(name) = value.as_str()
                    && !names.iter().any(|existing| existing == name)
                {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}
