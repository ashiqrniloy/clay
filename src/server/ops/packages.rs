use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::packages::{
    manifest::{PackageDiagnostic, validate_manifest_value},
    permissions::{PermissionValidationError, is_prohibited_authority, parse_permission},
    record::{PackageRecordError, assemble_package_record},
};

use super::ClayOpState;

/// One recorded validated package module: its absolute on-disk path plus the
/// validated package root that confines transitive imports.
#[derive(Debug, Clone)]
pub(crate) struct PackageLoadEntry {
    pub(crate) absolute_path: PathBuf,
    pub(crate) package_root: PathBuf,
    pub(crate) package_name: Option<String>,
}

/// Validated package `loadEntry` allowlist shared between the resolver op
/// (populates it) and `ClayModuleLoader` (checks it). It records load entries
/// for bundled and user-installed packages after `PackageService` validates,
/// authorizes, and enables them. Transitive relative imports from a validated
/// `loadEntry` are confined to the validated package root and recorded here on
/// first resolution.
// ponytail: in-memory map, keyed by opaque specifier. Ceiling: grows with the
// count of modules in loaded packages. Upgrade path: eviction only matters once
// packages can be unloaded dynamically (Phase 19 hot-reload); not needed before
// then.
#[derive(Debug, Default)]
pub(crate) struct PackageLoadEntryAllowlist {
    entries: Mutex<HashMap<String, PackageLoadEntry>>,
}

impl PackageLoadEntryAllowlist {
    /// Record an opaque validated `loadEntry` specifier with its absolute on-disk
    /// path and validated package root. Called by the resolver op after
    /// `PackageService::enable` succeeds.
    pub(crate) fn record(
        &self,
        opaque_specifier: &str,
        absolute_path: PathBuf,
        package_root: PathBuf,
    ) {
        self.record_for_package(opaque_specifier, absolute_path, package_root, None);
    }

    /// Record an opaque validated package module specifier with package
    /// ownership, so disable/revoke can withdraw all owned module entries.
    pub(crate) fn record_for_package(
        &self,
        opaque_specifier: &str,
        absolute_path: PathBuf,
        package_root: PathBuf,
        package_name: Option<&str>,
    ) {
        self.entries
            .lock()
            .expect("package loadEntry allowlist mutex poisoned")
            .insert(
                opaque_specifier.to_string(),
                PackageLoadEntry {
                    absolute_path,
                    package_root,
                    package_name: package_name.map(str::to_string),
                },
            );
    }

    /// Withdraw all module entries owned by a package. Returns the number of
    /// removed entries for audit diagnostics.
    pub(crate) fn revoke_package(&self, package_name: &str) -> usize {
        let mut entries = self
            .entries
            .lock()
            .expect("package loadEntry allowlist mutex poisoned");
        let before = entries.len();
        entries.retain(|_, entry| entry.package_name.as_deref() != Some(package_name));
        before.saturating_sub(entries.len())
    }

    /// Return the validated on-disk module path for an opaque specifier, or
    /// `None` if the specifier was never recorded. This is the allowlist gate
    /// the module loader checks in `resolve`/`load`.
    pub(crate) fn absolute_path(&self, opaque_specifier: &str) -> Option<PathBuf> {
        self.entries
            .lock()
            .expect("package loadEntry allowlist mutex poisoned")
            .get(opaque_specifier)
            .map(|entry| entry.absolute_path.clone())
    }

    /// Resolve a relative import from a validated package module, confining the
    /// result to the referrer's validated package root. Records the new opaque
    /// specifier and returns it so `load` can read it. Returns `None` if the
    /// referrer is not a validated package module or the resolved path escapes
    /// the package root.
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
                .expect("package loadEntry allowlist mutex poisoned");
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
        // preserving the already-recorded package prefix. Package names may be
        // scoped (`@vendor/foo`), so do not split on `/` to find the package id.
        let relative_to_root = canonical.strip_prefix(&referrer_entry.package_root).ok()?;
        let relative_tail = relative_to_root.to_string_lossy().replace('\\', "/");
        let referrer_relative_to_root = referrer_entry
            .absolute_path
            .strip_prefix(&referrer_entry.package_root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        let package_prefix = referrer.strip_suffix(&referrer_relative_to_root)?;
        let new_specifier = format!("{package_prefix}{relative_tail}");
        let mut entries = self
            .entries
            .lock()
            .expect("package loadEntry allowlist mutex poisoned");
        entries.insert(
            new_specifier.clone(),
            PackageLoadEntry {
                absolute_path: canonical,
                package_root: referrer_entry.package_root.clone(),
                package_name: referrer_entry.package_name.clone(),
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
pub(super) fn op_clay_packages_list_first_party_specifiers(
    _state: &mut OpState,
) -> Result<String, JsErrorBox> {
    let packages_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
    let mut specifiers = Vec::new();
    let entries = std::fs::read_dir(packages_root).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.packages.list_failed: failed to read first-party package root ({error})"
        ))
    })?;
    for entry in entries.flatten() {
        let package_json = entry.path().join("package.json");
        let Ok(text) = std::fs::read_to_string(package_json) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(name) = value.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.starts_with("@clay/") {
            specifiers.push(name.to_string());
        }
    }
    specifiers.sort();
    serde_json::to_string(&json!({ "specifiers": specifiers }))
        .map_err(serialize_error("clay.packages.list_failed"))
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
        if is_prohibited_authority(permission) {
            return Err(JsErrorBox::generic(format!(
                "clay.packages.prohibited_authority: prohibited authority `{permission}` cannot be requested by default"
            )));
        }
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

/// Resolve an installed, authorized package specifier, validate + enable it
/// through the existing `PackageService` path, record the validated on-disk
/// `loadEntry` in the shared allowlist, and return a typed summary with an
/// opaque `loadEntrySpecifier` the module loader can later import.
///
/// Bundled `@clay/*` packages are seeded from Clay's shipped package directory.
/// User-installed npm/GitHub/git/tarball/local-path packages resolve from the
/// package service's installed package registry by package name or original
/// requested specifier. In both cases the same validation, authorization,
/// package-root confinement, and module-loader allowlist path is used.
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

    let clay_state = state.borrow::<Arc<ClayOpState>>();

    let summary = {
        let mut service = clay_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");

        let (resolved_name, package_root) = match service.installed_package_for_specifier(specifier)
        {
            Some((name, installed)) => (name, installed.package_root),
            None => {
                let Some(package_name) = specifier.strip_prefix("@clay/") else {
                    return Err(JsErrorBox::generic(format!(
                        "clay.packages.not_installed: package `{specifier}` is not installed or authorized"
                    )));
                };
                if !is_valid_first_party_package_segment(package_name) {
                    return Err(invalid_specifier(format!(
                        "invalid bundled package name in `{specifier}`"
                    )));
                }
                let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("packages")
                    .join(package_name);
                let package_json_text =
                    std::fs::read_to_string(package_root.join("package.json")).map_err(|_| {
                        JsErrorBox::generic(format!(
                            "clay.packages.not_installed: bundled package `{specifier}` is not installed"
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
                service
                    .install_from_value_at_root(package_json.clone(), package_root.clone())
                    .map_err(|error| {
                        JsErrorBox::generic(format!("clay.packages.load_failed: {error}"))
                    })?;
                if let Ok(manifest) =
                    crate::packages::manifest::validate_manifest_value(&package_json)
                {
                    let _ = service.authorize_package(
                        &resolved_name,
                        manifest.clay.permissions,
                        crate::packages::authorization::RuntimeProfile::NativeTrust,
                        "clay-bundled-default",
                    );
                }
                (resolved_name, package_root)
            }
        };

        // Reuse the authoritative `PackageService` validation/enable path
        // (`assemble_package_record` + authorization + `check_enabled_packages`).
        // This is the same enable path the CLI/future UI uses; the resolver adds
        // no separate package loader for user-installed packages.
        let record = match service.enable(&resolved_name) {
            Ok(record) => record.clone(),
            Err(crate::packages::service::PackageServiceError::AlreadyEnabled { .. }) => service
                .enabled_records()
                .find(|enabled| enabled.manifest.name == resolved_name)
                .expect("enabled package must be present after AlreadyEnabled")
                .clone(),
            Err(error) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.load_failed: {error}"
                )));
            }
        };

        // Compute the validated on-disk `loadEntry` path and record the opaque
        // specifier in the shared allowlist the module loader checks. This is
        // load/reload-time work only; the module hot path remains allowlist
        // lookup plus file read.
        let load_entry = record.manifest.clay.load_entry.as_deref().ok_or_else(|| {
            JsErrorBox::generic(format!(
                "clay.packages.load_failed: package `{specifier}` declares no loadEntry"
            ))
        })?;
        let normalized_load_entry = load_entry
            .strip_prefix("./")
            .unwrap_or(load_entry)
            .replace('\\', "/");
        let opaque_specifier = format!("clay://packages/{resolved_name}/{normalized_load_entry}");
        let (absolute_load_entry, canonical_package_root) =
            canonical_load_entry_paths(&package_root, &normalized_load_entry, specifier)?;
        clay_state.load_entry_allowlist().record_for_package(
            &opaque_specifier,
            absolute_load_entry,
            canonical_package_root,
            Some(&record.manifest.name),
        );

        json!({
            "name": record.manifest.name,
            "version": record.manifest.version,
            "apiPrefix": record.manifest.clay.api_prefix,
            "loadEntrySpecifier": opaque_specifier,
            "sourceKind": crate::packages::manager::PackageSourceKind::from_spec(specifier).as_str(),
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
                "syntaxGrammars": record.contributions.syntax_grammars.len(),
            }
        })
    };

    serde_json::to_string(&summary).map_err(serialize_error("clay.packages.load_failed"))
}

fn is_valid_first_party_package_segment(package_name: &str) -> bool {
    !package_name.is_empty()
        && package_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn canonical_load_entry_paths(
    package_root: &Path,
    normalized_load_entry: &str,
    specifier: &str,
) -> Result<(PathBuf, PathBuf), JsErrorBox> {
    let canonical_package_root = std::fs::canonicalize(package_root).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.packages.load_failed: package `{specifier}` root could not be canonicalized ({error})"
        ))
    })?;
    let absolute_load_entry =
        std::fs::canonicalize(package_root.join(normalized_load_entry)).map_err(|error| {
            JsErrorBox::generic(format!(
                "clay.packages.load_failed: package `{specifier}` loadEntry could not be canonicalized ({error})"
            ))
        })?;
    if !absolute_load_entry.starts_with(&canonical_package_root) {
        return Err(JsErrorBox::generic(format!(
            "clay.packages.load_failed: package `{specifier}` loadEntry escapes its package root"
        )));
    }
    Ok((absolute_load_entry, canonical_package_root))
}

#[cfg(test)]
mod tests {
    use super::{canonical_load_entry_paths, is_valid_first_party_package_segment};
    use std::{fs, path::PathBuf};

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-package-root-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn first_party_package_segment_rejects_paths_urls_and_registry_syntax() {
        for denied in [
            "",
            "../escape",
            "foo/bar",
            "foo\\bar",
            "markdown?tag=latest",
            "markdown#hash",
            "npm:markdown",
            "https://example.test/pkg",
            ".",
        ] {
            assert!(
                !is_valid_first_party_package_segment(denied),
                "first-party package segment `{denied}` must be rejected"
            );
        }
        assert!(is_valid_first_party_package_segment("markdown"));
        assert!(is_valid_first_party_package_segment("markdown-tools2"));
    }

    #[test]
    fn canonical_load_entry_paths_accepts_file_inside_package_root() {
        let root = temp_root("inside");
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(
            root.join("dist/load.js"),
            "export default function load() {}",
        )
        .unwrap();

        let (load_entry, package_root) =
            canonical_load_entry_paths(&root, "dist/load.js", "@clay/test").unwrap();

        assert!(load_entry.starts_with(package_root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_load_entry_paths_rejects_file_outside_package_root() {
        let root = temp_root("outside");
        fs::create_dir_all(&root).unwrap();
        let outside = root.with_extension("js");
        fs::write(&outside, "export default function load() {}").unwrap();
        let relative_escape = format!("../{}", outside.file_name().unwrap().to_string_lossy());

        let error = canonical_load_entry_paths(&root, &relative_escape, "@clay/test").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("loadEntry escapes its package root")
        );
        let _ = fs::remove_file(outside);
        let _ = fs::remove_dir_all(root);
    }
}
