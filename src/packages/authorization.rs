use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::packages::manager::{PackageProvenance, PackageSourceKind};
use crate::packages::permissions::PackagePermission;
use crate::packages::record::LanguageServerContributionDescriptor;
use crate::protocol::WorkspaceRootId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    NativeTrust,
    Sandboxed,
    Restricted,
}

impl RuntimeProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeTrust => "native-trust",
            Self::Sandboxed => "sandboxed",
            Self::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServerGrant {
    pub package_name: String,
    pub requested_spec: String,
    pub source_kind: PackageSourceKind,
    pub resolved_version: String,
    pub api_prefix: String,
    pub contribution_id: String,
    pub descriptor_fingerprint: u64,
    pub canonical_executable: PathBuf,
    pub workspace_root_ids: Vec<WorkspaceRootId>,
    pub approved_by: String,
    pub approved_at_unix_ms: u128,
}

impl LanguageServerGrant {
    pub fn new(
        provenance: &PackageProvenance,
        api_prefix: impl Into<String>,
        descriptor: &LanguageServerContributionDescriptor,
        canonical_executable: PathBuf,
        mut workspace_root_ids: Vec<WorkspaceRootId>,
        approved_by: impl Into<String>,
    ) -> Self {
        workspace_root_ids.sort_unstable();
        workspace_root_ids.dedup();
        Self {
            package_name: provenance.resolved_name.clone(),
            requested_spec: provenance.requested_spec.clone(),
            source_kind: provenance.source_kind,
            resolved_version: provenance.resolved_version.clone(),
            api_prefix: api_prefix.into(),
            contribution_id: descriptor.id.clone(),
            descriptor_fingerprint: language_server_descriptor_fingerprint(descriptor),
            canonical_executable,
            workspace_root_ids,
            approved_by: approved_by.into(),
            approved_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis()),
        }
    }

    pub fn matches(
        &self,
        provenance: &PackageProvenance,
        api_prefix: &str,
        descriptor: &LanguageServerContributionDescriptor,
    ) -> bool {
        self.package_name == provenance.resolved_name
            && self.requested_spec == provenance.requested_spec
            && self.source_kind == provenance.source_kind
            && self.resolved_version == provenance.resolved_version
            && self.api_prefix == api_prefix
            && self.contribution_id == descriptor.id
            && self.descriptor_fingerprint == language_server_descriptor_fingerprint(descriptor)
            && !self.workspace_root_ids.is_empty()
    }
}

pub fn language_server_descriptor_fingerprint(
    descriptor: &LanguageServerContributionDescriptor,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    descriptor.hash(&mut hasher);
    hasher.finish()
}

pub fn resolve_language_server_executable(executable: &str) -> Option<PathBuf> {
    let path = PathBuf::from(executable);
    let candidate = if path.is_absolute() || path.components().count() > 1 {
        path
    } else {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .map(|directory| directory.join(&path))
            .find(|candidate| candidate.is_file())?
    };
    std::fs::canonicalize(candidate)
        .ok()
        .filter(|canonical| canonical.is_file())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAuthorizationRecord {
    pub package_name: String,
    pub requested_spec: String,
    pub source_kind: PackageSourceKind,
    pub resolved_version: String,
    pub api_prefix: String,
    pub approved_capabilities: Vec<PackagePermission>,
    pub runtime_profile: RuntimeProfile,
    pub approved_by: String,
}

impl PackageAuthorizationRecord {
    pub fn new(
        provenance: &PackageProvenance,
        api_prefix: impl Into<String>,
        approved_capabilities: Vec<PackagePermission>,
        runtime_profile: RuntimeProfile,
        approved_by: impl Into<String>,
    ) -> Self {
        Self {
            package_name: provenance.resolved_name.clone(),
            requested_spec: provenance.requested_spec.clone(),
            source_kind: provenance.source_kind,
            resolved_version: provenance.resolved_version.clone(),
            api_prefix: api_prefix.into(),
            approved_capabilities,
            runtime_profile,
            approved_by: approved_by.into(),
        }
    }

    pub fn grants(&self, permission: PackagePermission) -> bool {
        self.approved_capabilities.contains(&permission)
    }

    pub fn approved_capability_names(&self) -> Vec<String> {
        self.approved_capabilities
            .iter()
            .map(|capability| capability.as_str().to_string())
            .collect()
    }
}
