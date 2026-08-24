//! Editor-facing DTO helpers for the Tauri bridge.
//!
//! Document authority stays on the Clay server. This module only:
//! - re-exports the UTF-16↔UTF-8 map the frontend mirrors
//! - pins the JSON shape of document connection events the webview consumes

pub use clay::editor::position_map::{utf8_to_utf16, utf16_to_utf8};

#[cfg(test)]
mod tests {
    use super::{utf8_to_utf16, utf16_to_utf8};
    use clay::client::ClientConnectionEvent;
    use clay::protocol::{DocumentAccess, DocumentMetadata};

    #[test]
    fn position_map_matches_frontend_golden_vectors() {
        // Keep in lockstep with frontend/src/editor/position-map.ts.
        let vectors = [
            ("", 0, 0),
            ("abc", 1, 1),
            ("héllo", 2, 3),
            ("a😀b", 3, 5),
            ("e\u{0301}", 2, 3),
            ("a\r\nb", 2, 2),
            ("𐍈", 2, 4),
        ];
        for (text, utf16, utf8) in vectors {
            assert_eq!(utf16_to_utf8(text, utf16), utf8);
            assert_eq!(utf8_to_utf16(text, utf8), utf16);
        }
    }

    #[test]
    fn document_events_serialize_camel_case() {
        let ack = ClientConnectionEvent::EditAck {
            document_id: 1,
            version: 4,
            transaction_id: 9,
        };
        let json = serde_json::to_value(&ack).unwrap();
        assert_eq!(json["kind"], "editAck");
        assert_eq!(json["data"]["documentId"], 1);
        assert_eq!(json["data"]["transactionId"], 9);

        let opened = ClientConnectionEvent::DocumentOpened {
            metadata: DocumentMetadata {
                document_id: 1,
                version: 2,
                access: DocumentAccess::Editable { lease_id: 3 },
                lease_id: Some(3),
                dirty: true,
                workspace_root_id: 7,
                path: "notes.md".into(),
            },
            text: "hi".into(),
        };
        let json = serde_json::to_value(&opened).unwrap();
        assert_eq!(json["kind"], "documentOpened");
        assert_eq!(json["data"]["metadata"]["workspaceRootId"], 7);
        assert_eq!(json["data"]["metadata"]["leaseId"], 3);
        assert_eq!(json["data"]["text"], "hi");
    }
}
