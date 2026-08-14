//! Centralized accessibility label helpers for daily-editing surfaces.
//!
//! Labels stay sanitized for assistive tools and structural tests: basename-only
//! document titles, no absolute host paths, no clipboard/preedit contents, and
//! truncated recovery/prompt summaries.

use std::path::Path;

use masonry::accesskit::NodeId;
use masonry::core::WidgetId;

use crate::perf::budgets::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

/// Deterministic IDs for clay-owned synthetic accessibility nodes
/// (TabList/Tab, live announcement, status lines, menu items/status).
///
/// Real widget IDs are small sequential integers (masonry's global counter
/// starts at 1), so the high-bit prefix can never collide with a widget node.
/// The owner is the retained widget that attaches the synthetic node, so IDs
/// are stable across accessibility passes — the consumer never sees churn —
/// and die with their owner (a replaced widget gets fresh IDs). Slot is a
/// typed per-owner index; the 9-bit space is enough for every bounded list:
/// tabs <= `MAX_ACTIVE_CONNECTIONS` (64), menu items <= `TRANSIENT_MENU_MAX_ITEMS` (256).
pub(crate) const VIRTUAL_A11Y_NODE_PREFIX: u64 = 0xD000_0000_0000_0000;
const VIRTUAL_A11Y_OWNER_MASK: u64 = 0x0000_7FFF_FFFF_FFFF;
const VIRTUAL_A11Y_SLOT_BITS: u32 = 9;

pub(crate) fn virtual_a11y_node_id(owner: WidgetId, slot: u16) -> NodeId {
    assert!(
        u64::from(slot) < (1u64 << VIRTUAL_A11Y_SLOT_BITS),
        "virtual accessibility slot {slot} exceeds the 9-bit per-owner space"
    );
    NodeId::from(
        VIRTUAL_A11Y_NODE_PREFIX
            | ((owner.to_raw() & VIRTUAL_A11Y_OWNER_MASK) << VIRTUAL_A11Y_SLOT_BITS)
            | u64::from(slot),
    )
}

/// Per-owner slot namespaces (must be unique within one owner widget).
pub(crate) mod virtual_a11y_slots {
    // Shell: TabList = 1, live announcement = 2, Tab(i) = 3 + client_id.
    pub(crate) const SHELL_TAB_LIST: u16 = 1;
    pub(crate) const SHELL_ANNOUNCEMENT: u16 = 2;
    pub(crate) const SHELL_TAB_BASE: u16 = 3;
    // Editor / pane-document: status line = 1.
    pub(crate) const STATUS: u16 = 1;
    // Package region: status = 1, Item(i) = 2 + i (legacy numbering).
    pub(crate) const REGION_MENU_STATUS: u16 = 1;
    pub(crate) const REGION_MENU_ITEM_BASE: u16 = 2;
}

pub(crate) const ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS: usize = 64;
pub(crate) const ACCESSIBILITY_RECOVERY_SUMMARY_MAX_CHARS: usize =
    TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

/// Inputs for composing the editor AccessKit root label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorAccessibilityLabelParts<'a> {
    pub status_text: &'a str,
    pub theme_label: &'a str,
    pub composing: bool,
    pub recovery_summary: Option<&'a str>,
    pub visible_text: &'a str,
    pub empty_placeholder: &'a str,
}

/// Sanitize a workspace-relative or host path into a basename display name.
///
/// Absolute paths, `..` segments, and empty inputs collapse to a safe fallback
/// so accessibility never announces host filesystem layout.
pub(crate) fn sanitize_document_display_name(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "untitled".to_string();
    }

    let candidate = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .trim();

    let basename = if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || trimmed.starts_with('/')
            && Path::new(trimmed)
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| name.is_empty())
    {
        "untitled"
    } else {
        candidate
    };

    // Reject path-separator leakage and truncate for AccessKit budgets.
    let safe: String = basename
        .chars()
        .filter(|ch| *ch != '/' && *ch != '\\' && !ch.is_control())
        .take(ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS)
        .collect();
    if safe.is_empty() {
        "untitled".to_string()
    } else {
        safe
    }
}

pub(crate) fn dirty_marker(dirty: bool) -> &'static str {
    if dirty { " Dirty." } else { "" }
}

pub(crate) fn composing_marker(composing: bool) -> &'static str {
    if composing { " Composing." } else { "" }
}

pub(crate) fn theme_marker(theme_label: &str) -> String {
    format!(" Theme {theme_label}.")
}

pub(crate) fn truncate_accessibility_text(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out: String = value.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

pub(crate) fn sanitize_recovery_summary(summary: &str) -> Option<String> {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return None;
    }
    let safe: String = trimmed.chars().filter(|ch| !ch.is_control()).collect();
    let truncated = truncate_accessibility_text(&safe, ACCESSIBILITY_RECOVERY_SUMMARY_MAX_CHARS);
    if truncated.is_empty() {
        None
    } else {
        Some(truncated)
    }
}

/// Build one bounded accessible label for a transient-menu item.
///
/// Display/action data stays unchanged; only the semantic MenuItem label is
/// normalized. Separators and controls are removed, invalid/empty input falls
/// back to the display label and then `Menu item`, and the selected suffix is
/// included inside the same 256-character ceiling.
pub(crate) fn compose_menu_item_accessibility_label(
    accessibility_label: &str,
    display_label: &str,
    selected: bool,
) -> String {
    const SELECTED_SUFFIX: &str = " selected";

    let sanitize = |value: &str| {
        let without_separators: String = value
            .chars()
            .filter(|ch| !ch.is_control() && *ch != '/' && *ch != '\\')
            .collect();
        sanitize_recovery_summary(&without_separators)
    };
    let base = sanitize(accessibility_label)
        .or_else(|| sanitize(display_label))
        .unwrap_or_else(|| "Menu item".to_string());
    if selected {
        let max_base = ACCESSIBILITY_RECOVERY_SUMMARY_MAX_CHARS
            .saturating_sub(SELECTED_SUFFIX.chars().count());
        format!(
            "{}{SELECTED_SUFFIX}",
            truncate_accessibility_text(&base, max_base)
        )
    } else {
        truncate_accessibility_text(&base, ACCESSIBILITY_RECOVERY_SUMMARY_MAX_CHARS)
    }
}

pub(crate) fn pending_edits_summary(pending_edit_count: usize) -> Option<String> {
    if pending_edit_count == 0 {
        None
    } else if pending_edit_count == 1 {
        Some("Pending edits: 1.".to_string())
    } else {
        Some(format!("Pending edits: {pending_edit_count}."))
    }
}

pub(crate) fn compose_menu_result_count(result_count: usize) -> String {
    match result_count {
        1 => "1 result".to_string(),
        count => format!("{count} results"),
    }
}

pub(crate) fn compose_editor_accessibility_label(
    parts: EditorAccessibilityLabelParts<'_>,
) -> String {
    let composing = composing_marker(parts.composing);
    let theme = theme_marker(parts.theme_label);
    let recovery = parts
        .recovery_summary
        .and_then(sanitize_recovery_summary)
        .map(|summary| format!(" Recovery: {summary}"))
        .unwrap_or_default();
    if parts.visible_text.is_empty() {
        format!(
            "{placeholder}{composing}{theme}{recovery} {status}",
            placeholder = parts.empty_placeholder,
            status = parts.status_text
        )
    } else {
        format!(
            "{status}.{composing}{theme}{recovery} {text}",
            status = parts.status_text,
            text = parts.visible_text
        )
    }
}

pub(crate) fn compose_status_accessibility_label(
    status_text: &str,
    recovery_summary: Option<&str>,
) -> String {
    match recovery_summary.and_then(sanitize_recovery_summary) {
        Some(summary) => format!("{status_text} Recovery: {summary}"),
        None => status_text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_document_display_name_uses_basename_only() {
        assert_eq!(
            sanitize_document_display_name("/home/alice/secret/note.md"),
            "note.md"
        );
        assert_eq!(
            sanitize_document_display_name("src/nested/main.rs"),
            "main.rs"
        );
        assert_eq!(sanitize_document_display_name(""), "untitled");
        assert_eq!(sanitize_document_display_name("   "), "untitled");
        assert_eq!(sanitize_document_display_name(".."), "untitled");
        assert!(!sanitize_document_display_name("/tmp/../etc/passwd").contains('/'));
    }

    #[test]
    fn compose_editor_accessibility_label_marks_composing_and_recovery() {
        let label = compose_editor_accessibility_label(EditorAccessibilityLabelParts {
            status_text: "Clay — Connected — Editable — note.md — doc 7 — v1 — Dirty",
            theme_label: "default",
            composing: true,
            recovery_summary: Some("Reload or overwrite?"),
            visible_text: "hello",
            empty_placeholder: "Clay native text canvas.",
        });
        assert!(label.contains("Composing."));
        assert!(label.contains("Theme default."));
        assert!(label.contains("Recovery: Reload or overwrite?"));
        assert!(label.contains("Dirty"));
        assert!(label.ends_with(" hello"));
        assert!(!label.contains("/home/"));
    }

    #[test]
    fn pending_and_recovery_helpers_handle_empty_input() {
        assert_eq!(pending_edits_summary(0), None);
        assert_eq!(
            pending_edits_summary(2).as_deref(),
            Some("Pending edits: 2.")
        );
        assert_eq!(sanitize_recovery_summary("   "), None);
        assert!(
            sanitize_recovery_summary("Conflict\nreload")
                .unwrap()
                .contains("Conflict")
        );
        assert_eq!(compose_menu_result_count(0), "0 results");
        assert_eq!(compose_menu_result_count(1), "1 result");
        assert_eq!(compose_menu_result_count(2), "2 results");
    }

    #[test]
    fn menu_item_accessibility_labels_are_safe_and_bounded() {
        for length in [255, 256, 257] {
            let label = "x".repeat(length);
            let accessible = compose_menu_item_accessibility_label(&label, "fallback", false);
            assert!(accessible.chars().count() <= 256);
        }

        let selected = compose_menu_item_accessibility_label(&"x".repeat(256), "fallback", true);
        assert_eq!(selected.chars().count(), 256);
        assert!(selected.ends_with(" selected"));
        assert!(!selected.contains('/'));
        assert!(!selected.contains('\\'));

        assert_eq!(
            compose_menu_item_accessibility_label(
                "Open /home/user/secret\nfile",
                "fallback",
                false
            ),
            "Open homeusersecretfile"
        );
        assert_eq!(
            compose_menu_item_accessibility_label("/\\", "Fallback item", false),
            "Fallback item"
        );
        assert_eq!(
            compose_menu_item_accessibility_label("", "", false),
            "Menu item"
        );
    }
}
