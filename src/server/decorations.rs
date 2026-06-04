use crate::packages::permissions::PackagePermission;
use crate::packages::record::PackageRecord;
use crate::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES;
use crate::protocol::{DecorationSet, DocumentVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecorationValidationError {
    MissingPermission {
        package_prefix: String,
    },
    StaleDocumentVersion {
        decoration_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    InvalidViewportRange,
    InvalidSpanRange {
        index: usize,
    },
    SpanOutsideViewport {
        index: usize,
    },
    EmptyStyleToken {
        index: usize,
    },
    UnknownStyleToken {
        index: usize,
        style_token: String,
    },
    PackageProvenanceMismatch {
        index: usize,
    },
    PayloadBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    SerializationFailed,
}

pub fn validate_decoration_publication(
    package: &PackageRecord,
    current_document_version: DocumentVersion,
    set: DecorationSet,
) -> Result<DecorationSet, DecorationValidationError> {
    if !package
        .manifest
        .clay
        .permissions
        .contains(&PackagePermission::RenderDecorations)
    {
        return Err(DecorationValidationError::MissingPermission {
            package_prefix: package.manifest.clay.api_prefix.clone(),
        });
    }

    if set.document_version != current_document_version {
        return Err(DecorationValidationError::StaleDocumentVersion {
            decoration_version: set.document_version,
            current_version: current_document_version,
        });
    }

    validate_decoration_set(current_document_version, set, Some(package))
}

pub fn validate_decoration_set(
    current_document_version: DocumentVersion,
    set: DecorationSet,
    package: Option<&PackageRecord>,
) -> Result<DecorationSet, DecorationValidationError> {
    if set.document_version != current_document_version {
        return Err(DecorationValidationError::StaleDocumentVersion {
            decoration_version: set.document_version,
            current_version: current_document_version,
        });
    }
    if set.viewport_byte_start > set.viewport_byte_end {
        return Err(DecorationValidationError::InvalidViewportRange);
    }

    for (index, span) in set.spans.iter().enumerate() {
        if span.byte_start >= span.byte_end {
            return Err(DecorationValidationError::InvalidSpanRange { index });
        }
        if span.byte_start < set.viewport_byte_start || span.byte_end > set.viewport_byte_end {
            return Err(DecorationValidationError::SpanOutsideViewport { index });
        }
        if span.style_token.trim().is_empty()
            || span.style_token.contains('{')
            || span.style_token.contains('}')
        {
            return Err(DecorationValidationError::EmptyStyleToken { index });
        }
        if !is_known_style_token(&span.style_token) {
            return Err(DecorationValidationError::UnknownStyleToken {
                index,
                style_token: span.style_token.clone(),
            });
        }
        if let Some(package) = package {
            let provenance = &span.provenance;
            if provenance.package_name != package.manifest.name
                || provenance.package_version != package.manifest.version
                || provenance.package_prefix != package.manifest.clay.api_prefix
            {
                return Err(DecorationValidationError::PackageProvenanceMismatch { index });
            }
        }
    }

    let ordered = set.sorted_viewport_first();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&ordered)
        .map_err(|_| DecorationValidationError::SerializationFailed)?
        .len();
    if bytes > DECORATION_PAYLOAD_BUDGET_BYTES {
        return Err(DecorationValidationError::PayloadBudgetExceeded {
            bytes,
            budget: DECORATION_PAYLOAD_BUDGET_BYTES,
        });
    }

    Ok(ordered)
}

fn is_known_style_token(style_token: &str) -> bool {
    matches!(
        style_token,
        "markup.heading.1"
            | "markup.heading.2"
            | "markup.heading.3"
            | "markup.heading.4"
            | "markup.heading.5"
            | "markup.heading.6"
            | "markup.strong"
            | "markup.emphasis"
            | "markup.inline-code"
            | "markup.code-block"
            | "markup.list-marker"
            | "keyword.control"
            | "string.quoted"
            | "comment.line"
            | "punctuation.definition"
            | "diagnostic.error"
            | "diagnostic.warning"
            | "diagnostic.info"
            | "search.match"
            | "text"
    )
}
