use std::ops::Range;

use crop::Rope;

use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentVersion, EditOperation, EditRejection, LeaseId,
    LockOwner, ProtocolErrorCode, RegionLockConflict, RegionLockId, ServerMessage, TransactionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditableLease {
    client_id: ClientId,
    lease_id: LeaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegionLock {
    lock_id: RegionLockId,
    start: u64,
    end: u64,
    owner: LockOwner,
    created_at_version: DocumentVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AffectedRange {
    Insert { offset: u64 },
    Span { start: u64, end: u64 },
}

#[derive(Debug)]
pub(crate) struct DocumentState {
    document_id: DocumentId,
    version: DocumentVersion,
    text: Rope,
    active_lease: Option<EditableLease>,
    next_lease_id: LeaseId,
    last_transaction_id: Option<TransactionId>,
    dirty: bool,
    region_locks: Vec<RegionLock>,
    next_region_lock_id: RegionLockId,
}

impl DocumentState {
    pub(crate) fn new(document_id: DocumentId, text: String, _access: DocumentAccess) -> Self {
        Self {
            document_id,
            version: 1,
            text: Rope::from(text),
            active_lease: None,
            next_lease_id: 1,
            last_transaction_id: None,
            dirty: false,
            region_locks: Vec::new(),
            next_region_lock_id: 1,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "internal region lock registration awaits a future server/AI lock-management caller"
        )
    )]
    pub(crate) fn register_region_lock(
        &mut self,
        start: u64,
        end: u64,
        owner: LockOwner,
    ) -> Result<RegionLockId, String> {
        self.validate_lock_range(start, end)?;
        let lock_id = self.next_region_lock_id;
        self.next_region_lock_id = self.next_region_lock_id.saturating_add(1);
        self.region_locks.push(RegionLock {
            lock_id,
            start,
            end,
            owner,
            created_at_version: self.version,
        });
        Ok(lock_id)
    }

    pub(crate) fn acquire_access(&mut self, client_id: ClientId) -> DocumentAccess {
        match self.active_lease {
            Some(lease) if lease.client_id == client_id => DocumentAccess::Editable {
                lease_id: lease.lease_id,
            },
            Some(_) => DocumentAccess::ReadOnly,
            None => {
                let lease_id = self.next_lease_id;
                self.next_lease_id = self.next_lease_id.saturating_add(1);
                self.active_lease = Some(EditableLease {
                    client_id,
                    lease_id,
                });
                DocumentAccess::Editable { lease_id }
            }
        }
    }

    pub(crate) fn release_access(&mut self, client_id: ClientId) {
        if self
            .active_lease
            .is_some_and(|lease| lease.client_id == client_id)
        {
            self.active_lease = None;
        }
    }

    pub(crate) fn initial_document_message(&self, access: DocumentAccess) -> ServerMessage {
        let (document_id, version, text, lease_id) = self.snapshot_parts(&access);
        ServerMessage::InitialDocument {
            document_id,
            version,
            text,
            access,
            lease_id,
        }
    }

    pub(crate) fn resync_snapshot_message_for_client(
        &self,
        document_id: DocumentId,
        client_id: ClientId,
    ) -> ServerMessage {
        if document_id != self.document_id {
            return ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!("unknown document id {document_id}"),
            };
        }

        let access = self.access_for_client(Some(client_id));
        let (document_id, version, text, lease_id) = self.snapshot_parts(&access);
        ServerMessage::ResyncSnapshot {
            document_id,
            version,
            text,
            access,
            lease_id,
        }
    }

    pub(crate) fn apply_edit(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
        lease_id: Option<LeaseId>,
        base_version: DocumentVersion,
        transaction_id: TransactionId,
        operation: EditOperation,
    ) -> ServerMessage {
        if document_id != self.document_id {
            return ServerMessage::EditRejected {
                document_id,
                transaction_id,
                reason: EditRejection::InvalidDocument { document_id },
            };
        }

        if base_version < self.version {
            return ServerMessage::EditRejected {
                document_id: self.document_id,
                transaction_id,
                reason: EditRejection::StaleVersion {
                    client_base_version: base_version,
                    server_version: self.version,
                },
            };
        }

        if base_version > self.version {
            return ServerMessage::EditRejected {
                document_id: self.document_id,
                transaction_id,
                reason: EditRejection::FutureVersion {
                    client_base_version: base_version,
                    server_version: self.version,
                },
            };
        }

        if let Err(reason) = self.validate_lease(client_id, lease_id) {
            return ServerMessage::EditRejected {
                document_id: self.document_id,
                transaction_id,
                reason,
            };
        }

        let affected_range = match self.affected_range(&operation) {
            Ok(range) => range,
            Err(message) => {
                return ServerMessage::EditRejected {
                    document_id: self.document_id,
                    transaction_id,
                    reason: EditRejection::InvalidRange { message },
                };
            }
        };

        if let Some(conflict) = self.region_lock_conflict(affected_range) {
            return ServerMessage::EditRejected {
                document_id: self.document_id,
                transaction_id,
                reason: EditRejection::RegionLocked { conflict },
            };
        }

        self.apply_operation(operation);
        self.version += 1;
        self.last_transaction_id = Some(transaction_id);
        self.dirty = true;
        ServerMessage::EditAck {
            document_id: self.document_id,
            confirmed_version: self.version,
            transaction_id,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "dirty-state access is consumed by Phase 9 workspace save/reload integration"
        )
    )]
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "clean marking is consumed by Phase 9 workspace save/reload integration"
        )
    )]
    pub(crate) fn mark_clean(&mut self) {
        self.dirty = false;
    }

    fn apply_operation(&mut self, operation: EditOperation) {
        match operation {
            EditOperation::Insert { byte_offset, text } => {
                let offset = self
                    .validate_boundary(byte_offset)
                    .expect("edit boundary was validated before mutation");
                self.text.insert(offset, text);
            }
            EditOperation::Delete { start, end } => {
                let range = self
                    .validate_range(start, end)
                    .expect("edit range was validated before mutation");
                self.text.delete(range);
            }
            EditOperation::Replace { start, end, text } => {
                let range = self
                    .validate_range(start, end)
                    .expect("edit range was validated before mutation");
                self.text.replace(range, text);
            }
        }
    }

    fn affected_range(&self, operation: &EditOperation) -> Result<AffectedRange, String> {
        match operation {
            EditOperation::Insert { byte_offset, .. } => {
                self.validate_boundary(*byte_offset)?;
                Ok(AffectedRange::Insert {
                    offset: *byte_offset,
                })
            }
            EditOperation::Delete { start, end } => {
                self.validate_range(*start, *end)?;
                Ok(AffectedRange::Span {
                    start: *start,
                    end: *end,
                })
            }
            EditOperation::Replace { start, end, .. } if start == end => {
                self.validate_boundary(*start)?;
                Ok(AffectedRange::Insert { offset: *start })
            }
            EditOperation::Replace { start, end, .. } => {
                self.validate_range(*start, *end)?;
                Ok(AffectedRange::Span {
                    start: *start,
                    end: *end,
                })
            }
        }
    }

    fn region_lock_conflict(&self, affected_range: AffectedRange) -> Option<RegionLockConflict> {
        self.region_locks
            .iter()
            .find(|lock| lock.overlaps(affected_range))
            .map(RegionLock::conflict)
    }

    fn access_for_client(&self, client_id: Option<ClientId>) -> DocumentAccess {
        match (self.active_lease, client_id) {
            (Some(lease), Some(client_id)) if lease.client_id == client_id => {
                DocumentAccess::Editable {
                    lease_id: lease.lease_id,
                }
            }
            _ => DocumentAccess::ReadOnly,
        }
    }

    fn validate_lease(
        &self,
        client_id: ClientId,
        lease_id: Option<LeaseId>,
    ) -> Result<(), EditRejection> {
        let Some(active) = self.active_lease else {
            return Err(EditRejection::LeaseRequired);
        };
        let Some(lease_id) = lease_id else {
            return Err(EditRejection::LeaseRequired);
        };
        if active.client_id != client_id || active.lease_id != lease_id {
            return Err(EditRejection::LeaseExpired { lease_id });
        }
        Ok(())
    }

    fn snapshot_parts(
        &self,
        access: &DocumentAccess,
    ) -> (DocumentId, DocumentVersion, String, Option<u64>) {
        (
            self.document_id,
            self.version,
            self.text.to_string(),
            access.lease_id(),
        )
    }

    fn validate_range(&self, start: u64, end: u64) -> Result<Range<usize>, String> {
        let start = self.validate_boundary(start)?;
        let end = self.validate_boundary(end)?;
        if start > end {
            return Err("edit range start is after range end".to_string());
        }
        Ok(start..end)
    }

    fn validate_lock_range(&self, start: u64, end: u64) -> Result<(), String> {
        self.validate_range(start, end)?;
        if start == end {
            return Err("region lock range must not be empty".to_string());
        }
        Ok(())
    }

    fn validate_boundary(&self, offset: u64) -> Result<usize, String> {
        let offset = usize::try_from(offset).map_err(|_| "edit offset is too large".to_string())?;
        let text_len = self.text.byte_len();
        if offset > text_len {
            return Err(format!(
                "edit offset {offset} is past document length {text_len}"
            ));
        }
        if !self.text.is_char_boundary(offset) {
            return Err(format!("edit offset {offset} is not a UTF-8 boundary"));
        }
        Ok(offset)
    }
}

impl RegionLock {
    fn overlaps(&self, affected_range: AffectedRange) -> bool {
        match affected_range {
            AffectedRange::Insert { offset } => offset >= self.start && offset < self.end,
            AffectedRange::Span { start, end } => start < self.end && end > self.start,
        }
    }

    fn conflict(&self) -> RegionLockConflict {
        RegionLockConflict {
            lock_id: self.lock_id,
            start: self.start,
            end: self.end,
            owner: self.owner.clone(),
            created_at_version: self.created_at_version,
        }
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new(
            1,
            "Welcome to Clay's Phase 4 IPC server.\n".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentState;
    use crate::protocol::{
        DocumentAccess, EditOperation, EditRejection, LockOwner, RegionLockConflict, ServerMessage,
    };

    #[test]
    fn server_document_uses_rope_for_insert_delete_replace() {
        let mut document = DocumentState::new(
            7,
            "Hello 🌎".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);

        assert_eq!(
            document.apply_edit(
                7,
                0,
                Some(1),
                1,
                12,
                EditOperation::Insert {
                    byte_offset: 6,
                    text: "Clay ".to_string(),
                },
            ),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 12,
            }
        );
        assert_eq!(document.text.to_string(), "Hello Clay 🌎");

        assert_eq!(
            document.apply_edit(
                7,
                0,
                Some(1),
                2,
                13,
                EditOperation::Replace {
                    start: 0,
                    end: 5,
                    text: "Hi".to_string(),
                },
            ),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 3,
                transaction_id: 13,
            }
        );
        assert_eq!(document.text.to_string(), "Hi Clay 🌎");

        assert_eq!(
            document.apply_edit(
                7,
                0,
                Some(1),
                3,
                14,
                EditOperation::Delete { start: 2, end: 3 }
            ),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 4,
                transaction_id: 14,
            }
        );
        assert_eq!(document.text.to_string(), "HiClay 🌎");
    }

    #[test]
    fn server_document_rejects_non_boundary_rope_edit_without_panic() {
        let mut document =
            DocumentState::new(7, "é".to_string(), DocumentAccess::Editable { lease_id: 1 });
        document.acquire_access(0);

        let response = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 1,
                text: "x".to_string(),
            },
        );

        assert!(matches!(
            response,
            ServerMessage::EditRejected {
                reason: EditRejection::InvalidRange { .. },
                ..
            }
        ));
        assert_eq!(document.text.to_string(), "é");
        assert_eq!(document.version, 1);
    }

    #[test]
    fn server_document_rejects_out_of_range_rope_edit() {
        let mut document = DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);

        let response = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Delete { start: 0, end: 3 },
        );

        assert!(matches!(
            response,
            ServerMessage::EditRejected {
                reason: EditRejection::InvalidRange { .. },
                ..
            }
        ));
        assert_eq!(document.text.to_string(), "Hi");
        assert_eq!(document.version, 1);
    }

    #[test]
    fn server_document_snapshot_preserves_unicode() {
        let document = DocumentState::new(
            7,
            "Hi 🪐\n再見".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );

        assert_eq!(
            document.initial_document_message(DocumentAccess::Editable { lease_id: 1 }),
            ServerMessage::InitialDocument {
                document_id: 7,
                version: 1,
                text: "Hi 🪐\n再見".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            }
        );
    }

    #[test]
    fn server_accepts_edit_at_current_base_version() {
        let mut document = DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);

        let response = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 2,
                text: "!".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 12,
            }
        );
        assert_eq!(document.text.to_string(), "Hi!");
    }

    #[test]
    fn server_rejects_stale_base_version() {
        let mut document = DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);
        let accepted = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 2,
                text: "!".to_string(),
            },
        );
        assert!(matches!(accepted, ServerMessage::EditAck { .. }));

        let response = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            13,
            EditOperation::Insert {
                byte_offset: 3,
                text: "?".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 13,
                reason: EditRejection::StaleVersion {
                    client_base_version: 1,
                    server_version: 2,
                },
            }
        );
        assert_eq!(document.text.to_string(), "Hi!");
        assert_eq!(document.version, 2);
    }

    #[test]
    fn server_rejects_future_base_version() {
        let mut document = DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);

        let response = document.apply_edit(
            7,
            0,
            Some(1),
            2,
            12,
            EditOperation::Insert {
                byte_offset: 2,
                text: "!".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 12,
                reason: EditRejection::FutureVersion {
                    client_base_version: 2,
                    server_version: 1,
                },
            }
        );
        assert_eq!(document.text.to_string(), "Hi");
        assert_eq!(document.version, 1);
    }

    #[test]
    fn first_client_receives_editable_lease() {
        let mut document = DocumentState::new(7, "Hi".to_string(), DocumentAccess::ReadOnly);

        assert_eq!(
            document.acquire_access(42),
            DocumentAccess::Editable { lease_id: 1 }
        );
    }

    #[test]
    fn second_client_receives_read_only_access() {
        let mut document = DocumentState::new(7, "Hi".to_string(), DocumentAccess::ReadOnly);

        assert_eq!(
            document.acquire_access(1),
            DocumentAccess::Editable { lease_id: 1 }
        );
        assert_eq!(document.acquire_access(2), DocumentAccess::ReadOnly);
    }

    #[test]
    fn server_rejects_edit_without_current_lease() {
        let mut document = DocumentState::new(7, "Hi".to_string(), DocumentAccess::ReadOnly);
        document.acquire_access(1);

        let missing = document.apply_edit(
            7,
            1,
            None,
            1,
            12,
            EditOperation::Insert {
                byte_offset: 2,
                text: "!".to_string(),
            },
        );
        assert!(matches!(
            missing,
            ServerMessage::EditRejected {
                reason: EditRejection::LeaseRequired,
                ..
            }
        ));

        let wrong = document.apply_edit(
            7,
            2,
            Some(1),
            1,
            13,
            EditOperation::Insert {
                byte_offset: 2,
                text: "?".to_string(),
            },
        );
        assert_eq!(
            wrong,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 13,
                reason: EditRejection::LeaseExpired { lease_id: 1 },
            }
        );
        assert_eq!(document.text.to_string(), "Hi");
    }

    #[test]
    fn lease_released_or_retained_on_disconnect_matches_policy() {
        let mut document = DocumentState::new(7, "Hi".to_string(), DocumentAccess::ReadOnly);
        assert_eq!(
            document.acquire_access(1),
            DocumentAccess::Editable { lease_id: 1 }
        );
        document.release_access(2);
        assert_eq!(document.acquire_access(2), DocumentAccess::ReadOnly);
        document.release_access(1);
        assert_eq!(
            document.acquire_access(2),
            DocumentAccess::Editable { lease_id: 2 }
        );
    }

    #[test]
    fn server_document_version_advances_once_per_accepted_edit() {
        let mut document = DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);

        let rejected = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Delete { start: 9, end: 10 },
        );
        assert!(matches!(rejected, ServerMessage::EditRejected { .. }));
        assert_eq!(document.version, 1);

        let accepted = document.apply_edit(
            7,
            0,
            Some(1),
            1,
            13,
            EditOperation::Insert {
                byte_offset: 2,
                text: " Clay".to_string(),
            },
        );
        assert_eq!(
            accepted,
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 13,
            }
        );
        assert_eq!(document.version, 2);
        assert_eq!(document.last_transaction_id, Some(13));
    }

    #[test]
    fn server_rejects_insert_inside_region_lock() {
        let mut document = DocumentState::new(7, "abcdef".to_string(), DocumentAccess::ReadOnly);
        document.acquire_access(1);
        document
            .register_region_lock(2, 4, LockOwner::Server)
            .unwrap();

        let response = document.apply_edit(
            7,
            1,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 3,
                text: "X".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 12,
                reason: EditRejection::RegionLocked {
                    conflict: RegionLockConflict {
                        lock_id: 1,
                        start: 2,
                        end: 4,
                        owner: LockOwner::Server,
                        created_at_version: 1,
                    },
                },
            }
        );
        assert_eq!(document.text.to_string(), "abcdef");
        assert_eq!(document.version, 1);

        let replace_shaped_insert = document.apply_edit(
            7,
            1,
            Some(1),
            1,
            13,
            EditOperation::Replace {
                start: 2,
                end: 2,
                text: "Y".to_string(),
            },
        );
        assert!(matches!(
            replace_shaped_insert,
            ServerMessage::EditRejected {
                reason: EditRejection::RegionLocked { .. },
                ..
            }
        ));
        assert_eq!(document.text.to_string(), "abcdef");
        assert_eq!(document.version, 1);
    }

    #[test]
    fn server_rejects_delete_overlapping_region_lock() {
        let mut document = DocumentState::new(7, "abcdef".to_string(), DocumentAccess::ReadOnly);
        document.acquire_access(1);
        document
            .register_region_lock(2, 4, LockOwner::Server)
            .unwrap();

        let response = document.apply_edit(
            7,
            1,
            Some(1),
            1,
            12,
            EditOperation::Delete { start: 1, end: 3 },
        );

        assert!(matches!(
            response,
            ServerMessage::EditRejected {
                reason: EditRejection::RegionLocked { .. },
                ..
            }
        ));
        assert_eq!(document.text.to_string(), "abcdef");
        assert_eq!(document.version, 1);
    }

    #[test]
    fn server_accepts_edit_outside_region_lock() {
        let mut document = DocumentState::new(7, "abcdef".to_string(), DocumentAccess::ReadOnly);
        document.acquire_access(1);
        document
            .register_region_lock(2, 4, LockOwner::Server)
            .unwrap();

        let response = document.apply_edit(
            7,
            1,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 4,
                text: "X".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 12,
            }
        );
        assert_eq!(document.text.to_string(), "abcdXef");
    }

    #[test]
    fn region_lock_range_validation_rejects_invalid_boundaries() {
        let mut document = DocumentState::new(7, "éx".to_string(), DocumentAccess::ReadOnly);

        assert!(
            document
                .register_region_lock(2, 1, LockOwner::Server)
                .is_err()
        );
        assert!(
            document
                .register_region_lock(0, 0, LockOwner::Server)
                .is_err()
        );
        assert!(
            document
                .register_region_lock(1, 2, LockOwner::Server)
                .is_err()
        );
        assert!(
            document
                .register_region_lock(0, 9, LockOwner::Server)
                .is_err()
        );
        assert!(document.region_locks.is_empty());
    }

    #[test]
    fn region_lock_conflict_reports_range_metadata() {
        let mut document = DocumentState::new(7, "abcdef".to_string(), DocumentAccess::ReadOnly);
        document.acquire_access(1);
        document
            .register_region_lock(1, 5, LockOwner::Client { client_id: 99 })
            .unwrap();

        let response = document.apply_edit(
            7,
            1,
            Some(1),
            1,
            12,
            EditOperation::Replace {
                start: 4,
                end: 6,
                text: "yz".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 12,
                reason: EditRejection::RegionLocked {
                    conflict: RegionLockConflict {
                        lock_id: 1,
                        start: 1,
                        end: 5,
                        owner: LockOwner::Client { client_id: 99 },
                        created_at_version: 1,
                    },
                },
            }
        );
    }
}
