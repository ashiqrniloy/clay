use crate::packages::manager::{PackageProvenance, PackageSourceKind};
use crate::packages::permissions::PackagePermission;

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
