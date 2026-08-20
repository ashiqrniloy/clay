//! Shell accessibility virtual-node + announcement builders.
//!
//! Pure helpers for the shell accessibility pass: kurbo->AccessKit bounds
//! conversion (`accesskit_rect`, `node_window_size`) and the polite
//! live-region announcement builder (`compose_announcement`). The a11y tree
//! construction itself lives in `impl Widget for ClayShellWidget` (`mod.rs`),
//! which calls these helpers; stable virtual-node IDs come from
//! `crate::editor::accessibility`.

use masonry::accesskit::Node;
use masonry::kurbo::{Rect, Size};

use crate::perf::budgets::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

pub(super) fn node_window_size(node: &Node) -> Size {
    match node.bounds() {
        Some(bounds) => Size::new(bounds.width().max(0.0), bounds.height().max(0.0)),
        None => Size::ZERO,
    }
}

/// Phase 22.6: kurbo → AccessKit rect for virtual accessibility nodes.
pub(super) fn accesskit_rect(rect: Rect) -> masonry::accesskit::Rect {
    masonry::accesskit::Rect {
        x0: rect.x0,
        y0: rect.y0,
        x1: rect.x1,
        y1: rect.y1,
    }
}

/// Phase 22.6 (task 4): window-model actions that produce exactly one
/// polite live-region announcement each. One variant per user action keeps
/// the announcement strings in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnnouncementKind {
    TabActivated,
    TabCreated,
    TabClosed,
    SplitPaneVertical,
    SplitPaneHorizontal,
    PaneAdded,
    PaneClosed,
    PaneMovedForward,
    PaneMovedBackward,
}

/// Announcement length cap — the same budget constant menu labels use
/// (`src/perf/budgets.rs::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS`).
pub(crate) const ANNOUNCEMENT_MAX_CHARS: usize = TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

/// Shared announcement builder: O(1) (fixed-size inputs — a display name of
/// at most 64 chars plus two counts) and sanitized — the name passes
/// `sanitize_document_display_name`, so no host path, separator, or control
/// character can reach the live region.
pub(crate) fn compose_announcement(
    kind: AnnouncementKind,
    name: Option<&str>,
    position: usize,
    count: usize,
) -> String {
    let name = name
        .map(crate::editor::accessibility::sanitize_document_display_name)
        .unwrap_or_default();
    let text = match kind {
        AnnouncementKind::TabActivated => format!("Switched to tab {position}: {name}"),
        AnnouncementKind::TabCreated => format!("Opened tab {position}: {name}"),
        AnnouncementKind::TabClosed => {
            let tabs = if count == 1 { "tab" } else { "tabs" };
            format!("Closed tab {position}: {name}; {count} {tabs} open")
        }
        AnnouncementKind::SplitPaneVertical => "Split pane vertically".to_string(),
        AnnouncementKind::SplitPaneHorizontal => "Split pane horizontally".to_string(),
        AnnouncementKind::PaneAdded => "Added pane".to_string(),
        AnnouncementKind::PaneClosed => {
            let (pane, verb) = if count == 1 {
                ("pane", "remains")
            } else {
                ("panes", "remain")
            };
            format!("Closed pane; {count} {pane} {verb}")
        }
        AnnouncementKind::PaneMovedForward => "Moved pane forward".to_string(),
        AnnouncementKind::PaneMovedBackward => "Moved pane backward".to_string(),
    };
    if text.chars().count() > ANNOUNCEMENT_MAX_CHARS {
        text.chars().take(ANNOUNCEMENT_MAX_CHARS).collect()
    } else {
        text
    }
}
