//! Phase 18.11 completion protocol shapes.
//!
//! Typed completion request/result data for the `CompletionTriggerAndResult`
//! primitive. These shapes are the generic, language-neutral contract between
//! client trigger routing, the server-side cancellable provider lane, and the
//! `TransientMenuSession` display/accept path.
//!
//! # Authority boundary
//!
//! Completion items are inert text-replacement data only. No callbacks, snippets
//! with executable transforms, command side effects on accept, raw op names,
//! native handles, CSS, file paths, shell/network/AI directives, or client-side
//! JavaScript fields are represented here. A `CompletionItem` carries `label`,
//! `insert_text`, `detail`, `commit_characters`, and provenance only.
//!
//! These shapes add no filesystem, network, shell, AI, workspace-index, WASM,
//! raw-op, native-widget, client-runtime, package-manager, or package-enable
//! authority. Any future provider needing such authority must introduce explicit
//! permissions and an approved decision log before reuse.

use crate::perf::budgets::{
    COMPLETION_RESULT_MAX_ITEM_COMMIT_CHARS, COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
    COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS, COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
    COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
};
use crate::protocol::{BehaviorVersion, ClientId, DocumentId, DocumentVersion};

/// Monotonic per-client completion request identifier.
pub type CompletionRequestId = u64;

/// Monotonic completion provider generation. Bumped when providers are
/// registered, disabled, revoked, or reloaded so in-flight work can be
/// stale-dropped against the generation observed at request time.
pub type CompletionProviderGeneration = u64;

/// Package or built-in provenance retained on every completion item and result
/// set. Mirrors `DecorationProvenance`: package name, version, and the
/// package-prefixed API identifier.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompletionProvenance {
    pub package_name: String,
    pub package_version: String,
    pub package_prefix: String,
}

impl CompletionProvenance {
    /// Built-in `core` provenance used by the first-party buffer-word provider
    /// and any other built-in Rust providers that do not come from a package.
    pub fn builtin_core() -> Self {
        Self {
            package_name: "core".to_string(),
            package_version: "builtin".to_string(),
            package_prefix: "core".to_string(),
        }
    }
}

/// Why a completion request was issued. Trigger classification stays local and
/// manifest-driven; this is inert metadata carried to the server so providers
/// can shape results without executable trigger callbacks.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CompletionTrigger {
    /// A trigger character declared by an installed behavior manifest
    /// (`EditorBehaviorRules.autocomplete_triggers`). The carried string is the
    /// inert manifest trigger character; it is never executed.
    Character(String),
    /// A manual request issued by the bound `completion.trigger` command
    /// (Ctrl+Space). Manual requests never mutate document text.
    Manual,
}

/// Byte range in the document that a completion result replaces when an item is
/// accepted. `byte_start` must be <= `byte_end` and both must be valid offsets
/// within the requesting document at the request's `document_version`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionReplacementRange {
    pub byte_start: u64,
    pub byte_end: u64,
}

impl CompletionReplacementRange {
    pub fn new(byte_start: u64, byte_end: u64) -> Self {
        Self {
            byte_start,
            byte_end,
        }
    }

    /// Returns `true` when `byte_start <= byte_end`. Used by request validation
    /// before any provider work is scheduled.
    pub fn is_ordered(&self) -> bool {
        self.byte_start <= self.byte_end
    }
}

/// A typed, versioned completion request enqueued after a local-first edit or a
/// manual `completion.trigger` command. Carries enough metadata for the
/// server-side provider lane to stale-drop against newer edits, cursor moves,
/// mode changes, or provider generation changes.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompletionRequest {
    pub request_id: CompletionRequestId,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub cursor_byte_offset: u64,
    /// Range that a result's `insert_text` should replace when accepted.
    /// Typically the prefix/word range ending at `cursor_byte_offset`.
    pub replacement_range: CompletionReplacementRange,
    pub trigger: CompletionTrigger,
    /// Provider generation observed by the client when the request was built.
    /// Stale results whose `provider_generation` differs are dropped before UI
    /// publication.
    pub provider_generation: CompletionProviderGeneration,
}

/// One inert completion suggestion. Fields are text-replacement data only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Short human-readable label shown in the completion picker.
    pub label: String,
    /// Text inserted into the document when the item is accepted. Must replace
    /// the request's `replacement_range`.
    pub insert_text: String,
    /// Optional longer detail string shown alongside the label. May be empty.
    pub detail: String,
    /// Optional characters that, when typed while this item is selected, commit
    /// the item. May be empty. Carried as inert data; the client maps commit
    /// characters to local picker behavior only.
    pub commit_characters: String,
    pub provenance: CompletionProvenance,
}

impl CompletionItem {
    pub fn new(
        label: impl Into<String>,
        insert_text: impl Into<String>,
        provenance: CompletionProvenance,
    ) -> Self {
        Self {
            label: label.into(),
            insert_text: insert_text.into(),
            detail: String::new(),
            commit_characters: String::new(),
            provenance,
        }
    }

    /// Encoded byte length of this item's string fields. Used by the result
    /// payload budget check so a single item cannot blow the result budget.
    pub fn string_field_bytes(&self) -> usize {
        self.label.len()
            + self.insert_text.len()
            + self.detail.len()
            + self.commit_characters.len()
            + self.provenance.package_name.len()
            + self.provenance.package_version.len()
            + self.provenance.package_prefix.len()
    }
}

/// Status carried alongside a `CompletionResultSet`. Inert metadata only; the
/// client maps status to transient menu state without executing provider code.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStatus {
    /// At least one item was produced.
    Ok,
    /// The provider ran to completion but produced no items.
    Empty,
    /// The provider exceeded its timeout and returned a partial/empty result.
    Timeout,
    /// The provider reported an internal error. Carried as inert status; the
    /// client surfaces a transient menu diagnostic without executing code.
    ProviderError,
}

/// Why a completion result was rejected before client publication. Distinct from
/// `CompletionStatus`: rejections drop the result entirely; status publishes a
/// (possibly empty) result set.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CompletionRejection {
    /// The result's `document_version` is older than the document's current
    /// version (the cursor or text moved on).
    StaleDocumentVersion {
        result_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    /// The result's `behavior_version` no longer matches the installed manifest.
    StaleBehaviorVersion {
        result_version: BehaviorVersion,
        current_version: BehaviorVersion,
    },
    /// The result's `provider_generation` is older than the active generation
    /// (providers were registered/disabled/revoked/reloaded).
    StaleProviderGeneration {
        result_generation: CompletionProviderGeneration,
        current_generation: CompletionProviderGeneration,
    },
    /// The replacement range is not ordered (`byte_start > byte_end`) or is
    /// otherwise invalid for the requesting document.
    InvalidReplacementRange { byte_start: u64, byte_end: u64 },
    /// The encoded result payload exceeds `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`.
    PayloadTooLarge {
        payload_bytes: usize,
        budget_bytes: usize,
    },
    /// The result carries more items than `COMPLETION_RESULT_MAX_ITEMS`.
    TooManyItems { item_count: usize, max_items: usize },
    /// An item string field exceeds its per-field character budget.
    ItemFieldTooLong {
        field: CompletionItemField,
        length: usize,
        max_chars: usize,
    },
}

/// Which item string field exceeded its per-field budget. Used by
/// [`CompletionRejection::ItemFieldTooLong`].
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemField {
    Label,
    InsertText,
    Detail,
    CommitCharacters,
}

/// Bounded, versioned server-to-client completion result payload for one
/// request. Mirrors the `DecorationSet` shape: document/version metadata, a
/// bounded item vector, and package/built-in provenance.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompletionResultSet {
    pub request_id: CompletionRequestId,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub provider_generation: CompletionProviderGeneration,
    /// Range the provider assumed its `insert_text` values replace. Must match
    /// the request's `replacement_range` for an accepted item to commit
    /// cleanly.
    pub replacement_range: CompletionReplacementRange,
    pub status: CompletionStatus,
    pub items: Vec<CompletionItem>,
    pub provenance: CompletionProvenance,
}

impl CompletionResultSet {
    /// Returns `Ok(())` when the result set passes all payload, item-count,
    /// per-field, and ordering validation, or the typed rejection reason
    /// otherwise. Called before client publication and in tests.
    pub fn validate(&self) -> Result<(), CompletionRejection> {
        if !self.replacement_range.is_ordered() {
            return Err(CompletionRejection::InvalidReplacementRange {
                byte_start: self.replacement_range.byte_start,
                byte_end: self.replacement_range.byte_end,
            });
        }
        if self.items.len() > COMPLETION_RESULT_MAX_ITEMS {
            return Err(CompletionRejection::TooManyItems {
                item_count: self.items.len(),
                max_items: COMPLETION_RESULT_MAX_ITEMS,
            });
        }
        for item in &self.items {
            if item.label.chars().count() > COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS {
                return Err(CompletionRejection::ItemFieldTooLong {
                    field: CompletionItemField::Label,
                    length: item.label.chars().count(),
                    max_chars: COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
                });
            }
            if item.insert_text.chars().count() > COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS {
                return Err(CompletionRejection::ItemFieldTooLong {
                    field: CompletionItemField::InsertText,
                    length: item.insert_text.chars().count(),
                    max_chars: COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS,
                });
            }
            if item.detail.chars().count() > COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS {
                return Err(CompletionRejection::ItemFieldTooLong {
                    field: CompletionItemField::Detail,
                    length: item.detail.chars().count(),
                    max_chars: COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
                });
            }
            if item.commit_characters.chars().count() > COMPLETION_RESULT_MAX_ITEM_COMMIT_CHARS {
                return Err(CompletionRejection::ItemFieldTooLong {
                    field: CompletionItemField::CommitCharacters,
                    length: item.commit_characters.chars().count(),
                    max_chars: COMPLETION_RESULT_MAX_ITEM_COMMIT_CHARS,
                });
            }
        }
        Ok(())
    }
}

/// Validation failure for a [`CompletionRequest`] before it is dispatched to the
/// server-side provider lane. Distinct from [`CompletionRejection`]: request
/// validation runs before any provider work is scheduled.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionRequestRejection {
    /// The replacement range is not ordered (`byte_start > byte_end`).
    InvalidReplacementRange,
    /// The cursor offset lies before the replacement range start or after the
    /// replacement range end.
    CursorOutOfRange,
}

impl CompletionRequest {
    /// Returns `Ok(())` when the request is structurally valid before any
    /// provider work is scheduled, or the typed rejection reason otherwise.
    pub fn validate(&self) -> Result<(), CompletionRequestRejection> {
        if !self.replacement_range.is_ordered() {
            return Err(CompletionRequestRejection::InvalidReplacementRange);
        }
        if self.cursor_byte_offset < self.replacement_range.byte_start
            || self.cursor_byte_offset > self.replacement_range.byte_end
        {
            return Err(CompletionRequestRejection::CursorOutOfRange);
        }
        Ok(())
    }
}

/// Estimated lower bound on the encoded byte length of a result set, used to
/// reject oversized payloads before client publication without re-encoding.
/// Sums item string-field bytes plus a small fixed envelope allowance. The
/// true rkyv payload length is checked by the codec frame gate; this helper is
/// an earlier, allocation-free budget check.
pub fn estimated_result_payload_bytes(result: &CompletionResultSet) -> usize {
    const ENVELOPE_ALLOWANCE_BYTES: usize = 128;
    let items_bytes: usize = result
        .items
        .iter()
        .map(|item| item.string_field_bytes())
        .sum();
    ENVELOPE_ALLOWANCE_BYTES + items_bytes
}

/// Returns `Ok(())` when the result's estimated payload fits the result budget,
/// or the typed rejection reason otherwise. A pre-publication guard used in
/// addition to the codec frame gate.
pub fn check_result_payload_budget(
    result: &CompletionResultSet,
) -> Result<(), CompletionRejection> {
    let payload_bytes = estimated_result_payload_bytes(result);
    if payload_bytes > COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES {
        return Err(CompletionRejection::PayloadTooLarge {
            payload_bytes,
            budget_bytes: COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request(request_id: CompletionRequestId) -> CompletionRequest {
        CompletionRequest {
            request_id,
            client_id: 9,
            document_id: 7,
            document_version: 42,
            behavior_version: 3,
            cursor_byte_offset: 12,
            replacement_range: CompletionReplacementRange::new(10, 12),
            trigger: CompletionTrigger::Character(".".to_string()),
            provider_generation: 1,
        }
    }

    fn sample_result(
        request_id: CompletionRequestId,
        items: Vec<CompletionItem>,
    ) -> CompletionResultSet {
        CompletionResultSet {
            request_id,
            client_id: 9,
            document_id: 7,
            document_version: 42,
            behavior_version: 3,
            provider_generation: 1,
            replacement_range: CompletionReplacementRange::new(10, 12),
            status: CompletionStatus::Ok,
            items,
            provenance: CompletionProvenance::builtin_core(),
        }
    }

    #[test]
    fn valid_request_validates() {
        assert!(sample_request(1).validate().is_ok());
    }

    #[test]
    fn request_with_unordered_range_is_rejected() {
        let mut request = sample_request(2);
        request.replacement_range = CompletionReplacementRange::new(12, 10);
        assert_eq!(
            request.validate(),
            Err(CompletionRequestRejection::InvalidReplacementRange)
        );
    }

    #[test]
    fn request_with_cursor_out_of_range_is_rejected() {
        let mut request = sample_request(3);
        request.cursor_byte_offset = 5;
        assert_eq!(
            request.validate(),
            Err(CompletionRequestRejection::CursorOutOfRange)
        );
    }

    #[test]
    fn valid_result_validates() {
        let item = CompletionItem::new("foo", "foo", CompletionProvenance::builtin_core());
        let result = sample_result(1, vec![item]);
        assert!(result.validate().is_ok());
        assert!(check_result_payload_budget(&result).is_ok());
    }

    #[test]
    fn result_with_unordered_range_is_rejected() {
        let mut result = sample_result(2, Vec::new());
        result.replacement_range = CompletionReplacementRange::new(12, 10);
        assert!(matches!(
            result.validate(),
            Err(CompletionRejection::InvalidReplacementRange { .. })
        ));
    }

    #[test]
    fn result_with_too_many_items_is_rejected() {
        let prov = CompletionProvenance::builtin_core();
        let items: Vec<CompletionItem> = (0..(COMPLETION_RESULT_MAX_ITEMS + 1))
            .map(|i| CompletionItem::new(format!("item{i}"), format!("item{i}"), prov.clone()))
            .collect();
        let result = sample_result(3, items);
        assert!(matches!(
            result.validate(),
            Err(CompletionRejection::TooManyItems { .. })
        ));
    }

    #[test]
    fn result_with_oversized_label_is_rejected() {
        let mut item = CompletionItem::new("foo", "foo", CompletionProvenance::builtin_core());
        item.label = "x".repeat(COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS + 1);
        let result = sample_result(4, vec![item]);
        assert!(matches!(
            result.validate(),
            Err(CompletionRejection::ItemFieldTooLong {
                field: CompletionItemField::Label,
                ..
            })
        ));
    }

    #[test]
    fn result_with_oversized_payload_is_rejected_by_budget_check() {
        // Build an item set whose estimated payload exceeds the budget.
        let prov = CompletionProvenance::builtin_core();
        let chunk = "y".repeat(COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS);
        let items: Vec<CompletionItem> = (0..COMPLETION_RESULT_MAX_ITEMS)
            .map(|i| CompletionItem::new(format!("item{i}"), chunk.clone(), prov.clone()))
            .collect();
        let result = sample_result(5, items);
        let err = check_result_payload_budget(&result).unwrap_err();
        assert!(matches!(
            err,
            CompletionRejection::PayloadTooLarge {
                budget_bytes: COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
                ..
            }
        ));
    }

    #[test]
    fn completion_trigger_is_inert_metadata() {
        // The trigger variant carries an inert string; it is never executed.
        let trigger = CompletionTrigger::Character(".".to_string());
        let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&trigger).unwrap();
        let restored: CompletionTrigger =
            rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
        assert_eq!(restored, trigger);
        assert_eq!(restored, CompletionTrigger::Character(".".to_string()));
    }
}
