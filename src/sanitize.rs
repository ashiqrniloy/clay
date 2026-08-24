//! Bounded renderer-neutral labels for frontend and accessibility projections.

use std::path::Path;

use crate::perf::budgets::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

pub(crate) const DISPLAY_NAME_MAX_CHARS: usize = 64;

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
    let basename = if candidate.is_empty() || candidate == "." || candidate == ".." {
        "untitled"
    } else {
        candidate
    };
    let safe: String = basename
        .chars()
        .filter(|ch| *ch != '/' && *ch != '\\' && !ch.is_control())
        .take(DISPLAY_NAME_MAX_CHARS)
        .collect();
    if safe.is_empty() {
        "untitled".to_string()
    } else {
        safe
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out: String = value.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

pub(crate) fn sanitize_summary(summary: &str) -> Option<String> {
    let safe: String = summary
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .collect();
    (!safe.is_empty()).then(|| truncate(&safe, TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS))
}

pub(crate) fn menu_item_label(label: &str, fallback: &str, selected: bool) -> String {
    const SELECTED: &str = " selected";
    let clean = |value: &str| {
        let value: String = value
            .chars()
            .filter(|ch| !ch.is_control() && *ch != '/' && *ch != '\\')
            .collect();
        sanitize_summary(&value)
    };
    let base = clean(label)
        .or_else(|| clean(fallback))
        .unwrap_or_else(|| "Menu item".to_string());
    if selected {
        let max = TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS - SELECTED.len();
        format!("{}{SELECTED}", truncate(&base, max))
    } else {
        truncate(&base, TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS)
    }
}

pub(crate) fn menu_result_count(count: usize) -> String {
    if count == 1 {
        "1 result".to_string()
    } else {
        format!("{count} results")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_hide_paths_and_controls() {
        assert_eq!(
            sanitize_document_display_name("/secret/root/note.md"),
            "note.md"
        );
        assert_eq!(menu_item_label("/secret\n", "Open", false), "secret");
    }
}
