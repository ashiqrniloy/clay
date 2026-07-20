use std::{collections::HashMap, ops::Range};

use crop::Rope;

use crate::perf::{
    budgets::SYNTAX_CACHE_BUDGET_BYTES,
    metrics::{MetricMetadata, global_recorder},
};
use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentVersion, EditOperation, EditRejection, LeaseId,
    LockOwner, ParseByteRange, ParseInputEdit, ParsePoint, ParsePolicy, ParseWindowSnapshot,
    ProtocolErrorCode, RegionLockConflict, RegionLockId, ServerMessage, TransactionId,
};
use crate::server::locks::ranges_overlap;

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

#[derive(Debug, Clone, Copy)]
struct RetainedParseWindow {
    version: DocumentVersion,
    window_id: u64,
    byte_start: u64,
    byte_end: u64,
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
    retained_parse_windows: HashMap<(String, String), RetainedParseWindow>,
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
            retained_parse_windows: HashMap::new(),
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

        let access = self.access_for_client(client_id);
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
        self.apply_edit_with_parse_input(
            document_id,
            client_id,
            lease_id,
            base_version,
            transaction_id,
            operation,
        )
        .0
    }

    pub(crate) fn apply_edit_with_parse_input(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
        lease_id: Option<LeaseId>,
        base_version: DocumentVersion,
        transaction_id: TransactionId,
        operation: EditOperation,
    ) -> (ServerMessage, Option<ParseInputEdit>) {
        let recorder = global_recorder();
        let _scope = recorder.scope_with_metadata(
            "server.document.apply_edit",
            MetricMetadata::transaction(document_id, client_id, transaction_id, base_version),
        );
        if document_id != self.document_id {
            return (
                ServerMessage::EditRejected {
                    document_id,
                    transaction_id,
                    reason: EditRejection::InvalidDocument { document_id },
                },
                None,
            );
        }

        if base_version < self.version {
            return (
                ServerMessage::EditRejected {
                    document_id: self.document_id,
                    transaction_id,
                    reason: EditRejection::StaleVersion {
                        client_base_version: base_version,
                        server_version: self.version,
                    },
                },
                None,
            );
        }

        if base_version > self.version {
            return (
                ServerMessage::EditRejected {
                    document_id: self.document_id,
                    transaction_id,
                    reason: EditRejection::FutureVersion {
                        client_base_version: base_version,
                        server_version: self.version,
                    },
                },
                None,
            );
        }

        if let Err(reason) = self.validate_lease(client_id, lease_id) {
            return (
                ServerMessage::EditRejected {
                    document_id: self.document_id,
                    transaction_id,
                    reason,
                },
                None,
            );
        }

        let affected_range = match self.affected_range(&operation) {
            Ok(range) => range,
            Err(message) => {
                return (
                    ServerMessage::EditRejected {
                        document_id: self.document_id,
                        transaction_id,
                        reason: EditRejection::InvalidRange { message },
                    },
                    None,
                );
            }
        };

        if let Some(conflict) = self.region_lock_conflict(affected_range) {
            return (
                ServerMessage::EditRejected {
                    document_id: self.document_id,
                    transaction_id,
                    reason: EditRejection::RegionLocked { conflict },
                },
                None,
            );
        }

        let parse_input = match self.parse_input_edit(base_version, &operation) {
            Ok(edit) => edit,
            Err(message) => {
                return (
                    ServerMessage::EditRejected {
                        document_id: self.document_id,
                        transaction_id,
                        reason: EditRejection::InvalidRange { message },
                    },
                    None,
                );
            }
        };
        self.apply_operation(operation);
        self.version += 1;
        self.last_transaction_id = Some(transaction_id);
        self.dirty = true;
        recorder.record_counter("server.document.edit_ack", 1);
        (
            ServerMessage::EditAck {
                document_id: self.document_id,
                confirmed_version: self.version,
                transaction_id,
            },
            Some(parse_input),
        )
    }

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

    pub(crate) fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub(crate) fn version(&self) -> DocumentVersion {
        self.version
    }

    pub(crate) fn text(&self) -> String {
        self.text.to_string()
    }

    pub(crate) fn parse_window_snapshot(
        &self,
        package_prefix: &str,
        mode_id: &str,
        range: ParseByteRange,
        max_window_bytes: u64,
    ) -> Result<ParseWindowSnapshot, String> {
        self.validate_parse_snapshot_range(range, max_window_bytes)?;
        let start = usize::try_from(range.start)
            .map_err(|_| "parse snapshot start is too large".to_string())?;
        let end = usize::try_from(range.end)
            .map_err(|_| "parse snapshot end is too large".to_string())?;
        let base = self.parse_point(start);
        Ok(ParseWindowSnapshot {
            document_id: self.document_id,
            document_version: self.version,
            package_prefix: package_prefix.to_string(),
            mode_id: mode_id.to_string(),
            window_id: range.start,
            byte_start: range.start,
            byte_end: range.end,
            base_line: base.row,
            base_column: base.column,
            incremental_edit: false,
            text: self.text.byte_slice(start..end).to_string(),
        })
    }

    pub(crate) fn parse_window_after_edit(
        &mut self,
        package_prefix: &str,
        mode_id: &str,
        policy: ParsePolicy,
        edit: ParseInputEdit,
    ) -> Result<Option<ParseWindowSnapshot>, String> {
        if self.text.byte_len() == 0 {
            self.retained_parse_windows
                .remove(&(package_prefix.to_string(), mode_id.to_string()));
            return Ok(None);
        }
        if !edit.is_valid() || edit.document_version != self.version {
            return Err("parse input edit version or range is invalid".to_string());
        }

        if self.text.byte_len() as u64 <= policy.max_window_bytes {
            let range = ParseByteRange::new(0, self.text.byte_len() as u64);
            let mut snapshot = self.parse_window_snapshot(
                package_prefix,
                mode_id,
                range,
                policy.max_window_bytes,
            )?;
            snapshot.incremental_edit = true;
            self.retained_parse_windows.insert(
                (package_prefix.to_string(), mode_id.to_string()),
                RetainedParseWindow {
                    version: self.version,
                    window_id: 0,
                    byte_start: 0,
                    byte_end: self.text.byte_len() as u64,
                },
            );
            return Ok(Some(snapshot));
        }

        let key = (package_prefix.to_string(), mode_id.to_string());
        let retained = self.retained_parse_windows.get(&key).copied();
        let transformed_end = retained.and_then(|window| {
            if window.version != edit.base_document_version
                || edit.start_byte < window.byte_start
                || edit.old_end_byte > window.byte_end
            {
                return None;
            }
            shift_offset(
                window.byte_end,
                edit.new_end_byte as i128 - edit.old_end_byte as i128,
            )
            .filter(|end| {
                *end <= self.text.byte_len() as u64
                    && end.saturating_sub(window.byte_start) <= policy.max_window_bytes
            })
            .map(|end| (window, end))
        });

        let (window_id, start, end, incremental_edit) = if let Some((window, end)) = transformed_end
        {
            (window.window_id, window.byte_start, end, true)
        } else {
            let nominal_bytes = (policy.max_window_bytes / 2).max(1);
            let identity_offset = edit
                .start_byte
                .min((self.text.byte_len() as u64).saturating_sub(1));
            let anchor = identity_offset / nominal_bytes * nominal_bytes;
            let start = self.floor_char_boundary(anchor)?;
            let end = self.floor_char_boundary(
                start
                    .saturating_add(nominal_bytes)
                    .min(self.text.byte_len() as u64),
            )?;
            (start, start, end, false)
        };
        let range = ParseByteRange::new(start, end);
        let mut snapshot =
            self.parse_window_snapshot(package_prefix, mode_id, range, policy.max_window_bytes)?;
        snapshot.window_id = window_id;
        snapshot.incremental_edit = incremental_edit;
        self.retained_parse_windows.insert(
            key,
            RetainedParseWindow {
                version: self.version,
                window_id,
                byte_start: start,
                byte_end: end,
            },
        );
        Ok(Some(snapshot))
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "parse-window snapshots are consumed by large-file parser integration in a follow-up phase"
        )
    )]
    pub(crate) fn parse_window_snapshots(
        &self,
        package_prefix: &str,
        mode_id: &str,
        viewport: ParseByteRange,
        invalidated_ranges: &[ParseByteRange],
        policy: ParsePolicy,
    ) -> Result<Vec<ParseWindowSnapshot>, String> {
        if policy.max_window_bytes == 0
            || policy.memory_budget_bytes == 0
            || policy.memory_budget_bytes > SYNTAX_CACHE_BUDGET_BYTES as u64
        {
            return Err("parse policy exceeds supported bounds".to_string());
        }

        let mut ranges = Vec::with_capacity(invalidated_ranges.len().saturating_add(1));
        ranges.push(viewport);
        ranges.extend_from_slice(invalidated_ranges);
        ranges.sort_by(|left, right| {
            let left_visible = left.intersects(viewport);
            let right_visible = right.intersects(viewport);
            right_visible
                .cmp(&left_visible)
                .then_with(|| left.start.cmp(&right.start))
                .then_with(|| left.end.cmp(&right.end))
        });

        let mut snapshots = Vec::with_capacity(ranges.len());
        let mut retained_bytes = 0u64;
        for range in ranges {
            let window = self.expand_parse_window(range, policy)?;
            retained_bytes = retained_bytes.saturating_add(window.len());
            if retained_bytes > policy.memory_budget_bytes {
                return Err("parse window snapshots exceed memory budget".to_string());
            }
            snapshots.push(self.parse_window_snapshot(
                package_prefix,
                mode_id,
                window,
                policy.max_window_bytes,
            )?);
        }
        Ok(snapshots)
    }

    pub(crate) fn mark_clean_if_version(&mut self, version: DocumentVersion) -> bool {
        if self.version == version {
            self.dirty = false;
            true
        } else {
            false
        }
    }

    pub(crate) fn replace_text_from_storage(&mut self, text: String) {
        if self.text != text {
            self.text = Rope::from(text);
            self.version = self.version.saturating_add(1);
            self.retained_parse_windows.clear();
        }
        self.dirty = false;
    }

    fn parse_input_edit(
        &self,
        base_document_version: DocumentVersion,
        operation: &EditOperation,
    ) -> Result<ParseInputEdit, String> {
        let (start_byte, old_end_byte, inserted_text) = match operation {
            EditOperation::Insert { byte_offset, text } => {
                (*byte_offset, *byte_offset, text.as_str())
            }
            EditOperation::Delete { start, end } => (*start, *end, ""),
            EditOperation::Replace { start, end, text } => (*start, *end, text.as_str()),
        };
        let start = self.validate_boundary(start_byte)?;
        let old_end = self.validate_boundary(old_end_byte)?;
        let inserted_bytes = u64::try_from(inserted_text.len())
            .map_err(|_| "inserted text length is too large".to_string())?;
        let new_end_byte = start_byte
            .checked_add(inserted_bytes)
            .ok_or_else(|| "inserted text end is too large".to_string())?;
        let start_position = self.parse_point(start);
        let document_version = base_document_version
            .checked_add(1)
            .ok_or_else(|| "document version is too large".to_string())?;
        Ok(ParseInputEdit {
            base_document_version,
            document_version,
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position: self.parse_point(old_end),
            new_end_position: point_after_text(start_position, inserted_text),
        })
    }

    fn parse_point(&self, byte_offset: usize) -> ParsePoint {
        let row = self.text.line_of_byte(byte_offset);
        ParsePoint::new(
            row as u64,
            (byte_offset - self.text.byte_of_line(row)) as u64,
        )
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

    pub(crate) fn access_for_client(&self, client_id: ClientId) -> DocumentAccess {
        match (self.active_lease, Some(client_id)) {
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

    fn validate_parse_snapshot_range(
        &self,
        range: ParseByteRange,
        max_window_bytes: u64,
    ) -> Result<(), String> {
        if max_window_bytes == 0 {
            return Err("parse snapshot max window must be non-zero".to_string());
        }
        self.validate_range(range.start, range.end)?;
        if range.len() > max_window_bytes {
            return Err(format!(
                "parse snapshot range length {} exceeds max window {max_window_bytes}",
                range.len()
            ));
        }
        Ok(())
    }

    fn expand_parse_window(
        &self,
        range: ParseByteRange,
        policy: ParsePolicy,
    ) -> Result<ParseByteRange, String> {
        self.validate_parse_snapshot_range(range, policy.max_window_bytes)?;
        let original_len = range.len();
        let guard_budget = policy.max_window_bytes.saturating_sub(original_len);
        let before = policy.guard_bytes.min(guard_budget / 2);
        let after = policy.guard_bytes.min(guard_budget.saturating_sub(before));
        let start = self.floor_char_boundary(range.start.saturating_sub(before))?;
        let end = self.ceil_char_boundary(range.end.saturating_add(after))?;
        if end.saturating_sub(start) <= policy.max_window_bytes {
            return Ok(ParseByteRange::new(start, end));
        }

        let capped_end = self.floor_char_boundary(start.saturating_add(policy.max_window_bytes))?;
        if capped_end < range.end {
            return Err("parse snapshot range cannot fit inside max window".to_string());
        }
        Ok(ParseByteRange::new(start, capped_end))
    }

    fn floor_char_boundary(&self, offset: u64) -> Result<u64, String> {
        let mut offset = offset.min(self.text.byte_len() as u64);
        while offset > 0 {
            let candidate = usize::try_from(offset)
                .map_err(|_| "parse snapshot offset is too large".to_string())?;
            if self.text.is_char_boundary(candidate) {
                return Ok(offset);
            }
            offset -= 1;
        }
        Ok(0)
    }

    fn ceil_char_boundary(&self, offset: u64) -> Result<u64, String> {
        let text_len = self.text.byte_len() as u64;
        let mut offset = offset.min(text_len);
        while offset < text_len {
            let candidate = usize::try_from(offset)
                .map_err(|_| "parse snapshot offset is too large".to_string())?;
            if self.text.is_char_boundary(candidate) {
                return Ok(offset);
            }
            offset += 1;
        }
        Ok(text_len)
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

fn point_after_text(start: ParsePoint, text: &str) -> ParsePoint {
    match text.rsplit_once('\n') {
        Some((before_last_newline, trailing)) => ParsePoint::new(
            start.row
                + before_last_newline
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count() as u64
                + 1,
            trailing.len() as u64,
        ),
        None => ParsePoint::new(start.row, start.column + text.len() as u64),
    }
}

fn shift_offset(offset: u64, delta: i128) -> Option<u64> {
    u64::try_from(offset as i128 + delta).ok()
}

impl RegionLock {
    fn overlaps(&self, affected_range: AffectedRange) -> bool {
        match affected_range {
            AffectedRange::Insert { offset } => offset >= self.start && offset < self.end,
            AffectedRange::Span { start, end } => ranges_overlap(start, end, self.start, self.end),
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
        DocumentAccess, EditOperation, EditRejection, LockOwner, ParseByteRange, ParseInputEdit,
        ParsePoint, ParsePolicy, RegionLockConflict, ServerMessage,
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
    fn accepted_edits_record_exact_utf8_and_newline_coordinates() {
        for (operation, expected) in [
            (
                EditOperation::Insert {
                    byte_offset: 4,
                    text: "β\nQ".to_string(),
                },
                ParseInputEdit {
                    base_document_version: 1,
                    document_version: 2,
                    start_byte: 4,
                    old_end_byte: 4,
                    new_end_byte: 8,
                    start_position: ParsePoint::new(1, 0),
                    old_end_position: ParsePoint::new(1, 0),
                    new_end_position: ParsePoint::new(2, 1),
                },
            ),
            (
                EditOperation::Delete { start: 1, end: 4 },
                ParseInputEdit {
                    base_document_version: 1,
                    document_version: 2,
                    start_byte: 1,
                    old_end_byte: 4,
                    new_end_byte: 1,
                    start_position: ParsePoint::new(0, 1),
                    old_end_position: ParsePoint::new(1, 0),
                    new_end_position: ParsePoint::new(0, 1),
                },
            ),
            (
                EditOperation::Replace {
                    start: 4,
                    end: 7,
                    text: "ok\n".to_string(),
                },
                ParseInputEdit {
                    base_document_version: 1,
                    document_version: 2,
                    start_byte: 4,
                    old_end_byte: 7,
                    new_end_byte: 7,
                    start_position: ParsePoint::new(1, 0),
                    old_end_position: ParsePoint::new(1, 3),
                    new_end_position: ParsePoint::new(2, 0),
                },
            ),
        ] {
            let mut document = DocumentState::new(
                7,
                "aé\nxyz".to_string(),
                DocumentAccess::Editable { lease_id: 1 },
            );
            document.acquire_access(0);
            let (response, parse_input) =
                document.apply_edit_with_parse_input(7, 0, Some(1), 1, 12, operation);

            assert!(matches!(response, ServerMessage::EditAck { .. }));
            assert_eq!(parse_input, Some(expected));
        }
    }

    #[test]
    fn adjacent_edits_retain_window_identity_and_crossing_edit_falls_back() {
        let mut document = DocumentState::new(
            7,
            "a".repeat(10_000),
            DocumentAccess::Editable { lease_id: 1 },
        );
        document.acquire_access(0);
        let policy = ParsePolicy::new(4_096, 0, 30 * 1024 * 1024, 50);

        let (_, first_edit) = document.apply_edit_with_parse_input(
            7,
            0,
            Some(1),
            1,
            12,
            EditOperation::Insert {
                byte_offset: 3_000,
                text: "x".to_string(),
            },
        );
        let first = document
            .parse_window_after_edit("rust", "rust.rust", policy, first_edit.unwrap())
            .unwrap()
            .unwrap();
        let (_, second_edit) = document.apply_edit_with_parse_input(
            7,
            0,
            Some(1),
            2,
            13,
            EditOperation::Insert {
                byte_offset: 3_001,
                text: "y".to_string(),
            },
        );
        let second = document
            .parse_window_after_edit("rust", "rust.rust", policy, second_edit.unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(second.window_id, first.window_id);
        assert_eq!(second.byte_start, first.byte_start);
        assert!(second.incremental_edit);
        assert!(second.text_len_bytes() <= policy.max_window_bytes as usize);

        let (_, crossing_edit) = document.apply_edit_with_parse_input(
            7,
            0,
            Some(1),
            3,
            14,
            EditOperation::Delete {
                start: 4_000,
                end: 5_000,
            },
        );
        let fallback = document
            .parse_window_after_edit("rust", "rust.rust", policy, crossing_edit.unwrap())
            .unwrap()
            .unwrap();
        assert!(!fallback.incremental_edit);
        assert!(fallback.text_len_bytes() <= policy.max_window_bytes as usize);
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
    fn parse_window_snapshot_slices_only_requested_server_range() {
        let document = DocumentState::new(
            7,
            format!("{}VISIBLE{}", "a".repeat(8192), "b".repeat(8192)),
            DocumentAccess::Editable { lease_id: 1 },
        );

        let snapshot = document
            .parse_window_snapshot("plain", "plain", ParseByteRange::new(8192, 8199), 1024)
            .unwrap();

        assert_eq!(snapshot.document_id, 7);
        assert_eq!(snapshot.document_version, 1);
        assert_eq!(snapshot.package_prefix, "plain");
        assert_eq!(snapshot.mode_id, "plain");
        assert_eq!(snapshot.text, "VISIBLE");
        assert!(snapshot.text.len() < document.text.byte_len() / 1000);
    }

    #[test]
    fn parse_window_snapshots_validate_utf8_boundaries_and_memory_budget() {
        let document = DocumentState::new(
            7,
            "alpha\néclair\nomega".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let policy = ParsePolicy::new(16, 2, 32, 50);

        let snapshots = document
            .parse_window_snapshots(
                "plain",
                "plain",
                ParseByteRange::new(0, 5),
                &[ParseByteRange::new(8, 13)],
                policy,
            )
            .unwrap();

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].byte_start, 0);
        assert!(snapshots[1].text.contains("clair"));
        assert!(
            document
                .parse_window_snapshot("plain", "plain", ParseByteRange::new(7, 13), 16)
                .is_err()
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
