use crate::protocol::{
    DocumentAccess, DocumentId, DocumentVersion, EditOperation, ProtocolErrorCode, ServerMessage,
    TransactionId,
};

#[derive(Debug)]
pub(crate) struct DocumentState {
    document_id: DocumentId,
    version: DocumentVersion,
    text: String,
    access: DocumentAccess,
}

impl DocumentState {
    pub(crate) fn new(document_id: DocumentId, text: String, access: DocumentAccess) -> Self {
        Self {
            document_id,
            version: 1,
            text,
            access,
        }
    }

    pub(crate) fn initial_document_message(&self) -> ServerMessage {
        ServerMessage::InitialDocument {
            document_id: self.document_id,
            version: self.version,
            text: self.text.clone(),
            access: self.access.clone(),
        }
    }

    pub(crate) fn apply_edit(
        &mut self,
        document_id: DocumentId,
        transaction_id: TransactionId,
        operation: EditOperation,
    ) -> ServerMessage {
        if document_id != self.document_id {
            return ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!("unknown document id {document_id}"),
            };
        }

        if self.access != DocumentAccess::Editable {
            return ServerMessage::Error {
                code: ProtocolErrorCode::AccessDenied,
                message: "document is read-only".to_string(),
            };
        }

        match self.apply_operation(operation) {
            Ok(()) => {
                self.version += 1;
                ServerMessage::EditAck {
                    document_id: self.document_id,
                    version: self.version,
                    transaction_id,
                }
            }
            Err(message) => ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message,
            },
        }
    }

    fn apply_operation(&mut self, operation: EditOperation) -> Result<(), String> {
        match operation {
            EditOperation::Insert { byte_offset, text } => {
                let offset = self.validate_boundary(byte_offset)?;
                self.text.insert_str(offset, &text);
                Ok(())
            }
            EditOperation::Delete { start, end } => {
                let range = self.validate_range(start, end)?;
                self.text.replace_range(range, "");
                Ok(())
            }
            EditOperation::Replace { start, end, text } => {
                let range = self.validate_range(start, end)?;
                self.text.replace_range(range, &text);
                Ok(())
            }
        }
    }

    fn validate_range(&self, start: u64, end: u64) -> Result<std::ops::Range<usize>, String> {
        let start = self.validate_boundary(start)?;
        let end = self.validate_boundary(end)?;
        if start > end {
            return Err("edit range start is after range end".to_string());
        }
        Ok(start..end)
    }

    fn validate_boundary(&self, offset: u64) -> Result<usize, String> {
        let offset = usize::try_from(offset).map_err(|_| "edit offset is too large".to_string())?;
        if offset > self.text.len() {
            return Err(format!(
                "edit offset {offset} is past document length {}",
                self.text.len()
            ));
        }
        if !self.text.is_char_boundary(offset) {
            return Err(format!("edit offset {offset} is not a UTF-8 boundary"));
        }
        Ok(offset)
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new(
            1,
            "Welcome to Clay's Phase 4 IPC server.\n".to_string(),
            DocumentAccess::Editable,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentState;
    use crate::protocol::{DocumentAccess, EditOperation, ProtocolErrorCode, ServerMessage};

    #[test]
    fn document_state_applies_insert_and_acknowledges_version() {
        let mut document = DocumentState::new(7, "Hi".to_string(), DocumentAccess::Editable);

        let response = document.apply_edit(
            7,
            12,
            EditOperation::Insert {
                byte_offset: 2,
                text: " Clay".to_string(),
            },
        );

        assert_eq!(
            response,
            ServerMessage::EditAck {
                document_id: 7,
                version: 2,
                transaction_id: 12,
            }
        );
        assert_eq!(document.text, "Hi Clay");
    }

    #[test]
    fn document_state_rejects_non_boundary_edit() {
        let mut document = DocumentState::new(7, "é".to_string(), DocumentAccess::Editable);

        let response = document.apply_edit(
            7,
            12,
            EditOperation::Insert {
                byte_offset: 1,
                text: "x".to_string(),
            },
        );

        assert!(matches!(
            response,
            ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                ..
            }
        ));
    }
}
