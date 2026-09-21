//! Document metadata/access and the bounded document-head/chunk contract.

use super::*;

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct DocumentTextHead {
    pub total_bytes: u64,
    pub first_chunk: String,
}

impl DocumentTextHead {
    pub fn complete(text: String) -> Self {
        Self {
            total_bytes: text.len() as u64,
            first_chunk: text,
        }
    }
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DocumentChunkRejection {
    InvalidRequestSize {
        requested_bytes: u32,
        minimum_bytes: u32,
        maximum_bytes: u32,
    },
    InvalidOffset,
    StaleVersion {
        current_version: DocumentVersion,
    },
    UnknownDocument,
}

/// Validates and clamps an untrusted chunk-size request before rope access.
pub fn bounded_document_chunk_bytes(max_bytes: u32) -> Result<usize, DocumentChunkRejection> {
    const MINIMUM_BYTES: u32 = 4;
    if max_bytes < MINIMUM_BYTES {
        return Err(DocumentChunkRejection::InvalidRequestSize {
            requested_bytes: max_bytes,
            minimum_bytes: MINIMUM_BYTES,
            maximum_bytes: crate::perf::budgets::MAX_CHUNK_BYTES as u32,
        });
    }
    Ok((max_bytes as usize).min(crate::perf::budgets::MAX_CHUNK_BYTES))
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub access: DocumentAccess,
    pub lease_id: Option<LeaseId>,
    pub dirty: bool,
    pub workspace_root_id: WorkspaceRootId,
    pub path: String,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FileErrorCode {
    UnknownWorkspaceRoot,
    UnknownDocument,
    NotFound,
    AccessDenied,
    OutsideRoot,
    InvalidUtf8,
    PermissionDenied,
    UnsupportedFileType,
    DirectoryOpen,
    DirtyDocument,
    StaleFileMetadata,
    DocumentBudgetExceeded,
    BinaryFileNotSupported,
    WorkspaceLimitExceeded,
    InternalError,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum DocumentAccess {
    ReadOnly,
    Editable { lease_id: LeaseId },
}

impl DocumentAccess {
    pub const fn lease_id(&self) -> Option<LeaseId> {
        match self {
            Self::ReadOnly => None,
            Self::Editable { lease_id } => Some(*lease_id),
        }
    }
}
