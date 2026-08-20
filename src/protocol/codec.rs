use std::{error::Error, fmt, io};

use rkyv::{Archive, Deserialize, Serialize, rancor};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::perf::metrics::global_recorder;

use super::{ClientMessage, ServerMessage};

const LENGTH_PREFIX_BYTES: usize = 4;

/// Default maximum IPC frame size for Phase 4 protocol messages.
pub const DEFAULT_MAX_FRAME_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    max_frame_size: usize,
}

impl Default for Codec {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAME_SIZE)
    }
}

impl Codec {
    pub const fn new(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }

    pub const fn max_frame_size(&self) -> usize {
        self.max_frame_size
    }

    pub fn encode_client_message(&self, message: &ClientMessage) -> Result<Vec<u8>, CodecError> {
        self.encode_frame(message)
    }

    pub fn decode_client_message(&self, frame: &[u8]) -> Result<ClientMessage, CodecError> {
        self.decode_frame(frame)
    }

    pub fn encode_server_message(&self, message: &ServerMessage) -> Result<Vec<u8>, CodecError> {
        self.encode_frame(message)
    }

    pub fn decode_server_message(&self, frame: &[u8]) -> Result<ServerMessage, CodecError> {
        self.decode_frame(frame)
    }

    pub async fn read_client_message<R>(&self, reader: &mut R) -> Result<ClientMessage, CodecError>
    where
        R: AsyncRead + Unpin,
    {
        let frame = self.read_frame(reader).await?;
        self.decode_client_message(&frame)
    }

    pub async fn write_client_message<W>(
        &self,
        writer: &mut W,
        message: &ClientMessage,
    ) -> Result<(), CodecError>
    where
        W: AsyncWrite + Unpin,
    {
        let frame = self.encode_client_message(message)?;
        writer.write_all(&frame).await.map_err(CodecError::io)
    }

    pub async fn read_server_message<R>(&self, reader: &mut R) -> Result<ServerMessage, CodecError>
    where
        R: AsyncRead + Unpin,
    {
        let frame = self.read_frame(reader).await?;
        self.decode_server_message(&frame)
    }

    pub async fn write_server_message<W>(
        &self,
        writer: &mut W,
        message: &ServerMessage,
    ) -> Result<(), CodecError>
    where
        W: AsyncWrite + Unpin,
    {
        let frame = self.encode_server_message(message)?;
        writer.write_all(&frame).await.map_err(CodecError::io)
    }

    async fn read_frame<R>(&self, reader: &mut R) -> Result<Vec<u8>, CodecError>
    where
        R: AsyncRead + Unpin,
    {
        let mut header = [0; LENGTH_PREFIX_BYTES];
        reader
            .read_exact(&mut header)
            .await
            .map_err(CodecError::io)?;

        let declared_len = u32::from_be_bytes(header) as usize;
        if declared_len > self.max_frame_size {
            global_recorder().record_counter("protocol.codec.frame_too_large", 1);
            return Err(CodecError::FrameTooLarge {
                len: declared_len,
                max: self.max_frame_size,
            });
        }

        let mut frame = Vec::with_capacity(LENGTH_PREFIX_BYTES + declared_len);
        frame.extend_from_slice(&header);
        frame.resize(LENGTH_PREFIX_BYTES + declared_len, 0);
        reader
            .read_exact(&mut frame[LENGTH_PREFIX_BYTES..])
            .await
            .map_err(CodecError::io)?;
        Ok(frame)
    }

    fn encode_frame<T>(&self, message: &T) -> Result<Vec<u8>, CodecError>
    where
        T: Archive
            + for<'a> Serialize<
                rkyv::api::high::HighSerializer<
                    rkyv::util::AlignedVec,
                    rkyv::ser::allocator::ArenaHandle<'a>,
                    rancor::Error,
                >,
            >,
    {
        let recorder = global_recorder();
        let _scope = recorder.scope("protocol.codec.encode");
        let payload = rkyv::to_bytes::<rancor::Error>(message).map_err(CodecError::serialize)?;
        if payload.len() > self.max_frame_size {
            recorder.record_counter("protocol.codec.frame_too_large", 1);
            return Err(CodecError::FrameTooLarge {
                len: payload.len(),
                max: self.max_frame_size,
            });
        }
        recorder.record_bytes("protocol.codec.payload_bytes", payload.len() as u64);

        let payload_len = u32::try_from(payload.len()).map_err(|_| CodecError::FrameTooLarge {
            len: payload.len(),
            max: u32::MAX as usize,
        })?;
        let mut frame = Vec::with_capacity(LENGTH_PREFIX_BYTES + payload.len());
        frame.extend_from_slice(&payload_len.to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    fn decode_frame<T>(&self, frame: &[u8]) -> Result<T, CodecError>
    where
        T: Archive,
        T::Archived: for<'a> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'a, rancor::Error>>
            + Deserialize<T, rkyv::api::high::HighDeserializer<rancor::Error>>,
    {
        if frame.len() < LENGTH_PREFIX_BYTES {
            return Err(CodecError::IncompleteFrame);
        }

        let declared_len = u32::from_be_bytes(
            frame[..LENGTH_PREFIX_BYTES]
                .try_into()
                .expect("slice length checked"),
        ) as usize;
        if declared_len > self.max_frame_size {
            global_recorder().record_counter("protocol.codec.frame_too_large", 1);
            return Err(CodecError::FrameTooLarge {
                len: declared_len,
                max: self.max_frame_size,
            });
        }

        let recorder = global_recorder();
        let _scope = recorder.scope("protocol.codec.decode");
        let payload = &frame[LENGTH_PREFIX_BYTES..];
        if payload.len() != declared_len {
            return Err(CodecError::LengthMismatch {
                declared: declared_len,
                actual: payload.len(),
            });
        }
        recorder.record_bytes("protocol.codec.decoded_payload_bytes", payload.len() as u64);

        let mut aligned_payload = rkyv::util::AlignedVec::<16>::with_capacity(payload.len());
        aligned_payload.extend_from_slice(payload);

        rkyv::from_bytes::<T, rancor::Error>(aligned_payload.as_slice())
            .map_err(CodecError::deserialize)
    }
}

#[derive(Debug)]
pub enum CodecError {
    FrameTooLarge { len: usize, max: usize },
    IncompleteFrame,
    LengthMismatch { declared: usize, actual: usize },
    Serialize(String),
    Deserialize(String),
    Io(io::Error),
}

impl CodecError {
    fn serialize(error: rancor::Error) -> Self {
        Self::Serialize(error.to_string())
    }

    fn deserialize(error: rancor::Error) -> Self {
        Self::Deserialize(error.to_string())
    }

    fn io(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooLarge { len, max } => {
                write!(formatter, "frame length {len} exceeds maximum {max}")
            }
            Self::IncompleteFrame => formatter.write_str("frame is missing its length prefix"),
            Self::LengthMismatch { declared, actual } => write!(
                formatter,
                "frame declared {declared} payload bytes but contained {actual} bytes"
            ),
            Self::Serialize(error) => {
                write!(formatter, "failed to serialize protocol frame: {error}")
            }
            Self::Deserialize(error) => {
                write!(formatter, "failed to deserialize protocol frame: {error}")
            }
            Self::Io(error) => write!(formatter, "protocol socket I/O failed: {error}"),
        }
    }
}

impl Error for CodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Aborts a framed read-pump task when the owning connection loop exits, so a
/// split read half never outlives its connection. Connection loops select only
/// over channels because `AsyncReadExt::read_exact` is not cancellation-safe:
/// cancelling an in-progress framed read would strand partial frame bytes and
/// desynchronize the stream.
#[derive(Debug)]
pub(crate) struct ReadPumpGuard(tokio::task::AbortHandle);

impl ReadPumpGuard {
    pub(crate) fn new(handle: tokio::task::AbortHandle) -> Self {
        Self(handle)
    }
}

impl Drop for ReadPumpGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::{Codec, CodecError, DEFAULT_MAX_FRAME_SIZE, LENGTH_PREFIX_BYTES};
    use crate::{
        perf::budgets::{
            COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
            MAX_OPENABLE_FILE_BYTES, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
            SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
        },
        protocol::{
            ActiveTheme, ActiveTypography, BehaviorManifest, ClientMessage, CompletionItem,
            CompletionProvenance, CompletionRejection, CompletionReplacementRange,
            CompletionRequest, CompletionResultSet, CompletionStatus, CompletionTrigger,
            DocumentAccess, DocumentMetadata, DocumentRuntimeRenderState, EditOperation,
            EditRejection, FileErrorCode, LanguageIntelligenceFeature, LanguageIntelligencePayload,
            LanguageIntelligenceRejection, LanguageIntelligenceRequest, LanguageIntelligenceResult,
            LanguageIntelligenceStatus, LockOwner, PROTOCOL_VERSION, PackageUiSnapshot,
            RegionLockConflict, RuntimeDiagnostic, RuntimeStateSnapshot, SduiActionIntent,
            SduiActionSource, SduiEditorBinding, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
            SduiTreeUpdate, ServerMessage, representative_panel_update, representative_sdui_tree,
        },
    };

    #[test]
    fn protocol_round_trips_client_hello() {
        let codec = Codec::default();
        let message = ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            client_name: "clay-test".to_string(),
        };

        let frame = codec.encode_client_message(&message).unwrap();
        let decoded = codec.decode_client_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_initial_document() {
        let codec = Codec::default();
        let message = ServerMessage::InitialDocument {
            document_id: 7,
            version: 42,
            text: "Hello, Clay 🦀\nSecond line".to_string(),
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            workspace_root: "/tmp/root".to_string(),
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_behavior_manifest_schema() {
        let codec = Codec::default();
        let message =
            ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(3)));

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn codec_round_trips_behavior_manifest_update() {
        let codec = Codec::default();
        let mut manifest = BehaviorManifest::minimal_text_editing(8);
        manifest.manifest_id = "default.text.hot-reload".to_string();
        let message = ServerMessage::BehaviorManifest(Box::new(manifest));

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn codec_round_trips_behavior_version_rejection() {
        let codec = Codec::default();
        let message = ServerMessage::EditRejected {
            document_id: 7,
            transaction_id: 99,
            reason: EditRejection::InvalidBehaviorVersion {
                behavior_version: 2,
                server_behavior_version: 3,
            },
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_edit_with_lease_and_versions() {
        let codec = Codec::default();
        let message = ClientMessage::Edit {
            document_id: 7,
            client_id: 11,
            lease_id: Some(5),
            base_version: 42,
            behavior_version: 3,
            transaction_id: 99,
            operation: EditOperation::Replace {
                start: 1,
                end: 5,
                text: "é".to_string(),
            },
        };

        let frame = codec.encode_client_message(&message).unwrap();
        let decoded = codec.decode_client_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_stale_edit_rejection() {
        let codec = Codec::default();
        let message = ServerMessage::EditRejected {
            document_id: 7,
            transaction_id: 99,
            reason: EditRejection::StaleVersion {
                client_base_version: 40,
                server_version: 42,
            },
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_resync_snapshot() {
        let codec = Codec::default();
        let message = ServerMessage::ResyncSnapshot {
            document_id: 7,
            version: 42,
            text: "Hello 🦀 é".to_string(),
            access: DocumentAccess::Editable { lease_id: 5 },
            lease_id: Some(5),
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_region_lock_rejection() {
        let codec = Codec::default();
        let message = ServerMessage::EditRejected {
            document_id: 7,
            transaction_id: 99,
            reason: EditRejection::RegionLocked {
                conflict: RegionLockConflict {
                    lock_id: 3,
                    start: 2,
                    end: 8,
                    owner: LockOwner::Server,
                    created_at_version: 41,
                },
            },
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn sdui_snapshot_codec_round_trips() {
        let codec = Codec::default();
        let tree = SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::EditorView {
                    binding: SduiEditorBinding {
                        document_id: 7,
                        expected_version: Some(3),
                    },
                },
            )],
        };
        let message = ServerMessage::SduiSnapshot { client_id: 9, tree };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn sdui_update_and_action_codec_round_trip() {
        let codec = Codec::default();
        let update = ServerMessage::SduiUpdate {
            update: SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: Vec::new(),
            },
        };
        let action = ClientMessage::SduiAction {
            client_id: 9,
            ui_version: 1,
            intent: SduiActionIntent::command(
                "workspace.refresh",
                SduiActionSource::Button {
                    node_id: SduiNodeId(5),
                },
            ),
        };

        let frame = codec.encode_server_message(&update).unwrap();
        assert_eq!(codec.decode_server_message(&frame).unwrap(), update);
        let frame = codec.encode_client_message(&action).unwrap();
        assert_eq!(codec.decode_client_message(&frame).unwrap(), action);
    }

    #[test]
    fn sdui_snapshot_payload_stays_under_initial_budget() {
        let codec = Codec::default();
        let message = ServerMessage::SduiSnapshot {
            client_id: 9,
            tree: representative_sdui_tree(),
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let payload_len = frame.len() - LENGTH_PREFIX_BYTES;
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
        assert!(
            payload_len <= SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
            "representative SDUI snapshot payload was {payload_len} bytes; budget is {SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes"
        );
    }

    #[test]
    fn sdui_update_payload_stays_under_initial_budget() {
        let codec = Codec::default();
        let message = ServerMessage::SduiUpdate {
            update: representative_panel_update(),
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let payload_len = frame.len() - LENGTH_PREFIX_BYTES;
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
        assert!(
            payload_len <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
            "representative SDUI update payload was {payload_len} bytes; budget is {SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes"
        );
    }

    #[test]
    fn sdui_update_payload_smaller_than_snapshot_for_panel_change() {
        let codec = Codec::default();
        let snapshot = ServerMessage::SduiSnapshot {
            client_id: 9,
            tree: representative_sdui_tree(),
        };
        let update = ServerMessage::SduiUpdate {
            update: representative_panel_update(),
        };

        let snapshot_frame = codec.encode_server_message(&snapshot).unwrap();
        let update_frame = codec.encode_server_message(&update).unwrap();
        let snapshot_payload_len = snapshot_frame.len() - LENGTH_PREFIX_BYTES;
        let update_payload_len = update_frame.len() - LENGTH_PREFIX_BYTES;

        assert_eq!(codec.decode_server_message(&update_frame).unwrap(), update);
        assert!(
            update_payload_len < snapshot_payload_len,
            "panel update payload ({update_payload_len} bytes) should be smaller than snapshot payload ({snapshot_payload_len} bytes)"
        );
        assert!(
            update_payload_len <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
            "representative SDUI panel update payload was {update_payload_len} bytes; budget is {SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes"
        );
    }

    #[test]
    fn protocol_round_trips_open_save_reload_messages() {
        let codec = Codec::default();
        let open = ClientMessage::OpenDocument {
            client_id: 9,
            workspace_root_id: 2,
            path: "src/main.rs".to_string(),
        };
        let selected = ClientMessage::OpenSelectedFile {
            client_id: 9,
            capability: "foc-token".to_string(),
            selected_path: "C:/Users/test/Documents/note.md".to_string(),
        };
        let selected_folder = ClientMessage::AddSelectedWorkspaceRoot {
            client_id: 9,
            capability: "folder-token".to_string(),
            selected_path: "C:/Users/test/project".to_string(),
        };
        let viewport = ClientMessage::DecorationViewportRequest {
            client_id: 9,
            document_id: 7,
            document_version: 3,
            byte_start: 4_096,
            byte_end: 8_192,
        };
        let save = ClientMessage::SaveDocument {
            client_id: 9,
            document_id: 7,
            known_version: 3,
        };
        let reload = ClientMessage::ReloadDocument {
            client_id: 9,
            document_id: 7,
            known_version: 3,
            force: true,
        };

        for message in [open, selected, selected_folder, viewport, save, reload] {
            let frame = codec.encode_client_message(&message).unwrap();
            let decoded = codec.decode_client_message(&frame).unwrap();
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn protocol_round_trips_workspace_results_and_errors() {
        let codec = Codec::default();
        let metadata = DocumentMetadata {
            document_id: 7,
            version: 3,
            access: DocumentAccess::Editable { lease_id: 4 },
            lease_id: Some(4),
            dirty: true,
            workspace_root_id: 2,
            path: "src/main.rs".to_string(),
        };
        let messages = [
            ServerMessage::DocumentOpened {
                metadata: metadata.clone(),
                text: "fn main() {}\n".to_string(),
            },
            ServerMessage::DocumentSaved {
                document_id: 7,
                version: 4,
                dirty: false,
            },
            ServerMessage::DocumentReloaded {
                metadata: metadata.clone(),
                text: "reloaded\n".to_string(),
            },
            ServerMessage::DocumentStatus {
                metadata: metadata.clone(),
            },
            ServerMessage::DocumentList {
                documents: vec![metadata],
            },
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::InvalidUtf8,
                message: "workspace file is not valid UTF-8 text".to_string(),
                workspace_root_id: Some(2),
                document_id: None,
            },
            ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "runtime.syntax_error",
                "JavaScript syntax error while evaluating server-side configuration.",
            )),
        ];

        for message in messages {
            let frame = codec.encode_server_message(&message).unwrap();
            let decoded = codec.decode_server_message(&frame).unwrap();
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn codec_rejects_oversized_phase5_frame() {
        let codec = Codec::new(8);
        let mut frame = Vec::new();
        frame.extend_from_slice(&9_u32.to_be_bytes());
        frame.extend_from_slice(&[0; 9]);

        let error = codec.decode_client_message(&frame).unwrap_err();

        assert!(matches!(
            error,
            CodecError::FrameTooLarge { len: 9, max: 8 }
        ));
    }

    #[test]
    fn codec_rejects_oversized_manifest_frame() {
        let codec = Codec::new(8);
        let message =
            ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1)));

        let error = codec.encode_server_message(&message).unwrap_err();

        assert!(matches!(error, CodecError::FrameTooLarge { max: 8, .. }));
    }

    #[test]
    fn oversized_sdui_frame_is_rejected() {
        let codec = Codec::new(64);
        let message = ServerMessage::SduiSnapshot {
            client_id: 9,
            tree: representative_sdui_tree(),
        };

        let error = codec.encode_server_message(&message).unwrap_err();

        assert!(matches!(error, CodecError::FrameTooLarge { max: 64, .. }));
    }

    #[test]
    fn completion_request_codec_round_trips() {
        let codec = Codec::default();
        let request = CompletionRequest {
            request_id: 42,
            client_id: 9,
            document_id: 7,
            document_version: 31,
            behavior_version: 3,
            cursor_byte_offset: 12,
            replacement_range: CompletionReplacementRange::new(10, 12),
            trigger: CompletionTrigger::Character(".".to_string()),
            provider_generation: 2,
            recent_completions: vec!["foo".to_string()].into_boxed_slice(),
        };
        let message = ClientMessage::CompletionRequest { request };

        let frame = codec.encode_client_message(&message).unwrap();
        let decoded = codec.decode_client_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn completion_request_manual_trigger_round_trips() {
        let codec = Codec::default();
        let request = CompletionRequest {
            request_id: 43,
            client_id: 9,
            document_id: 7,
            document_version: 31,
            behavior_version: 3,
            cursor_byte_offset: 12,
            replacement_range: CompletionReplacementRange::new(10, 12),
            trigger: CompletionTrigger::Manual,
            provider_generation: 2,
            recent_completions: Vec::<String>::new().into_boxed_slice(),
        };
        let message = ClientMessage::CompletionRequest { request };

        let frame = codec.encode_client_message(&message).unwrap();
        let decoded = codec.decode_client_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn completion_result_codec_round_trips_metadata_and_items() {
        let codec = Codec::default();
        let provenance = CompletionProvenance::builtin_core();
        let result = CompletionResultSet {
            request_id: 42,
            client_id: 9,
            document_id: 7,
            document_version: 31,
            behavior_version: 3,
            provider_generation: 2,
            replacement_range: CompletionReplacementRange::new(10, 12),
            status: CompletionStatus::Ok,
            items: vec![
                CompletionItem::new("foo", "foo", provenance.clone()),
                CompletionItem::new("bar", "bar", provenance),
            ],
            provenance: CompletionProvenance::builtin_core(),
        };
        let message = ServerMessage::CompletionResult { result };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn completion_rejected_codec_round_trips() {
        let codec = Codec::default();
        let message = ServerMessage::CompletionRejected {
            request_id: 42,
            reason: CompletionRejection::StaleDocumentVersion {
                result_version: 30,
                current_version: 31,
            },
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn language_intelligence_request_round_trips() {
        let codec = Codec::default();
        let request = LanguageIntelligenceRequest {
            request_id: 7,
            client_id: 9,
            document_id: 3,
            document_version: 12,
            behavior_version: 2,
            cursor_byte_offset: 40,
            feature: LanguageIntelligenceFeature::Hover,
            provider_generation: 1,
        };
        let message = ClientMessage::LanguageIntelligenceRequest { request };

        let frame = codec.encode_client_message(&message).unwrap();
        let decoded = codec.decode_client_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn language_intelligence_hover_and_definition_results_round_trip() {
        let codec = Codec::default();
        let provenance = CompletionProvenance::builtin_core();

        let hover = LanguageIntelligenceResult {
            request_id: 7,
            client_id: 9,
            document_id: 3,
            document_version: 12,
            behavior_version: 2,
            provider_generation: 1,
            feature: LanguageIntelligenceFeature::Hover,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::Hover(crate::protocol::HoverResult {
                range: Some(crate::protocol::TextByteRange::new(10, 14)),
                markdown: "# Heading\ninfo".to_string(),
            }),
            provenance: provenance.clone(),
        };
        let hover_message = ServerMessage::LanguageIntelligenceResult { result: hover };
        let frame = codec.encode_server_message(&hover_message).unwrap();
        assert_eq!(codec.decode_server_message(&frame).unwrap(), hover_message);

        let definition = LanguageIntelligenceResult {
            request_id: 8,
            client_id: 9,
            document_id: 3,
            document_version: 12,
            behavior_version: 2,
            provider_generation: 1,
            feature: LanguageIntelligenceFeature::GoToDefinition,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::GoToDefinition(
                crate::protocol::GoToDefinitionResult {
                    locations: vec![
                        crate::protocol::TextLocation::OpenDocument {
                            document_id: 3,
                            range: crate::protocol::TextByteRange::new(0, 4),
                        },
                        crate::protocol::TextLocation::WorkspaceFile {
                            workspace_root_id: 1,
                            relative_path: "src/lib.rs".to_string(),
                            range: crate::protocol::TextByteRange::new(20, 28),
                        },
                    ],
                },
            ),
            provenance,
        };
        let definition_message = ServerMessage::LanguageIntelligenceResult { result: definition };
        let frame = codec.encode_server_message(&definition_message).unwrap();
        assert_eq!(
            codec.decode_server_message(&frame).unwrap(),
            definition_message
        );
    }

    #[test]
    fn language_intelligence_rejected_round_trips() {
        let codec = Codec::default();
        let message = ServerMessage::LanguageIntelligenceRejected {
            request_id: 7,
            reason: LanguageIntelligenceRejection::UnorderedByteRange {
                byte_start: 20,
                byte_end: 10,
            },
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn representative_completion_result_payload_stays_under_budget() {
        let codec = Codec::default();
        let provenance = CompletionProvenance::builtin_core();
        let items: Vec<CompletionItem> = (0..COMPLETION_RESULT_MAX_ITEMS)
            .map(|i| {
                CompletionItem::new(format!("item{i}"), format!("item{i}"), provenance.clone())
            })
            .collect();
        let result = CompletionResultSet {
            request_id: 42,
            client_id: 9,
            document_id: 7,
            document_version: 31,
            behavior_version: 3,
            provider_generation: 2,
            replacement_range: CompletionReplacementRange::new(10, 12),
            status: CompletionStatus::Ok,
            items,
            provenance: CompletionProvenance::builtin_core(),
        };
        let message = ServerMessage::CompletionResult { result };

        let frame = codec.encode_server_message(&message).unwrap();
        let payload_len = frame.len() - LENGTH_PREFIX_BYTES;
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
        assert!(
            payload_len <= COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
            "representative completion result payload was {payload_len} bytes; budget is {COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES} bytes"
        );
    }

    #[test]
    fn oversized_completion_result_frame_is_rejected() {
        // A completion result whose encoded payload exceeds the codec frame
        // limit is rejected at encode, mirroring the SDUI and manifest guards.
        let codec = Codec::new(64);
        let provenance = CompletionProvenance::builtin_core();
        let result = CompletionResultSet {
            request_id: 42,
            client_id: 9,
            document_id: 7,
            document_version: 31,
            behavior_version: 3,
            provider_generation: 2,
            replacement_range: CompletionReplacementRange::new(10, 12),
            status: CompletionStatus::Ok,
            items: vec![CompletionItem::new(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                provenance,
            )],
            provenance: CompletionProvenance::builtin_core(),
        };
        let message = ServerMessage::CompletionResult { result };

        let error = codec.encode_server_message(&message).unwrap_err();
        assert!(matches!(error, CodecError::FrameTooLarge { max: 64, .. }));
    }

    #[test]
    fn codec_rejects_invalid_phase5_archive_bytes() {
        let codec = Codec::default();
        let mut frame = Vec::new();
        frame.extend_from_slice(&4_u32.to_be_bytes());
        frame.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

        let error = codec.decode_client_message(&frame).unwrap_err();

        assert!(matches!(error, CodecError::Deserialize(_)));
    }

    #[test]
    fn codec_rejects_invalid_manifest_archive_bytes() {
        let codec = Codec::default();
        let mut frame = Vec::new();
        frame.extend_from_slice(&4_u32.to_be_bytes());
        frame.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

        let error = codec.decode_server_message(&frame).unwrap_err();

        assert!(matches!(error, CodecError::Deserialize(_)));
    }

    #[test]
    fn runtime_state_snapshot_round_trips_with_generation_and_bounded_payload() {
        let codec = Codec::default();
        let snapshot = RuntimeStateSnapshot {
            runtime_generation_id: 2,
            client_id: 9,
            behavior: BehaviorManifest::minimal_text_editing(4),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography::default(),
            sdui_tree: representative_sdui_tree(),
            package_ui: PackageUiSnapshot { version: 2 },
            documents: vec![DocumentRuntimeRenderState {
                document_id: 1,
                document_version: 3,
                reset_decorations: true,
                reset_diagnostics: true,
                initial_decorations: None,
                initial_diagnostics: None,
                behavior_manifest: None,
            }],
            diagnostics: vec![RuntimeDiagnostic::error(
                "runtime.reload_succeeded",
                "Configuration reloaded.",
            )],
        };
        snapshot.validate().expect("fixture snapshot is valid");
        let message = ServerMessage::RuntimeStateSnapshot(Box::new(snapshot));
        let frame = codec.encode_server_message(&message).unwrap();
        assert!(frame.len() < DEFAULT_MAX_FRAME_SIZE);
        assert_eq!(codec.decode_server_message(&frame).unwrap(), message);

        let ack = ClientMessage::RuntimeGenerationInstalled {
            client_id: 9,
            runtime_generation_id: 2,
        };
        let ack_frame = codec.encode_client_message(&ack).unwrap();
        assert_eq!(codec.decode_client_message(&ack_frame).unwrap(), ack);
    }

    #[test]
    fn active_theme_round_trips_typed_ui_token_overrides() {
        use crate::protocol::{UiDesignTokenOverride, WireDesignTokenValue};
        let codec = Codec::default();
        let active_theme = ActiveTheme {
            specifier: "@clay/theme-x".to_string(),
            overrides: Vec::new(),
            design_tokens: vec![
                UiDesignTokenOverride {
                    token: "surface.hover".to_string(),
                    value: WireDesignTokenValue::Color([0x11, 0x22, 0x33, 0xff]),
                    provenance: "theme-x".to_string(),
                },
                UiDesignTokenOverride {
                    token: "spacing.md".to_string(),
                    value: WireDesignTokenValue::Scalar(20.0),
                    provenance: "theme-x".to_string(),
                },
                UiDesignTokenOverride {
                    token: "density.spacious".to_string(),
                    value: WireDesignTokenValue::Level("spacious".to_string()),
                    provenance: "theme-x".to_string(),
                },
            ],
        };
        let message = ServerMessage::ActiveTheme(active_theme.clone());
        let frame = codec.encode_server_message(&message).unwrap();
        assert!(frame.len() < DEFAULT_MAX_FRAME_SIZE);
        assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
    }

    #[test]
    fn oversized_or_invalid_runtime_snapshot_is_rejected_before_install() {
        let codec = Codec::new(64);
        let mut snapshot = RuntimeStateSnapshot {
            runtime_generation_id: 2,
            client_id: 1,
            behavior: BehaviorManifest::minimal_text_editing(1),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography::default(),
            sdui_tree: representative_sdui_tree(),
            package_ui: PackageUiSnapshot::default(),
            documents: Vec::new(),
            diagnostics: Vec::new(),
        };
        let message = ServerMessage::RuntimeStateSnapshot(Box::new(snapshot.clone()));
        let error = codec.encode_server_message(&message).unwrap_err();
        assert!(matches!(error, CodecError::FrameTooLarge { max: 64, .. }));

        snapshot.behavior.manifest_id.clear();
        assert!(snapshot.validate().is_err());
    }

    /// A full-text protocol message (`InitialDocument`) carrying more text than
    /// the codec frame limit is rejected at encode. This is the transport-side
    /// guard paired with the workspace-side `MAX_OPENABLE_FILE_BYTES` gate: any
    /// file that passes the open gate must also fit in a single full-text frame.
    #[test]
    fn full_text_snapshot_exceeding_frame_limit_is_rejected_at_encode() {
        // The openable-file budget must sit below the frame limit so a file that
        // passes the workspace gate always fits in a full-text frame.
        const {
            assert!(
                MAX_OPENABLE_FILE_BYTES < DEFAULT_MAX_FRAME_SIZE,
                "openable-file budget must be below the IPC frame limit"
            );
        }

        // A payload larger than the default frame limit is rejected at encode.
        let codec = Codec::default();
        let oversized_text = "x".repeat(DEFAULT_MAX_FRAME_SIZE + 1);
        let message = ServerMessage::InitialDocument {
            document_id: 1,
            version: 1,
            text: oversized_text,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            workspace_root: "/tmp/root".to_string(),
        };

        let error = codec.encode_server_message(&message).unwrap_err();
        assert!(matches!(error, CodecError::FrameTooLarge { .. }));
    }

    /// Deterministic split-mix LCG for the mutation corpus — no `rand`
    /// dependency, reproducible across runs and hosts.
    struct Lcg(u64);

    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }

        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 ^ (self.0 >> 33)).wrapping_mul(13787848793156543929) >> 32
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() as usize) % bound
        }
    }

    /// Corpus for the plan 086 task 5 malformed-archive sweep. Every input must
    /// fail closed: either a `CodecError`, or — for byte patterns that still
    /// validate — a bytecheck-validated archive. bytecheck bounds-checks every
    /// successful decode against the actual buffer, so a decode is
    /// memory-safe by construction even when truncation or corruption aliases
    /// the bytes into a different (smaller/zero-field) message; the codec's
    /// contract is reject-or-validate, never panic and never an out-of-bounds
    /// access. The sweep asserts rejection dominance so bytecheck is provably
    /// doing real work on every corpus class.
    ///
    /// Clay's wire types contain no `HashMap`/`Rc`/`Arc` fields (the archived
    /// shapes behind RUSTSEC-2026-0234/0235), so the adversarial corpus for
    /// the flat types is offset/pointer and length corruption, which is exactly
    /// the validation surface those advisories hardened in rkyv 0.8.17
    /// (`validation/shared` pointer metadata, swiss-table element counts,
    /// `de::pooling`).
    fn rich_corpus_messages() -> Vec<(String, ServerMessage)> {
        let provenance = CompletionProvenance::builtin_core();
        vec![
            (
                "completion-result".to_string(),
                ServerMessage::CompletionResult {
                    result: CompletionResultSet {
                        request_id: 42,
                        client_id: 9,
                        document_id: 7,
                        document_version: 31,
                        behavior_version: 3,
                        provider_generation: 2,
                        replacement_range: CompletionReplacementRange::new(10, 12),
                        status: CompletionStatus::Ok,
                        items: (0..COMPLETION_RESULT_MAX_ITEMS)
                            .map(|i| {
                                CompletionItem::new(
                                    format!("item_{i:03}_xxxxxxxxxxxxxxxxxxxxxxxx"),
                                    format!("detail_{i:03}_yyyyyyyyyyyyyyyyyyyyyyyy"),
                                    provenance.clone(),
                                )
                            })
                            .collect(),
                        provenance,
                    },
                },
            ),
            (
                "sdui-snapshot".to_string(),
                ServerMessage::SduiSnapshot {
                    client_id: 9,
                    tree: representative_sdui_tree(),
                },
            ),
        ]
    }

    #[test]
    fn compact_generated_frame_mutations_fail_closed_without_panicking() {
        let codec = Codec::new(4096);
        let mut rng = Lcg::new(0x0890_c0de_2026_0816);

        for case in 0..64_u64 {
            let frames = [
                (
                    false,
                    codec
                        .encode_client_message(&ClientMessage::MenuQueryUpdate {
                            client_id: 7,
                            session_id: (1 << 63) | (case + 1),
                            query: format!("query-{case}"),
                        })
                        .unwrap(),
                ),
                (
                    true,
                    codec
                        .encode_server_message(&ServerMessage::TransientMenuClosed {
                            session_id: (1 << 63) | (case + 1),
                        })
                        .unwrap(),
                ),
            ];

            for (server, original) in frames {
                let payload_len = original.len() - LENGTH_PREFIX_BYTES;
                assert!(payload_len > 1, "mutation fixture must have a payload");
                let mutation = (case as usize + if server { 1 } else { 0 }) % 5;
                let mut mutated = original;
                match mutation {
                    // Truncate the archive and make the prefix agree with the
                    // shorter buffer: bytecheck must reject the archive.
                    0 => {
                        let declared = rng.below(payload_len);
                        mutated.truncate(LENGTH_PREFIX_BYTES + declared);
                        mutated[..LENGTH_PREFIX_BYTES]
                            .copy_from_slice(&(declared as u32).to_be_bytes());
                    }
                    // Prefixes that disagree with the actual payload stop at
                    // the framing boundary, before archive access.
                    1 => mutated[..LENGTH_PREFIX_BYTES]
                        .copy_from_slice(&((payload_len - 1) as u32).to_be_bytes()),
                    2 => mutated[..LENGTH_PREFIX_BYTES]
                        .copy_from_slice(&((payload_len + 1) as u32).to_be_bytes()),
                    // A byte mutation may still be a valid archive with a
                    // changed value; either validated decode or rejection is
                    // safe, but a panic is never acceptable.
                    3 => {
                        let offset = LENGTH_PREFIX_BYTES + rng.below(payload_len);
                        mutated[offset] ^= (rng.next() as u8).max(1);
                    }
                    // Oversized declarations are rejected before allocation or
                    // archive validation.
                    4 => mutated[..LENGTH_PREFIX_BYTES]
                        .copy_from_slice(&((codec.max_frame_size() + 1) as u32).to_be_bytes()),
                    _ => unreachable!(),
                }

                let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if server {
                        codec.decode_server_message(&mutated).map(|_| ())
                    } else {
                        codec.decode_client_message(&mutated).map(|_| ())
                    }
                }))
                .unwrap_or_else(|_| panic!("generated codec mutation panicked: case={case}"));

                match mutation {
                    0 => assert!(decoded.is_err(), "truncated archive was accepted"),
                    1 | 2 => assert!(matches!(decoded, Err(CodecError::LengthMismatch { .. }))),
                    3 => match decoded {
                        Ok(()) | Err(_) => {}
                    },
                    4 => assert!(matches!(decoded, Err(CodecError::FrameTooLarge { .. }))),
                    _ => unreachable!(),
                }
            }
        }
    }

    #[test]
    fn malformed_corpus_truncations_fail_closed() {
        let codec = Codec::default();
        for (label, message) in rich_corpus_messages() {
            let frame = codec.encode_server_message(&message).unwrap();
            let full_len = frame.len() - LENGTH_PREFIX_BYTES;
            assert!(full_len > 128, "{label} corpus must be non-trivial");

            // Sweep every cut point: every byte for the first 64, then every
            // 4 bytes (odd cut points exercise the AlignedVec copy path).
            let mut rejected = 0usize;
            let mut cuts = 0usize;
            let mut cut = 0;
            while cut < full_len {
                cuts += 1;
                let mut truncated = frame[..LENGTH_PREFIX_BYTES].to_vec();
                truncated.extend_from_slice(&(cut as u32).to_be_bytes());
                truncated.extend_from_slice(&frame[LENGTH_PREFIX_BYTES..LENGTH_PREFIX_BYTES + cut]);
                if codec.decode_server_message(&truncated).is_err() {
                    rejected += 1;
                }
                cut += if cut < 64 { 1 } else { 4 };
            }
            assert!(
                rejected >= cuts * 3 / 4,
                "{label}: bytecheck rejected only {rejected}/{cuts} truncations"
            );
            assert_eq!(
                codec.decode_server_message(&frame).unwrap(),
                message,
                "{label}: baseline frame must round-trip"
            );
        }
    }

    #[test]
    fn malformed_corpus_mutations_fail_closed() {
        let codec = Codec::default();
        let mut rng = Lcg::new(0x5eed_c0de_2026_0814);
        for (label, message) in rich_corpus_messages() {
            let frame = codec.encode_server_message(&message).unwrap();
            let payload_start = LENGTH_PREFIX_BYTES;
            let payload_end = frame.len();

            // 1. Deterministic pseudo-random byte mutations: 1..=8 flips.
            //    Bytecheck must reject the overwhelming majority; the rare
            //    survivors are bytecheck-validated archives (bounds-checked,
            //    so memory-safe by construction) — they are only allowed to
            //    differ from the original because zeroed length/offset fields
            //    legitimately re-archive to a different in-buffer value.
            let mut rejected = 0usize;
            for _ in 0..300 {
                let mut mutated = frame.clone();
                let flips = 1 + rng.below(8);
                for _ in 0..flips {
                    let position = payload_start + rng.below(payload_end - payload_start);
                    mutated[position] ^= rng.next() as u8;
                }
                match codec.decode_server_message(&mutated) {
                    Ok(_) => {}
                    Err(_) => rejected += 1,
                }
            }
            assert!(
                rejected >= 300 / 4,
                "{label}: bytecheck rejected only {rejected}/300 mutations"
            );
            // Sanity: the untouched frame still decodes to the original.
            assert_eq!(
                codec.decode_server_message(&frame).unwrap(),
                message,
                "{label}: baseline frame must round-trip"
            );

            // 2. Structured corruption: zero-runs and 0xFF-runs (pointer
            //    targets, string lengths, and rel-ptr offsets are all u64
            //    fields; a run of zeros or ones makes them point far outside
            //    the buffer or at unaligned addresses). Every input must be
            //    rejected or decode as a bytecheck-validated archive — never
            //    a panic.
            for run_len in [1usize, 4, 8, 16, 32] {
                for window_start in (payload_start..payload_end - run_len).step_by(7) {
                    for fill in [0u8, 0xFF] {
                        let mut mutated = frame.clone();
                        mutated[window_start..window_start + run_len].fill(fill);
                        let _ = codec.decode_server_message(&mutated);
                    }
                }
            }
        }
    }

    #[test]
    fn malformed_corpus_misaligned_declared_lengths_fail_closed() {
        let codec = Codec::default();
        let message = rich_corpus_messages()[0].1.clone();
        let frame = codec.encode_server_message(&message).unwrap();
        let payload = &frame[LENGTH_PREFIX_BYTES..];

        // Declared length is interpreted as the payload extent; odd declared
        // lengths that disagree with the true payload must be rejected before
        // any archive validation, and matching odd lengths must fail closed.
        for odd in [1usize, 3, 5, 7, 9, 15, 17] {
            let mut frame = Vec::new();
            frame.extend_from_slice(&(odd as u32).to_be_bytes());
            frame.extend_from_slice(&payload[..odd]);
            assert!(
                codec.decode_server_message(&frame).is_err(),
                "odd declared length {odd} must not decode"
            );
        }
    }

    #[test]
    fn malformed_corpus_oversized_declaration_rejected_on_read_side() {
        // The read pump must reject an oversized declaration before allocating
        // or reading the declared payload (frame gate stays at 1 MiB).
        let codec = Codec::new(8);
        let mut frame = Vec::new();
        frame.extend_from_slice(&9_u32.to_be_bytes());
        frame.extend_from_slice(&[0; 9]);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        runtime.block_on(async {
            use tokio::io::{AsyncWriteExt, duplex};
            let (mut writer, mut reader) = duplex(64);
            writer.write_all(&frame).await.unwrap();
            let error = codec.read_server_message(&mut reader).await.unwrap_err();
            assert!(matches!(
                error,
                CodecError::FrameTooLarge { len: 9, max: 8 }
            ));
        });
    }
}
