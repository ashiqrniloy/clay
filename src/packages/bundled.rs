//! Immutable bundled first-party trust inventory and runtime-domain
//! classification (Plan 061, decision log 2026-07-21-0001).
//!
//! Trusted runtime placement resolves ONLY from the checked-in
//! [`BUNDLED_PACKAGES`] inventory bound to exact package name, version,
//! canonical root under `packages/`, and a fingerprint of the shipped
//! `package.json` bytes. `@clay/*` naming, requested source kinds, and normal
//! user authorization never promote code into the trusted domain.
//!
//! The fingerprint is FNV-1a-64, not a cryptographic hash: the trust root is
//! the checked-in source tree itself (an attacker who can write under
//! `CARGO_MANIFEST_DIR` can replace the whole binary), so the fingerprint's
//! job is exact binding plus drift detection. `build.rs` generates
//! [`BUNDLED_PACKAGES`] from `bundled-inventory.toml` plus each listed
//! `package.json`. Unlisted `packages/*` dirs are never trusted.

use std::path::{Path, PathBuf};

use crate::packages::manager::{PackageProvenance, PackageSourceKind};

/// Runtime trust domain for an enabled package. Host-owned; never exposed to
/// JavaScript or the public crate API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeDomain {
    /// Verified bundled first-party package; runs in the trusted runtime.
    Trusted,
    /// Everything else, including `@clay/*` packages from local/npm/git
    /// sources; runs in the shared third-party runtime.
    ThirdParty,
}

/// One checked-in bundled inventory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BundledPackageEntry {
    /// Exact manifest package name.
    pub(crate) name: &'static str,
    /// Exact manifest version.
    pub(crate) version: &'static str,
    /// Directory under `packages/` containing the shipped package.
    pub(crate) root: &'static str,
    /// FNV-1a-64 hex fingerprint of the shipped `package.json` bytes.
    pub(crate) manifest_fingerprint: &'static str,
}

/// One inventory helper export. Specifier is the exact import allowlist key
/// (`lsp-shared/client.js`); `file` is relative to the helper root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BundledHelperExport {
    pub(crate) specifier: &'static str,
    pub(crate) root: &'static str,
    pub(crate) file: &'static str,
}

// Generated bundled inventory. Helpers (`BUNDLED_HELPERS`) are fingerprinted
// and export-mapped but never loadPackage-able. Edit `bundled-inventory.toml`
// plus the package tree; fingerprints are computed at build time.
include!(concat!(env!("OUT_DIR"), "/bundled_packages.rs"));

/// FNV-1a 64-bit fingerprint as lowercase hex.
pub(crate) fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Look up a bundled inventory entry by exact package name.
pub(crate) fn bundled_entry(name: &str) -> Option<&'static BundledPackageEntry> {
    BUNDLED_PACKAGES.iter().find(|entry| entry.name == name)
}

/// Look up a helper inventory entry by directory name.
pub(crate) fn bundled_helper(root: &str) -> Option<&'static BundledPackageEntry> {
    BUNDLED_HELPERS.iter().find(|entry| entry.root == root)
}

/// Look up a helper export by exact specifier (`lsp-shared/client.js`).
pub(crate) fn helper_export(specifier: &str) -> Option<&'static BundledHelperExport> {
    BUNDLED_HELPER_EXPORTS
        .iter()
        .find(|export| export.specifier == specifier)
}

/// Canonical helper root and export file for an exact inventory specifier.
/// Fails closed if the file is missing or escapes the helper root.
pub(crate) fn resolve_helper_export(
    specifier: &str,
) -> Option<(&'static BundledHelperExport, PathBuf, PathBuf)> {
    let export = helper_export(specifier)?;
    let _helper = bundled_helper(export.root)?;
    let root = std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("packages")
            .join(export.root),
    )
    .ok()?;
    let file = std::fs::canonicalize(root.join(export.file)).ok()?;
    if !file.starts_with(&root) {
        return None;
    }
    Some((export, file, root))
}

/// Canonical shipped root for one inventory entry.
fn canonical_bundled_root(entry: &BundledPackageEntry) -> Option<PathBuf> {
    std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("packages")
            .join(entry.root),
    )
    .ok()
}

/// Why bundled trust verification failed. Details stay crate-internal so a
/// rejected caller learns only that bundled trust was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BundledTrustError {
    UnknownPackage,
    SourceKindMismatch,
    VersionMismatch,
    RootMismatch,
    ManifestUnreadable,
    IntegrityMismatch,
}

/// Verify bundled provenance against the compiled inventory. Exact match on
/// requested source kind, name, version, canonical root, and manifest
/// fingerprint is required; anything less fails closed. Runs at
/// install/enable/load time only — never on provider or editor hot paths.
pub(crate) fn verify_bundled_trust(
    provenance: &PackageProvenance,
) -> Result<&'static BundledPackageEntry, BundledTrustError> {
    let entry =
        bundled_entry(&provenance.resolved_name).ok_or(BundledTrustError::UnknownPackage)?;
    if provenance.source_kind != PackageSourceKind::ClayShipped {
        return Err(BundledTrustError::SourceKindMismatch);
    }
    if provenance.resolved_version != entry.version {
        return Err(BundledTrustError::VersionMismatch);
    }
    let expected_root = canonical_bundled_root(entry).ok_or(BundledTrustError::RootMismatch)?;
    let actual_root = std::fs::canonicalize(&provenance.package_root)
        .map_err(|_| BundledTrustError::RootMismatch)?;
    if actual_root != expected_root {
        return Err(BundledTrustError::RootMismatch);
    }
    let manifest_bytes = std::fs::read(actual_root.join("package.json"))
        .map_err(|_| BundledTrustError::ManifestUnreadable)?;
    if fnv1a64_hex(&manifest_bytes) != entry.manifest_fingerprint {
        return Err(BundledTrustError::IntegrityMismatch);
    }
    Ok(entry)
}

/// Classify the runtime domain for one package's provenance. Name alone never
/// selects [`RuntimeDomain::Trusted`].
pub(crate) fn runtime_domain(provenance: &PackageProvenance) -> RuntimeDomain {
    match verify_bundled_trust(provenance) {
        Ok(_) => RuntimeDomain::Trusted,
        Err(_) => RuntimeDomain::ThirdParty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_matches_source_tree() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        assert_eq!(
            BUNDLED_PACKAGES
                .iter()
                .map(|entry| entry.root)
                .collect::<Vec<_>>(),
            inventory_list_roots(),
            "BUNDLED_PACKAGES roots drifted from bundled-inventory.toml"
        );
        for entry in BUNDLED_PACKAGES {
            let bytes = std::fs::read(packages_dir.join(entry.root).join("package.json"))
                .unwrap_or_else(|error| panic!("{} manifest readable: {error}", entry.root));
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|error| panic!("{} manifest parses: {error}", entry.root));
            assert_eq!(value.get("name").and_then(|v| v.as_str()), Some(entry.name));
            assert_eq!(
                value.get("version").and_then(|v| v.as_str()),
                Some(entry.version)
            );
            assert!(
                value.get("clay").is_some(),
                "{} must declare clay metadata",
                entry.root
            );
            assert_eq!(
                fnv1a64_hex(&bytes),
                entry.manifest_fingerprint,
                "{} fingerprint drifted; rebuild after editing the listed package.json",
                entry.root
            );
        }
        for entry in BUNDLED_HELPERS {
            let bytes = std::fs::read(packages_dir.join(entry.root).join("package.json"))
                .unwrap_or_else(|error| panic!("{} helper manifest readable: {error}", entry.root));
            assert_eq!(
                fnv1a64_hex(&bytes),
                entry.manifest_fingerprint,
                "{} helper fingerprint drifted; rebuild after editing the helper package.json",
                entry.root
            );
            assert!(
                value_has_export_files(entry.root),
                "{} helper must keep exported files on disk",
                entry.root
            );
        }
    }

    fn value_has_export_files(root: &str) -> bool {
        BUNDLED_HELPER_EXPORTS
            .iter()
            .filter(|export| export.root == root)
            .all(|export| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("packages")
                    .join(export.root)
                    .join(export.file)
                    .is_file()
            })
    }

    #[test]
    fn unlisted_package_dirs_are_not_trusted() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        let listed = inventory_list_roots();
        assert!(
            packages_dir.join("lsp-shared/package.json").is_file(),
            "lsp-shared helper must remain on disk"
        );
        assert!(
            !listed.contains(&"lsp-shared"),
            "lsp-shared must stay off the loadable root list"
        );
        assert!(bundled_entry("@clay/lsp-shared").is_none());
        assert!(bundled_entry("lsp-shared").is_none());
        assert!(
            BUNDLED_HELPERS
                .iter()
                .any(|entry| entry.root == "lsp-shared"),
            "lsp-shared must be a fingerprinted helper"
        );
        for dir in std::fs::read_dir(&packages_dir).expect("packages dir readable") {
            let dir = dir.expect("package dir entry").path();
            if !dir.is_dir() {
                continue;
            }
            let root = dir.file_name().expect("dir name").to_string_lossy();
            if listed.contains(&root.as_ref()) {
                continue;
            }
            assert!(
                !BUNDLED_PACKAGES.iter().any(|entry| entry.root == root),
                "unlisted packages/{root} must not be in BUNDLED_PACKAGES"
            );
            let Ok(bytes) = std::fs::read(dir.join("package.json")) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
                assert!(
                    bundled_entry(name).is_none(),
                    "unlisted packages/{root} name `{name}` must not be trusted"
                );
            }
        }
    }

    fn inventory_list_roots() -> Vec<&'static str> {
        include_str!("bundled-inventory.toml")
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let value = line.strip_prefix("root")?.trim().strip_prefix('=')?.trim();
                value.strip_prefix('"')?.strip_suffix('"')
            })
            .collect()
    }

    #[test]
    fn real_bundled_package_verifies_trusted() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/markdown");
        let provenance = PackageProvenance {
            requested_spec: "@clay/markdown".to_string(),
            source_kind: PackageSourceKind::ClayShipped,
            resolved_name: "@clay/markdown".to_string(),
            resolved_version: "0.1.0".to_string(),
            package_root: root,
            lockfile_path: None,
            integrity: None,
            diagnostics: String::new(),
        };
        assert_eq!(
            verify_bundled_trust(&provenance).map(|entry| entry.name),
            Ok("@clay/markdown")
        );
        assert_eq!(runtime_domain(&provenance), RuntimeDomain::Trusted);
    }

    #[test]
    fn spoofed_bundled_provenance_fails_closed() {
        let real_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/markdown");
        let base = PackageProvenance {
            requested_spec: "file:/tmp/fake".to_string(),
            source_kind: PackageSourceKind::LocalPath,
            resolved_name: "@clay/markdown".to_string(),
            resolved_version: "0.1.0".to_string(),
            package_root: real_root.clone(),
            lockfile_path: None,
            integrity: None,
            diagnostics: String::new(),
        };
        // Local/npm/git source kinds never classify trusted, even at the real root.
        assert_eq!(
            verify_bundled_trust(&base),
            Err(BundledTrustError::SourceKindMismatch)
        );
        // Wrong version at the real root fails.
        let mut wrong_version = base.clone();
        wrong_version.source_kind = PackageSourceKind::ClayShipped;
        wrong_version.resolved_version = "9.9.9".to_string();
        assert_eq!(
            verify_bundled_trust(&wrong_version),
            Err(BundledTrustError::VersionMismatch)
        );
        // Right name/version at a foreign root fails.
        let mut wrong_root = base.clone();
        wrong_root.source_kind = PackageSourceKind::ClayShipped;
        wrong_root.package_root = std::env::temp_dir();
        assert_eq!(
            verify_bundled_trust(&wrong_root),
            Err(BundledTrustError::RootMismatch)
        );
        // Unknown @clay/* name fails.
        let mut unknown = base.clone();
        unknown.source_kind = PackageSourceKind::ClayShipped;
        unknown.resolved_name = "@clay/evil".to_string();
        assert_eq!(
            verify_bundled_trust(&unknown),
            Err(BundledTrustError::UnknownPackage)
        );
        for provenance in [&base, &wrong_version, &wrong_root, &unknown] {
            assert_eq!(runtime_domain(provenance), RuntimeDomain::ThirdParty);
        }
    }

    #[test]
    fn tampered_manifest_fails_integrity() {
        let entry = bundled_entry("@clay/markdown").expect("markdown entry");
        let root = canonical_bundled_root(entry).expect("canonical root");
        let bytes = std::fs::read(root.join("package.json")).expect("manifest readable");
        assert_eq!(fnv1a64_hex(&bytes), entry.manifest_fingerprint);
        let mut tampered = bytes;
        tampered.push(b' ');
        assert_ne!(fnv1a64_hex(&tampered), entry.manifest_fingerprint);
    }

    /// Plan 061 task 8: every bundled package declares extension points whose
    /// scopes name only real contributions of that package (or its grammar
    /// language id) — no declaration may reference foreign or invented ids.
    #[test]
    fn bundled_extension_points_match_real_contributions() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        // Contribution ids registered by package load entries at runtime
        // rather than through manifest contribution sections.
        const RUNTIME_IDS: &[&str] = &[
            "markdown.preview",
            "markdown.preview.root",
            "markdown.previewStatus",
            "markdown.syntaxDecorations",
            "markdown.parseDecorations",
        ];
        for entry in BUNDLED_PACKAGES {
            let manifest_path = packages_dir.join(entry.root).join("package.json");
            let bytes = std::fs::read(&manifest_path).expect("bundled manifest readable");
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("bundled manifest parses");
            let record = crate::packages::record::assemble_package_record(&value)
                .unwrap_or_else(|error| panic!("{} assembles: {}", entry.name, error.message));
            let points = &record.manifest.clay.extension_points;
            assert!(
                !points.is_empty(),
                "{} must declare at least one extension point",
                entry.name
            );
            let contributions = &record.contributions;
            let mut ids: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
            ids.extend(contributions.commands.iter().map(|d| d.id.as_str()));
            ids.extend(
                contributions
                    .completion_providers
                    .iter()
                    .map(|d| d.id.as_str()),
            );
            ids.extend(contributions.language_servers.iter().map(|d| d.id.as_str()));
            ids.extend(
                contributions
                    .language_intelligence_providers
                    .iter()
                    .map(|d| d.id.as_str()),
            );
            ids.extend(contributions.sdui.iter().map(|d| d.region_id.as_str()));
            ids.extend(contributions.ui_components.iter().map(|d| d.id.as_str()));
            ids.extend(contributions.ui_panels.iter().map(|d| d.id.as_str()));
            ids.extend(contributions.syntax_grammars.iter().map(|d| d.id.as_str()));
            ids.extend(
                crate::server::syntax::SyntaxGrammarRegistry::native_owned_grammar_ids(
                    &record.manifest.clay.api_prefix,
                ),
            );
            ids.extend(RUNTIME_IDS.iter().copied());
            for point in points {
                assert!(
                    point.version >= 1,
                    "{} point {} needs a positive version",
                    entry.name,
                    point.id
                );
                for scope in &point.scopes {
                    assert!(
                        ids.contains(scope.as_str()),
                        "{} extension point {} declares scope `{scope}` that names no real contribution of the package",
                        entry.name,
                        point.id
                    );
                }
            }
        }
    }
}
