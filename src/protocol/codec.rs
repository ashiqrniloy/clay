use std::{error::Error, fmt};

use rkyv::{Archive, Deserialize, Serialize, rancor};

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
        let payload = rkyv::to_bytes::<rancor::Error>(message).map_err(CodecError::serialize)?;
        if payload.len() > self.max_frame_size {
            return Err(CodecError::FrameTooLarge {
                len: payload.len(),
                max: self.max_frame_size,
            });
        }

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
            return Err(CodecError::FrameTooLarge {
                len: declared_len,
                max: self.max_frame_size,
            });
        }

        let payload = &frame[LENGTH_PREFIX_BYTES..];
        if payload.len() != declared_len {
            return Err(CodecError::LengthMismatch {
                declared: declared_len,
                actual: payload.len(),
            });
        }

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
}

impl CodecError {
    fn serialize(error: rancor::Error) -> Self {
        Self::Serialize(error.to_string())
    }

    fn deserialize(error: rancor::Error) -> Self {
        Self::Deserialize(error.to_string())
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
        }
    }
}

impl Error for CodecError {}

#[cfg(test)]
mod tests {
    use super::{Codec, CodecError};
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, EditOperation, PROTOCOL_VERSION,
        ServerMessage,
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
            access: DocumentAccess::Editable,
        };

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_behavior_manifest() {
        let codec = Codec::default();
        let message = ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3));

        let frame = codec.encode_server_message(&message).unwrap();
        let decoded = codec.decode_server_message(&frame).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn protocol_round_trips_edit_operation_delta() {
        let codec = Codec::default();
        let message = ClientMessage::Edit {
            document_id: 7,
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
    fn codec_rejects_oversized_frame() {
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
    fn codec_rejects_invalid_archive_bytes() {
        let codec = Codec::default();
        let mut frame = Vec::new();
        frame.extend_from_slice(&4_u32.to_be_bytes());
        frame.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

        let error = codec.decode_client_message(&frame).unwrap_err();

        assert!(matches!(error, CodecError::Deserialize(_)));
    }
}
