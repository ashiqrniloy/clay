//! Durable, host-owned package approval records (Plan 061 task 6,
//! `clay-package-approval-v1` / `clay-package-replacement-v1`).
//!
//! One record per adopted package binds exact identity (name, resolved
//! version, source, integrity, root, api prefix), the complete granted
//! capability/process sets, approved relation requests, and approved
//! replacements. Package code can never author or mutate these records; the
//! only writers are host flows (`PackageService`).
//!
//! The store is a single JSON document under the package store root with
//! restrictive permissions (Unix `0o600`), written atomically (temp file +
//! fsync + rename). Loading fails closed on corruption, truncation, unknown
//! store version, oversize payloads, or unsafe permissions: a store Clay
//! cannot trust behaves as if no package were ever approved.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::packages::extension_points::{MAX_EXTENSION_SCOPES, MAX_SCOPE_CHARS};
use crate::packages::manager::PackageProvenance;
use crate::packages::manifest::PackageGraphRelations;
use crate::packages::permissions::PackagePermission;

/// Store file name inside the package store root.
pub const APPROVAL_STORE_FILE_NAME: &str = "clay-package-approvals.json";
/// Store format version; unknown versions fail closed.
pub const APPROVAL_STORE_VERSION: u64 = 1;
/// Maximum approvals held by the store.
pub const MAX_APPROVAL_RECORDS: usize = 256;
/// Maximum serialized store size accepted at load (fail closed above).
pub const MAX_APPROVAL_STORE_BYTES: usize = 256 * 1024;
/// Maximum compatibility claims per replacement record.
pub const MAX_REPLACEMENT_COMPATIBILITY_CLAIMS: usize = 32;
/// Maximum characters per compatibility claim.
pub const MAX_COMPATIBILITY_CLAIM_CHARS: usize = 128;

/// One approved relation edge (`clay-package-relation-v1`, durable form).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedRelation {
    pub package: String,
    pub extension_point: String,
    pub version: u64,
    pub operation: String,
    pub scopes: Vec<String>,
}

/// One approved full-package replacement (`clay-package-replacement-v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedReplacement {
    pub target: String,
    pub replacement_package: String,
    pub replacement_version: String,
    pub replacement_source: String,
    pub replacement_integrity: Option<String>,
    /// Host-computed exact contribution ids withdrawn, shown at approval.
    pub withdrawn_contributions: Vec<String>,
    pub compatibility_claims: Vec<String>,
    pub rollback_restore_target: bool,
}

/// Durable per-package approval (`clay-package-approval-v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageApprovalRecord {
    pub package: String,
    pub resolved_version: String,
    /// Requested specifier the user approved (exact-match bound).
    pub source: String,
    pub integrity: Option<String>,
    pub package_root: String,
    pub api_prefix: String,
    /// Complete granted capability set (permission strings).
    pub capabilities: Vec<String>,
    /// Complete granted language-server contribution id set.
    pub processes: Vec<String>,
    pub relations: Vec<ApprovedRelation>,
    pub replacements: Vec<ApprovedReplacement>,
    pub approved_by: String,
    /// RFC 3339 timestamp supplied by the host approval flow.
    pub approved_at: String,
    pub revoked: bool,
}

// ── Manual JSON (de)serialization ────────────────────────────────────────────
// The crate intentionally has no serde derive dependency; the store uses
// explicit bounded conversions instead.

fn json_strings(value: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    let array = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("approval record field `{field}` must be an array"))?;
    array
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("approval record field `{field}` entries must be strings"))
        })
        .collect()
}

fn json_required_string(object: &Map<String, Value>, field: &str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("approval record field `{field}` must be a string"))
}

fn json_optional_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|raw| Some(raw.to_owned()))
            .ok_or_else(|| format!("approval record field `{field}` must be a string or null")),
    }
}

impl ApprovedRelation {
    fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "approval relation must be an object".to_string())?;
        Ok(Self {
            package: json_required_string(object, "package")?,
            extension_point: json_required_string(object, "extension_point")?,
            version: object
                .get("version")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    "approval relation `version` must be a positive integer".to_string()
                })?,
            operation: json_required_string(object, "operation")?,
            scopes: json_strings(object.get("scopes"), "relation.scopes")?,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "package": self.package,
            "extension_point": self.extension_point,
            "version": self.version,
            "operation": self.operation,
            "scopes": self.scopes,
        })
    }
}

impl ApprovedReplacement {
    fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "approval replacement must be an object".to_string())?;
        Ok(Self {
            target: json_required_string(object, "target")?,
            replacement_package: json_required_string(object, "replacement_package")?,
            replacement_version: json_required_string(object, "replacement_version")?,
            replacement_source: json_required_string(object, "replacement_source")?,
            replacement_integrity: json_optional_string(object, "replacement_integrity")?,
            withdrawn_contributions: json_strings(
                object.get("withdrawn_contributions"),
                "replacement.withdrawn_contributions",
            )?,
            compatibility_claims: json_strings(
                object.get("compatibility_claims"),
                "replacement.compatibility_claims",
            )?,
            rollback_restore_target: object
                .get("rollback_restore_target")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    "approval replacement `rollback_restore_target` must be a boolean".to_string()
                })?,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "target": self.target,
            "replacement_package": self.replacement_package,
            "replacement_version": self.replacement_version,
            "replacement_source": self.replacement_source,
            "replacement_integrity": self.replacement_integrity,
            "withdrawn_contributions": self.withdrawn_contributions,
            "compatibility_claims": self.compatibility_claims,
            "rollback_restore_target": self.rollback_restore_target,
        })
    }
}

impl PackageApprovalRecord {
    fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "approval record must be an object".to_string())?;
        Ok(Self {
            package: json_required_string(object, "package")?,
            resolved_version: json_required_string(object, "resolved_version")?,
            source: json_required_string(object, "source")?,
            integrity: json_optional_string(object, "integrity")?,
            package_root: json_required_string(object, "package_root")?,
            api_prefix: json_required_string(object, "api_prefix")?,
            capabilities: json_strings(object.get("capabilities"), "capabilities")?,
            processes: json_strings(object.get("processes"), "processes")?,
            relations: object
                .get("relations")
                .and_then(Value::as_array)
                .ok_or_else(|| "approval record `relations` must be an array".to_string())?
                .iter()
                .map(ApprovedRelation::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            replacements: object
                .get("replacements")
                .and_then(Value::as_array)
                .ok_or_else(|| "approval record `replacements` must be an array".to_string())?
                .iter()
                .map(ApprovedReplacement::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            approved_by: json_required_string(object, "approved_by")?,
            approved_at: json_required_string(object, "approved_at")?,
            revoked: object
                .get("revoked")
                .and_then(Value::as_bool)
                .ok_or_else(|| "approval record `revoked` must be a boolean".to_string())?,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "package": self.package,
            "resolved_version": self.resolved_version,
            "source": self.source,
            "integrity": self.integrity,
            "package_root": self.package_root,
            "api_prefix": self.api_prefix,
            "capabilities": self.capabilities,
            "processes": self.processes,
            "relations": self.relations.iter().map(ApprovedRelation::to_json).collect::<Vec<_>>(),
            "replacements": self
                .replacements
                .iter()
                .map(ApprovedReplacement::to_json)
                .collect::<Vec<_>>(),
            "approved_by": self.approved_by,
            "approved_at": self.approved_at,
            "revoked": self.revoked,
        })
    }
}

/// Why an approval does not cover a request. Codes are deterministic for
/// diagnostics and adoption-prompt rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalMismatch {
    NotFound,
    Revoked,
    IdentityChanged { field: &'static str },
    CapabilityExpansion { capability: String },
    ProcessExpansion { process: String },
    RelationExpansion { relation: String },
    ReplacementExpansion { target: String },
}

impl ApprovalMismatch {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "package_approval.missing",
            Self::Revoked => "package_approval.revoked",
            Self::IdentityChanged { .. } => "package_approval.identity_changed",
            Self::CapabilityExpansion { .. } => "package_approval.capability_expansion",
            Self::ProcessExpansion { .. } => "package_approval.process_expansion",
            Self::RelationExpansion { .. } => "package_approval.relation_expansion",
            Self::ReplacementExpansion { .. } => "package_approval.replacement_expansion",
        }
    }
}

/// Store load/persist failures. Every variant is fail-closed: callers must
/// not treat them as "no approvals".
#[derive(Debug)]
pub(crate) enum ApprovalStoreError {
    Corrupt { reason: String },
    UnsafePermissions,
    Io(std::io::Error),
}

impl std::fmt::Display for ApprovalStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt { reason } => {
                write!(f, "package_approval_store.corrupt: {reason}")
            }
            Self::UnsafePermissions => write!(
                f,
                "package_approval_store.unsafe_permissions: approval store must be owner-only"
            ),
            Self::Io(error) => write!(f, "package_approval_store.io: {error}"),
        }
    }
}

impl std::error::Error for ApprovalStoreError {}

/// Host-owned approval store. `path = None` is the in-memory form used by
/// tests and ephemeral services; such a store holds no durable approvals and
/// never persists.
pub(crate) struct PackageApprovalStore {
    path: Option<PathBuf>,
    approvals: HashMap<String, PackageApprovalRecord>,
}

impl std::fmt::Debug for PackageApprovalStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageApprovalStore")
            .field("path", &self.path)
            .field("approvals", &self.approvals.len())
            .finish()
    }
}

impl PackageApprovalStore {
    /// In-memory store (tests, ephemeral services). `save` is a no-op.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            approvals: HashMap::new(),
        }
    }

    /// Open the durable store at `<store_root>/clay-package-approvals.json`.
    /// A missing file yields an empty store; any corruption, truncation,
    /// unknown version, oversize payload, duplicate record, or unsafe file
    /// permissions fails closed.
    pub fn open(store_root: &Path) -> Result<Self, ApprovalStoreError> {
        let path = store_root.join(APPROVAL_STORE_FILE_NAME);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: Some(path),
                    approvals: HashMap::new(),
                });
            }
            Err(error) => return Err(ApprovalStoreError::Io(error)),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(ApprovalStoreError::UnsafePermissions);
            }
        }
        if metadata.len() as usize > MAX_APPROVAL_STORE_BYTES {
            return Err(ApprovalStoreError::Corrupt {
                reason: format!("store exceeds {MAX_APPROVAL_STORE_BYTES} bytes"),
            });
        }
        let bytes = fs::read(&path).map_err(ApprovalStoreError::Io)?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|error| ApprovalStoreError::Corrupt {
                reason: format!("invalid store JSON: {error}"),
            })?;
        let version = document.get("version").and_then(Value::as_u64);
        if version != Some(APPROVAL_STORE_VERSION) {
            return Err(ApprovalStoreError::Corrupt {
                reason: format!(
                    "unknown store version {version:?} (expected {APPROVAL_STORE_VERSION})"
                ),
            });
        }
        let records = document
            .get("approvals")
            .and_then(Value::as_array)
            .ok_or_else(|| ApprovalStoreError::Corrupt {
                reason: "store `approvals` must be an array".to_string(),
            })?;
        if records.len() > MAX_APPROVAL_RECORDS {
            return Err(ApprovalStoreError::Corrupt {
                reason: format!("store exceeds {MAX_APPROVAL_RECORDS} records"),
            });
        }
        let mut approvals = HashMap::with_capacity(records.len());
        for record_value in records {
            let record = PackageApprovalRecord::from_json(record_value)
                .map_err(|reason| ApprovalStoreError::Corrupt { reason })?;
            validate_record(&record).map_err(|reason| ApprovalStoreError::Corrupt { reason })?;
            if approvals.insert(record.package.clone(), record).is_some() {
                return Err(ApprovalStoreError::Corrupt {
                    reason: "duplicate approval record".to_string(),
                });
            }
        }
        Ok(Self {
            path: Some(path),
            approvals,
        })
    }

    #[cfg(test)]
    pub(crate) fn get(&self, package: &str) -> Option<&PackageApprovalRecord> {
        self.approvals.get(package)
    }

    pub fn records(&self) -> impl Iterator<Item = &PackageApprovalRecord> {
        self.approvals.values()
    }

    /// Insert or replace one host-authored record and persist. This is the
    /// only mutation entry point; package code has no path to it.
    pub fn upsert(&mut self, record: PackageApprovalRecord) -> Result<(), ApprovalStoreError> {
        validate_record(&record).map_err(|reason| ApprovalStoreError::Corrupt { reason })?;
        if !self.approvals.contains_key(&record.package)
            && self.approvals.len() >= MAX_APPROVAL_RECORDS
        {
            return Err(ApprovalStoreError::Corrupt {
                reason: format!("store is full ({MAX_APPROVAL_RECORDS} records)"),
            });
        }
        self.approvals.insert(record.package.clone(), record);
        self.save()
    }

    /// Clone of the in-memory approval map for transactional snapshots.
    pub(crate) fn snapshot(&self) -> HashMap<String, PackageApprovalRecord> {
        self.approvals.clone()
    }

    /// Restore a previously taken snapshot and persist. Used by the enable
    /// transaction's rollback path so approval mutations stay atomic with
    /// package-graph mutations.
    pub(crate) fn restore(
        &mut self,
        snapshot: HashMap<String, PackageApprovalRecord>,
    ) -> Result<(), ApprovalStoreError> {
        self.approvals = snapshot;
        self.save()
    }

    /// Mark a record revoked and persist. Revocation keeps the record so
    /// diagnostics can distinguish "revoked" from "never approved".
    pub fn revoke(&mut self, package: &str) -> Result<bool, ApprovalStoreError> {
        let Some(record) = self.approvals.get_mut(package) else {
            return Ok(false);
        };
        record.revoked = true;
        self.save()?;
        Ok(true)
    }

    fn save(&self) -> Result<(), ApprovalStoreError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut approvals: Vec<&PackageApprovalRecord> = self.approvals.values().collect();
        approvals.sort_by(|a, b| a.package.cmp(&b.package));
        let document = serde_json::json!({
            "version": APPROVAL_STORE_VERSION,
            "approvals": approvals
                .into_iter()
                .map(PackageApprovalRecord::to_json)
                .collect::<Vec<_>>(),
        });
        let bytes =
            serde_json::to_vec_pretty(&document).map_err(|error| ApprovalStoreError::Corrupt {
                reason: format!("store serialization failed: {error}"),
            })?;
        // The store can be written before its parent directory exists (first
        // approval in a fresh config root); create it so the atomic write
        // cannot fail with ENOENT and mask the caller's original error.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ApprovalStoreError::Io)?;
        }
        atomic_write_owner_only(path, &bytes)
    }

    /// Check that a current, unrevoked approval covers the exact request.
    ///
    /// Identity fields must match exactly; requested capabilities, processes,
    /// relations, and replacements must each be an exact subset of the
    /// approved sets (a narrower request reuses the approval; any expansion
    /// requires re-approval).
    pub fn approval_covers(
        &self,
        provenance: &PackageProvenance,
        api_prefix: &str,
        capabilities: &[PackagePermission],
        processes: &[String],
        relations: &PackageGraphRelations,
    ) -> Result<(), ApprovalMismatch> {
        let record = self
            .approvals
            .get(&provenance.resolved_name)
            .ok_or(ApprovalMismatch::NotFound)?;
        if record.revoked {
            return Err(ApprovalMismatch::Revoked);
        }
        for (field, expected, actual) in [
            (
                "resolved_version",
                record.resolved_version.as_str(),
                provenance.resolved_version.as_str(),
            ),
            (
                "source",
                record.source.as_str(),
                provenance.requested_spec.as_str(),
            ),
            (
                "package_root",
                record.package_root.as_str(),
                provenance.package_root.to_string_lossy().as_ref(),
            ),
            ("api_prefix", record.api_prefix.as_str(), api_prefix),
        ] {
            if expected != actual {
                return Err(ApprovalMismatch::IdentityChanged { field });
            }
        }
        if record.integrity != provenance.integrity {
            return Err(ApprovalMismatch::IdentityChanged { field: "integrity" });
        }
        for permission in capabilities {
            let raw = permission.as_str();
            if !record.capabilities.iter().any(|approved| approved == raw) {
                return Err(ApprovalMismatch::CapabilityExpansion {
                    capability: raw.to_string(),
                });
            }
        }
        for process in processes {
            if !record.processes.iter().any(|approved| approved == process) {
                return Err(ApprovalMismatch::ProcessExpansion {
                    process: process.clone(),
                });
            }
        }
        // Replacement edges are user-owned graph control (Plan 061 task 11):
        // every declared `replaces` target must appear in the approved
        // replacement list. An unapproved replacement edge never executes.
        for target in &relations.replaces {
            if !record
                .replacements
                .iter()
                .any(|approved| approved.target == *target)
            {
                return Err(ApprovalMismatch::ReplacementExpansion {
                    target: target.clone(),
                });
            }
        }
        for request in &relations.relation_requests {
            let key = format!(
                "{}:{}@{}:{}",
                request.package,
                request.extension_point,
                request.version,
                request.operation.as_str()
            );
            let covered = record.relations.iter().any(|approved| {
                approved.package == request.package
                    && approved.extension_point == request.extension_point
                    && approved.version == request.version
                    && approved.operation == request.operation.as_str()
                    && request
                        .scopes
                        .iter()
                        .all(|scope| approved.scopes.contains(scope))
            });
            if !covered {
                return Err(ApprovalMismatch::RelationExpansion { relation: key });
            }
        }
        Ok(())
    }
}

/// Validate one record's shape and bounds (used at load and upsert).
fn validate_record(record: &PackageApprovalRecord) -> Result<(), String> {
    if record.package.trim().is_empty()
        || record.resolved_version.trim().is_empty()
        || record.api_prefix.trim().is_empty()
    {
        return Err("approval record identity fields must be non-empty".to_string());
    }
    for relation in &record.relations {
        if relation.scopes.len() > MAX_EXTENSION_SCOPES
            || relation
                .scopes
                .iter()
                .any(|scope| scope.chars().count() > MAX_SCOPE_CHARS)
        {
            return Err(format!(
                "approval relation `{}` exceeds scope bounds",
                relation.extension_point
            ));
        }
    }
    for replacement in &record.replacements {
        if replacement.compatibility_claims.len() > MAX_REPLACEMENT_COMPATIBILITY_CLAIMS
            || replacement
                .compatibility_claims
                .iter()
                .any(|claim| claim.chars().count() > MAX_COMPATIBILITY_CLAIM_CHARS)
        {
            return Err(format!(
                "approval replacement `{}` exceeds compatibility-claim bounds",
                replacement.target
            ));
        }
    }
    Ok(())
}

/// Write `bytes` to `path` atomically with owner-only permissions: create a
/// same-directory temp file with mode `0o600`, write + fsync, then rename
/// over the target. The rename is atomic, so a crash mid-write never leaves
/// a torn store; a stale temp is removed on failure.
// ponytail: no parent-directory fsync; the atomic rename already guarantees
// the store is never torn, matching src/server/workspace.rs atomic saves.
// Plan 060 filesystem-integrity work may consolidate this helper later.
fn atomic_write_owner_only(path: &Path, bytes: &[u8]) -> Result<(), ApprovalStoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clay-approvals".to_string());
    let temp_path = parent.join(format!(".{stem}.tmp-{}", std::process::id()));

    let result = (|| -> Result<(), ApprovalStoreError> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp_path).map_err(ApprovalStoreError::Io)?;
        file.write_all(bytes).map_err(ApprovalStoreError::Io)?;
        file.sync_all().map_err(ApprovalStoreError::Io)?;
        drop(file);
        #[cfg(unix)]
        {
            // Ensure owner-only mode even if a previous store existed with
            // wider permissions (rename preserves the temp's mode).
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))
                .map_err(ApprovalStoreError::Io)?;
        }
        fs::rename(&temp_path, path).map_err(ApprovalStoreError::Io)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

/// Current UTC time as RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`) for approval
/// records. No chrono dependency: days-to-civil conversion from seconds.
pub(crate) fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    // Howard Hinnant's civil_from_days algorithm.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-approvals-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn record(package: &str) -> PackageApprovalRecord {
        PackageApprovalRecord {
            package: package.to_string(),
            resolved_version: "1.2.3".to_string(),
            source: "npm:@vendor/example@1.2.3".to_string(),
            integrity: Some("sha512-abc".to_string()),
            package_root: "/clay/packages/node_modules/@vendor/example".to_string(),
            api_prefix: "example".to_string(),
            capabilities: vec![
                "mode-registration".to_string(),
                "completion-provider".to_string(),
            ],
            processes: vec!["example.server".to_string()],
            relations: vec![ApprovedRelation {
                package: "@clay/markdown".to_string(),
                extension_point: "markdown.completionProviders".to_string(),
                version: 1,
                operation: "append".to_string(),
                scopes: vec!["example.wikilinks".to_string()],
            }],
            replacements: Vec::new(),
            approved_by: "user".to_string(),
            approved_at: "2026-07-21T00:00:00Z".to_string(),
            revoked: false,
        }
    }

    /// Phase 22.6 (plan 077 task 6): package scopes are host-owned and
    /// tab-independent. Approval records bind package identity and granted
    /// capability/process sets only — no client, tab, pane, or workspace
    /// keying — so tab create/close/move can neither widen nor narrow them.
    /// The exact key set is pinned: a future grant-carrying field changes
    /// the count and fails this test.
    #[test]
    fn approval_records_carry_no_tab_client_or_workspace_keying() {
        let object = record("@vendor/example").to_json();
        let object = object.as_object().expect("record serializes as an object");
        let mut keys: Vec<&String> = object.keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "api_prefix",
                "approved_at",
                "approved_by",
                "capabilities",
                "integrity",
                "package",
                "package_root",
                "processes",
                "relations",
                "replacements",
                "resolved_version",
                "revoked",
                "source",
            ],
            "approval record shape is the documented package-identity field set"
        );
        for forbidden in ["client", "tab", "pane", "workspace", "connection"] {
            assert!(
                !keys.iter().any(|key| key.contains(forbidden)),
                "approval record must not carry {forbidden} keying"
            );
        }
    }

    #[test]
    fn store_round_trips_and_revokes() {
        let root = temp_root("roundtrip");
        let mut store = PackageApprovalStore::open(&root).expect("missing file opens empty");
        assert!(store.records().next().is_none());
        store.upsert(record("@vendor/example")).unwrap();
        let reopened = PackageApprovalStore::open(&root).expect("store reloads");
        let loaded = reopened.get("@vendor/example").expect("record persisted");
        assert_eq!(loaded.resolved_version, "1.2.3");
        assert!(!loaded.revoked);

        let mut reopened = reopened;
        assert!(reopened.revoke("@vendor/example").unwrap());
        let reopened = PackageApprovalStore::open(&root).unwrap();
        assert!(reopened.get("@vendor/example").unwrap().revoked);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(root.join(APPROVAL_STORE_FILE_NAME))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "store must be owner-only, got {mode:o}");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn store_fails_closed_on_corrupt_truncated_and_unknown_version() {
        for (name, bytes, expected) in [
            ("corrupt", b"{ not json".as_slice(), "invalid store JSON"),
            (
                "truncated",
                b"{\"version\":1,\"approvals\":[{".as_slice(),
                "invalid store JSON",
            ),
            (
                "version",
                b"{\"version\":2,\"approvals\":[]}".as_slice(),
                "unknown store version",
            ),
        ] {
            let root = temp_root(name);
            let path = root.join(APPROVAL_STORE_FILE_NAME);
            fs::write(&path, bytes).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            let error = PackageApprovalStore::open(&root).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains(expected),
                "expected `{expected}`, got {message}"
            );
            let _ = fs::remove_dir_all(&root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn store_fails_closed_on_unsafe_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_root("perms");
        let path = root.join(APPROVAL_STORE_FILE_NAME);
        fs::write(&path, b"{\"version\":1,\"approvals\":[]}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let error = PackageApprovalStore::open(&root).unwrap_err();
        assert!(matches!(error, ApprovalStoreError::UnsafePermissions));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn approval_covers_exact_and_narrower_but_not_expanded_requests() {
        let mut store = PackageApprovalStore::in_memory();
        store.upsert(record("@vendor/example")).unwrap();
        let provenance = PackageProvenance {
            requested_spec: "npm:@vendor/example@1.2.3".to_string(),
            source_kind: crate::packages::manager::PackageSourceKind::NpmRegistry,
            resolved_name: "@vendor/example".to_string(),
            resolved_version: "1.2.3".to_string(),
            package_root: PathBuf::from("/clay/packages/node_modules/@vendor/example"),
            lockfile_path: None,
            integrity: Some("sha512-abc".to_string()),
            diagnostics: String::new(),
        };
        let mut relations = PackageGraphRelations::default();
        relations.relation_requests.push(
            crate::packages::extension_points::StructuredRelationRequest {
                package: "@clay/markdown".to_string(),
                extension_point: "markdown.completionProviders".to_string(),
                version: 1,
                operation: crate::packages::extension_points::RelationOperation::Append,
                scopes: vec!["example.wikilinks".to_string()],
                justification: None,
                relation_key: "extends".to_string(),
            },
        );

        // Exact request covered.
        store
            .approval_covers(
                &provenance,
                "example",
                &[
                    PackagePermission::ModeRegistration,
                    PackagePermission::CompletionProvider,
                ],
                &["example.server".to_string()],
                &relations,
            )
            .expect("exact request covered");
        // Narrower request reuses the approval.
        store
            .approval_covers(
                &provenance,
                "example",
                &[],
                &[],
                &PackageGraphRelations::default(),
            )
            .expect("narrower request reuses approval");

        // Identity drift fails.
        let drifted = PackageProvenance {
            resolved_version: "1.2.4".to_string(),
            ..provenance.clone()
        };
        assert!(matches!(
            store.approval_covers(
                &drifted,
                "example",
                &[],
                &[],
                &PackageGraphRelations::default()
            ),
            Err(ApprovalMismatch::IdentityChanged {
                field: "resolved_version"
            })
        ));
        // Capability expansion fails.
        assert!(matches!(
            store.approval_covers(
                &provenance,
                "example",
                &[PackagePermission::PackageControl],
                &[],
                &PackageGraphRelations::default()
            ),
            Err(ApprovalMismatch::CapabilityExpansion { .. })
        ));
        // Relation expansion (new scope) fails.
        let mut expanded = relations.clone();
        expanded.relation_requests[0]
            .scopes
            .push("example.other".to_string());
        assert!(matches!(
            store.approval_covers(&provenance, "example", &[], &[], &expanded),
            Err(ApprovalMismatch::RelationExpansion { .. })
        ));
        // Missing/revoked approvals fail closed.
        let missing = PackageProvenance {
            resolved_name: "@vendor/never".to_string(),
            ..provenance.clone()
        };
        assert!(matches!(
            store.approval_covers(
                &missing,
                "never",
                &[],
                &[],
                &PackageGraphRelations::default()
            ),
            Err(ApprovalMismatch::NotFound)
        ));
        store.revoke("@vendor/example").unwrap();
        assert!(matches!(
            store.approval_covers(
                &provenance,
                "example",
                &[],
                &[],
                &PackageGraphRelations::default()
            ),
            Err(ApprovalMismatch::Revoked)
        ));
    }
}
