use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::packages::{
    manifest::{PackageDiagnostic, validate_manifest_value},
    permissions::{PermissionValidationError, parse_permission},
    record::{PackageRecordError, assemble_package_record},
};

use super::ClayOpState;

/// One recorded validated package module: its absolute on-disk path plus the
/// validated package root that confines transitive imports.
#[derive(Debug, Clone)]
pub(crate) struct FirstPartyLoadEntry {
    pub(crate) absolute_path: PathBuf,
    pub(crate) package_root: PathBuf,
}

/// Validated first-party `loadEntry` allowlist shared between the resolver op
/// (populates it) and `ClayModuleLoader` (checks it). This is the single gate
/// that lets a resolver-validated first-party `loadEntry` load from outside the
/// configuration root; every other specifier stays deny-by-default. Transitive
/// relative imports from a validated `loadEntry` are confined to the validated
/// package root and recorded here on first resolution.
// ponytail: in-memory map, keyed by opaque specifier. Ceiling: grows with the
// count of modules in loaded first-party packages (bounded by the on-disk
// first-party package set). Upgrade path: eviction only matters once packages
// can be unloaded dynamically (Phase 19 hot-reload); not needed before then.
#[derive(Debug, Default)]
pub(crate) struct FirstPartyLoadEntryAllowlist {
    entries: Mutex<HashMap<String, FirstPartyLoadEntry>>,
}

impl FirstPartyLoadEntryAllowlist {
    /// Record an opaque validated `loadEntry` specifier with its absolute on-disk
    /// path and validated package root. Called by the resolver op after
    /// `PackageService::enable` succeeds.
    pub(crate) fn record(
        &self,
        opaque_specifier: &str,
        absolute_path: PathBuf,
        package_root: PathBuf,
    ) {
        self.entries
            .lock()
            .expect("first-party loadEntry allowlist mutex poisoned")
            .insert(
                opaque_specifier.to_string(),
                FirstPartyLoadEntry {
                    absolute_path,
                    package_root,
                },
            );
    }

    /// Return the validated on-disk module path for an opaque specifier, or
    /// `None` if the specifier was never recorded. This is the allowlist gate
    /// the module loader checks in `resolve`/`load`.
    pub(crate) fn absolute_path(&self, opaque_specifier: &str) -> Option<PathBuf> {
        self.entries
            .lock()
            .expect("first-party loadEntry allowlist mutex poisoned")
            .get(opaque_specifier)
            .map(|entry| entry.absolute_path.clone())
    }

    /// Resolve a relative import from a validated package module, confining the
    /// result to the referrer's validated package root. Records the new opaque
    /// specifier and returns it so `load` can read it. Returns `None` if the
    /// referrer is not a validated package module or the resolved path escapes
    /// the package root (deny-by-default for the transitive graph too).
    ///
    /// `referrer` is the opaque `clay://packages/...` specifier already in the
    /// allowlist; `relative_specifier` is the `./`/`../` import requested from it.
    pub(crate) fn resolve_relative(
        &self,
        referrer: &str,
        relative_specifier: &str,
    ) -> Option<String> {
        // Clone the referrer entry out of the map so the immutable borrow ends
        // before the mutable insert below; all path work happens outside the lock.
        let referrer_entry = {
            let entries = self
                .entries
                .lock()
                .expect("first-party loadEntry allowlist mutex poisoned");
            entries.get(referrer).cloned()?
        };
        // Join the relative specifier against the referrer's directory and
        // canonicalize so `..` segments collapse before the confinement check.
        let base = referrer_entry.absolute_path.parent()?;
        let candidate = base.join(relative_specifier);
        let canonical = std::fs::canonicalize(&candidate).ok()?;
        if !canonical.starts_with(&referrer_entry.package_root) {
            return None;
        }
        // Derive the opaque specifier from the package root + the relative tail,
        // keeping the `clay://packages/@clay/<name>/...` shape.
        let relative_to_root = canonical.strip_prefix(&referrer_entry.package_root).ok()?;
        let relative_tail = relative_to_root.to_string_lossy().replace('\\', "/");
        let package_segment = referrer
            .strip_prefix("clay://packages/")?
            .split('/')
            .next()?;
        let new_specifier = format!("clay://packages/{package_segment}/{relative_tail}");
        let mut entries = self
            .entries
            .lock()
            .expect("first-party loadEntry allowlist mutex poisoned");
        entries.insert(
            new_specifier.clone(),
            FirstPartyLoadEntry {
                absolute_path: canonical,
                package_root: referrer_entry.package_root.clone(),
            },
        );
        Some(new_specifier)
    }
}

#[op2]
#[string]
pub(super) fn op_clay_packages_validate_manifest(
    _state: &mut OpState,
    #[string] manifest_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&manifest_json, "clay.packages.invalid_manifest")?;
    let manifest =
        validate_manifest_value(&value).map_err(package_error("clay.packages.invalid_manifest"))?;
    serde_json::to_string(&json!({
        "name": manifest.name,
        "version": manifest.version,
        "apiPrefix": manifest.clay.api_prefix,
        "permissions": manifest.clay.permissions.iter().map(|permission| permission.as_str()).collect::<Vec<_>>(),
        "modes": manifest.clay.modes,
        "entry": manifest.clay.entry,
        "loadEntry": manifest.clay.load_entry,
    }))
    .map_err(serialize_error("clay.packages.validation_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_packages_load_package(
    _state: &mut OpState,
    #[string] package_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&package_json, "clay.packages.invalid_package")?;
    let record =
        assemble_package_record(&value).map_err(record_error("clay.packages.load_failed"))?;
    serde_json::to_string(&json!({
        "name": record.manifest.name,
        "version": record.manifest.version,
        "apiPrefix": record.manifest.clay.api_prefix,
        "entry": record.manifest.clay.entry,
        "loadEntry": record.manifest.clay.load_entry,
        "docs": record.docs.docs_path,
        "estimatedManifestBytes": record.performance.estimated_manifest_bytes,
        "apiDependencies": record.api_dependencies.iter().map(|dependency| dependency.api_id.as_str()).collect::<Vec<_>>(),
        "contributions": {
            "commands": record.contributions.commands.len(),
            "configuration": record.contributions.configuration.len(),
            "keyRouting": record.contributions.key_routing.len(),
            "textTransforms": record.contributions.text_transforms.len(),
            "sdui": record.contributions.sdui.len(),
            "decorations": record.contributions.decorations.len(),
            "uiPanels": record.contributions.ui_panels.len(),
            "uiComponents": record.contributions.ui_components.len(),
            "uiOverlays": record.contributions.ui_overlays.len(),
            "themeTokens": record.contributions.theme_tokens.len(),
            "input": record.contributions.input_contributions.len(),
            "uiStateScopes": record.contributions.ui_state_scopes.len(),
            "layoutOverrides": record.contributions.layout_overrides.len(),
            "packageOptions": record.contributions.package_options.len(),
        }
    }))
    .map_err(serialize_error("clay.packages.load_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_packages_validate_permissions(
    _state: &mut OpState,
    #[string] permissions_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&permissions_json, "clay.packages.invalid_permissions")?;
    let Some(values) = value.as_array() else {
        return Err(JsErrorBox::generic(
            "clay.packages.invalid_permissions: permissions must be an array of strings",
        ));
    };
    let mut permissions = Vec::new();
    for value in values {
        let Some(permission) = value.as_str() else {
            return Err(JsErrorBox::generic(
                "clay.packages.invalid_permissions: permissions must be an array of strings",
            ));
        };
        match parse_permission(permission) {
            Ok(permission) => permissions.push(permission.as_str()),
            Err(PermissionValidationError::UnknownPermission { .. }) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.unknown_permission: unknown Clay package permission `{permission}`"
                )));
            }
            Err(PermissionValidationError::ProhibitedAuthority { .. }) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.prohibited_authority: prohibited authority `{permission}` cannot be requested by default"
                )));
            }
        }
    }

    serde_json::to_string(&json!({ "permissions": permissions }))
        .map_err(serialize_error("clay.packages.validation_failed"))
}

fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| JsErrorBox::generic(format!("{code}: input must be valid JSON ({error})")))
}

fn package_error(code: &'static str) -> impl Fn(PackageDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn record_error(code: &'static str) -> impl Fn(PackageRecordError) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}

fn invalid_specifier(message: impl std::fmt::Display) -> JsErrorBox {
    JsErrorBox::generic(format!("clay.packages.invalid_specifier: {message}"))
}

/// Resolve a first-party `@clay/*` package specifier, validate + enable it
/// through the existing `PackageService` path, record the validated on-disk
/// `loadEntry` in the shared allowlist, and return a typed summary with an
/// opaque `loadEntrySpecifier` the module loader can later import.
///
/// Deny-by-default: only resolver-validated first-party `@clay/*` packages may
/// load. No filesystem, network, shell, registry, package-enable/disable, or
/// package-manager authority is granted beyond reading the single validated
/// `loadEntry` file (enforced by the loader, not by this op).
#[op2]
#[string]
pub(super) fn op_clay_packages_load_package_by_specifier(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request = parse_json(&request_json, "clay.packages.invalid_request")?;
    let Some(specifier) = request.get("specifier").and_then(Value::as_str) else {
        return Err(invalid_specifier(
            "loadPackage requires a `specifier` string",
        ));
    };
    if specifier.trim().is_empty() {
        return Err(invalid_specifier(
            "loadPackage requires a non-empty `specifier`",
        ));
    }

    // 1. Deny-by-default: reject any non-`@clay/*` specifier before touching the
    //    package service. External packages, registry specs (`npm:foo`), and
    //    bare names are all denied.
    let Some(package_name) = specifier.strip_prefix("@clay/") else {
        return Err(invalid_specifier(format!(
            "loadPackage only resolves first-party `@clay/*` packages; `{specifier}` is denied"
        )));
    };
    if package_name.is_empty()
        || package_name.contains('/')
        || package_name.contains('\\')
        || package_name.contains("..")
    {
        return Err(invalid_specifier(format!(
            "invalid first-party package name in `{specifier}`"
        )));
    }

    let clay_state = state.borrow::<Arc<ClayOpState>>();

    // 2. Resolve the installed first-party package from the on-disk `packages/`
    //    directory (the same `CARGO_MANIFEST_DIR/packages` root the markdown-it
    //    bundle is read from). This is the first-party package registry.
    let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join(package_name);
    let package_json_text =
        std::fs::read_to_string(package_root.join("package.json")).map_err(|_| {
            JsErrorBox::generic(format!(
                "clay.packages.not_installed: first-party package `{specifier}` is not installed"
            ))
        })?;
    let package_json: Value = serde_json::from_str(&package_json_text).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.packages.load_failed: invalid package.json for `{specifier}` ({error})"
        ))
    })?;
    let resolved_name = package_json
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(specifier)
        .to_string();

    // 3. Reuse the authoritative `PackageService` validation/enable path
    //    (`assemble_package_record` + `check_enabled_packages`). This is the
    //    same enable path the CLI/future UI uses; the resolver adds no second
    //    source of truth for enabled-package state.
    let summary = {
        let mut service = clay_state
            .first_party_packages()
            .lock()
            .expect("first-party package service mutex poisoned");
        // Seed the installed record (idempotent overwrite) then enable.
        let _ = service.install_from_value(package_json.clone());
        let record = match service.enable(&resolved_name) {
            Ok(record) => record,
            Err(crate::packages::service::PackageServiceError::AlreadyEnabled { .. }) => service
                .enabled_records()
                .find(|enabled| enabled.manifest.name == resolved_name)
                .expect("enabled package must be present after AlreadyEnabled"),
            Err(error) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.load_failed: {error}"
                )));
            }
        };

        // 4. Compute the validated on-disk `loadEntry` path and record the
        //    opaque specifier in the shared allowlist the module loader checks.
        let load_entry = record.manifest.clay.load_entry.as_deref().ok_or_else(|| {
            JsErrorBox::generic(format!(
                "clay.packages.load_failed: package `{specifier}` declares no loadEntry"
            ))
        })?;
        let normalized_load_entry = load_entry
            .strip_prefix("./")
            .unwrap_or(load_entry)
            .replace('\\', "/");
        let absolute_load_entry = package_root.join(&normalized_load_entry);
        let opaque_specifier = format!("clay://packages/{resolved_name}/{normalized_load_entry}");
        // Canonicalize both paths so the loader's confinement `starts_with`
        // check matches the canonicalized transitive-import paths (Windows
        // prefixes canonical paths with `\\?\`).
        let absolute_load_entry =
            std::fs::canonicalize(&absolute_load_entry).unwrap_or(absolute_load_entry);
        let canonical_package_root = std::fs::canonicalize(&package_root).unwrap_or(package_root);
        clay_state.load_entry_allowlist().record(
            &opaque_specifier,
            absolute_load_entry,
            canonical_package_root,
        );

        // 5. Build the typed summary.
        json!({
            "name": record.manifest.name,
            "version": record.manifest.version,
            "apiPrefix": record.manifest.clay.api_prefix,
            "loadEntrySpecifier": opaque_specifier,
            "modes": record.manifest.clay.modes,
            "permissions": record.manifest.clay.permissions.iter().map(|permission| permission.as_str()).collect::<Vec<_>>(),
            "contributions": {
                "commands": record.contributions.commands.len(),
                "configuration": record.contributions.configuration.len(),
                "keyRouting": record.contributions.key_routing.len(),
                "textTransforms": record.contributions.text_transforms.len(),
                "sdui": record.contributions.sdui.len(),
                "decorations": record.contributions.decorations.len(),
                "uiPanels": record.contributions.ui_panels.len(),
                "uiComponents": record.contributions.ui_components.len(),
                "uiOverlays": record.contributions.ui_overlays.len(),
                "themeTokens": record.contributions.theme_tokens.len(),
                "input": record.contributions.input_contributions.len(),
                "uiStateScopes": record.contributions.ui_state_scopes.len(),
                "layoutOverrides": record.contributions.layout_overrides.len(),
                "packageOptions": record.contributions.package_options.len(),
            }
        })
    };

    serde_json::to_string(&summary).map_err(serialize_error("clay.packages.load_failed"))
}
