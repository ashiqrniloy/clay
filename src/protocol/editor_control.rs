//! Plan 071 follow-up round (`editor-control`): the bounded server→client
//! push channel that lets gated packages trigger editor command IDs
//! programmatically. The server publishes [`EditorCommandRequest`] only after
//! the `editor-control` gate (approved permission + declared active mode)
//! passes; the client re-parses the command ID deny-by-default and dispatches
//! it through the same path as keybinding-routed command IDs.

/// Maximum byte length of a pushed editor command ID.
pub const MAX_EDITOR_COMMAND_REQUEST_ID_BYTES: usize = 256;
/// Maximum byte length of the host-stamped package provenance prefix.
pub const MAX_EDITOR_COMMAND_PROVENANCE_BYTES: usize = 64;
/// Maximum byte length of the active mode ID recorded for provenance.
pub const MAX_EDITOR_COMMAND_MODE_ID_BYTES: usize = 128;

/// Advisory server→client request to execute one known editor command ID
/// (movement/selection/caret/multi-cursor/textobject/smart-select). The
/// client drops unknown IDs silently; stale or malformed requests never
/// mutate document text.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EditorCommandRequest {
    /// Direction-specific argless editor command ID, e.g.
    /// `editor.clientMoveCursor.nextWordStart`.
    pub command_id: String,
    /// Host-stamped apiPrefix of the package that requested execution
    /// (`clay.config` for trusted user-configuration callers).
    pub package_prefix: String,
    /// Active major mode ID at gate time (provenance for the user).
    pub mode_id: String,
}

impl EditorCommandRequest {
    /// Wire-shape validation only (bounded strings). Known-command-ID
    /// enforcement happens at the publishing op and again client-side.
    pub fn validate(&self) -> bool {
        !self.command_id.is_empty()
            && self.command_id.len() <= MAX_EDITOR_COMMAND_REQUEST_ID_BYTES
            && !self.package_prefix.is_empty()
            && self.package_prefix.len() <= MAX_EDITOR_COMMAND_PROVENANCE_BYTES
            && !self.mode_id.is_empty()
            && self.mode_id.len() <= MAX_EDITOR_COMMAND_MODE_ID_BYTES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(command_id: &str, package_prefix: &str, mode_id: &str) -> EditorCommandRequest {
        EditorCommandRequest {
            command_id: command_id.to_string(),
            package_prefix: package_prefix.to_string(),
            mode_id: mode_id.to_string(),
        }
    }

    #[test]
    fn editor_command_request_validate_is_bounded() {
        assert!(
            request(
                "editor.clientMoveCursor.nextWordStart",
                "markdown",
                "markdown"
            )
            .validate()
        );
        assert!(!request("", "markdown", "markdown").validate());
        assert!(!request("editor.clientMoveCursor.nextWordStart", "", "markdown").validate());
        assert!(!request("editor.clientMoveCursor.nextWordStart", "markdown", "").validate());
        assert!(
            !request(
                &"x".repeat(MAX_EDITOR_COMMAND_REQUEST_ID_BYTES + 1),
                "markdown",
                "markdown"
            )
            .validate()
        );
        assert!(
            !request(
                "id",
                &"x".repeat(MAX_EDITOR_COMMAND_PROVENANCE_BYTES + 1),
                "markdown"
            )
            .validate()
        );
        assert!(
            !request(
                "id",
                "markdown",
                &"x".repeat(MAX_EDITOR_COMMAND_MODE_ID_BYTES + 1)
            )
            .validate()
        );
    }
}
