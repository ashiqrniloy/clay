#!/usr/bin/env python3
"""Plan 133 task 5: replace the 21 hand-written closed-choice checks in
`src/server/ui.rs` with the `CHOICE_FIELDS` table plus one `validate_choice`.

Each site is located by its (unique) diagnostic message, so the script fails
loudly if the file does not match what it was written against.
"""

import pathlib
import re
import sys

PATH = pathlib.Path("src/server/ui.rs")

TABLE = '''
/// One row of [`CHOICE_FIELDS`]: `(key, allowed, rule, label)`.
///
/// `key` is the name [`validate_choice`] looks the row up by (`<family>.<field>`
/// for nested fields, `<layoutOverride>.<property>` for override values);
/// `label` is the diagnostic prefix that [`choice_message`] completes with the
/// allowed list, so a message can never drift from the values actually accepted.
type ChoiceRow = (
    &'static str,
    &'static [&'static str],
    UiContributionRule,
    &'static str,
);

/// Every closed string-choice field validated by [`validate_choice`]. Adding an
/// allowed value is one edit in the `VALID_*` slice the row points at: the
/// diagnostic text follows that slice.
const CHOICE_FIELDS: &[ChoiceRow] = &[
    ("slot", VALID_SLOTS, UiContributionRule::InvalidSlot, "panel slot must be one of"),
    (
        "defaultVisibility",
        VALID_VISIBILITY,
        UiContributionRule::InvalidPolicy,
        "panel defaultVisibility must be",
    ),
    (
        "anchor",
        VALID_OVERLAY_ANCHORS,
        UiContributionRule::InvalidPolicy,
        "overlay anchor must be one of",
    ),
    (
        "focusPolicy",
        VALID_FOCUS_POLICIES,
        UiContributionRule::InvalidPolicy,
        "overlay focusPolicy must be",
    ),
    (
        "dismissalPolicy",
        VALID_DISMISSAL_POLICIES,
        UiContributionRule::InvalidPolicy,
        "overlay dismissalPolicy must be",
    ),
    ("input.scope", VALID_INPUT_SCOPES, UiContributionRule::InvalidInputScope, "input scope must be"),
    (
        "pointer.click",
        VALID_POINTER_CLICK_POLICIES,
        UiContributionRule::InvalidPolicy,
        "pointer.click must be",
    ),
    (
        "pointer.drag",
        VALID_POINTER_DRAG_POLICIES,
        UiContributionRule::InvalidPolicy,
        "pointer.drag must be",
    ),
    (
        "focus.policy",
        VALID_COMPONENT_FOCUS_POLICIES,
        UiContributionRule::InvalidFocusPolicy,
        "focus.policy must be",
    ),
    (
        "selectionPolicy",
        VALID_SELECTION_POLICIES,
        UiContributionRule::InvalidPolicy,
        "selectionPolicy must be",
    ),
    (
        "state.scope",
        VALID_UI_STATE_SCOPES,
        UiContributionRule::InvalidStateScope,
        "UI state scope must be",
    ),
    (
        "state.owner",
        VALID_UI_STATE_OWNERS,
        UiContributionRule::InvalidLifecycle,
        "UI state owner must be",
    ),
    (
        "state.lifetime",
        VALID_UI_STATE_LIFETIMES,
        UiContributionRule::InvalidLifecycle,
        "UI state lifetime must be",
    ),
    (
        "state.persistence",
        VALID_UI_STATE_PERSISTENCE,
        UiContributionRule::InvalidLifecycle,
        "UI state persistence must be",
    ),
    (
        "implementationStatus",
        VALID_UI_STATE_STATUSES,
        UiContributionRule::InvalidLifecycle,
        "implementationStatus must be",
    ),
    (
        "valueSchema.kind",
        VALID_UI_STATE_SCHEMA_KINDS,
        UiContributionRule::InvalidStateSchema,
        "valueSchema.kind must be",
    ),
    (
        "layoutOverride.property",
        VALID_LAYOUT_OVERRIDE_PROPERTIES,
        UiContributionRule::InvalidLayoutOverride,
        "layout override property must be",
    ),
    (
        "layoutOverride.source",
        VALID_LAYOUT_OVERRIDE_SOURCES,
        UiContributionRule::InvalidLayoutOverride,
        "layout override source must be",
    ),
    ("layoutOverride.slot", VALID_SLOTS, UiContributionRule::InvalidSlot, "slot override value must be"),
    (
        "layoutOverride.visibility",
        VALID_VISIBILITY,
        UiContributionRule::InvalidPolicy,
        "visibility override value must be",
    ),
    (
        "layoutOverride.fallback",
        VALID_FALLBACK_BEHAVIORS,
        UiContributionRule::InvalidLayoutOverride,
        "fallback override value must be",
    ),
];

fn choice_field(key: &str) -> &'static ChoiceRow {
    CHOICE_FIELDS
        .iter()
        .find(|(row_key, ..)| *row_key == key)
        .unwrap_or_else(|| panic!("choice field `{key}` is not declared in CHOICE_FIELDS"))
}

/// Builds an out-of-set diagnostic: the row's label plus the allowed values
/// (`a, b, or c`).
fn choice_message(allowed: &[&str], label: &str) -> String {
    let Some((last, head)) = allowed.split_last() else {
        return label.to_owned();
    };
    if head.is_empty() {
        return format!("{label} {last}");
    }
    format!("{label} {} or {last}", head.join(", "))
}

/// The one validator behind every closed-choice field: allowed values,
/// diagnostic rule, and message all come from the [`CHOICE_FIELDS`] row.
fn validate_choice(
    key: &str,
    value: &str,
    id: &str,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    let (_, allowed, rule, label) = *choice_field(key);
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(context.error(rule, Some(id), choice_message(allowed, label)))
}
'''

SITES = {
    "panel slot must be one of left, right, top, or bottom":
        'validate_choice("slot", &slot, &id, &context)?;',
    "panel defaultVisibility must be visible, hidden, or collapsed":
        'validate_choice("defaultVisibility", &default_visibility, &id, &context)?;',
    "overlay anchor must be one of working-area, active-pane, main, or pointer":
        'validate_choice("anchor", &anchor, &id, &context)?;',
    "overlay focusPolicy must be none, restore, or trap":
        'validate_choice("focusPolicy", &focus_policy, &id, &context)?;',
    "overlay dismissalPolicy must be manual, escape, outside, or escape-or-outside":
        'validate_choice("dismissalPolicy", &dismissal_policy, &id, &context)?;',
    "input scope must be component, panel, or overlay":
        'validate_choice("input.scope", &scope, &id, &context)?;',
    "pointer.click must be none, focus, action, or select":
        'validate_choice("pointer.click", &pointer_click, &id, &context)?;',
    "pointer.drag must be none, select, or pan":
        'validate_choice("pointer.drag", &pointer_drag, &id, &context)?;',
    "focus.policy must be none, restore-editor, focus-component, or trap":
        'validate_choice("focus.policy", &focus_policy, &id, &context)?;',
    "selectionPolicy must be preserve-editor, component-local, or disabled":
        'validate_choice("selectionPolicy", &selection_policy, &id, &context)?;',
    "UI state scope must be package-global, user-config, workspace, document, pane, component, or transient-overlay":
        'validate_choice("state.scope", &scope, &id, &context)?;',
    "UI state owner must be package, shell, or server":
        'validate_choice("state.owner", &owner, &id, &context)?;',
    "UI state lifetime must be session, workspace, document, or transient":
        'validate_choice("state.lifetime", &lifetime, &id, &context)?;',
    "UI state persistence must be none, client-local, server-canonical, or deferred":
        'validate_choice("state.persistence", &persistence, &id, &context)?;',
    "implementationStatus must be implemented or deferred":
        'validate_choice("implementationStatus", &implementation_status, &id, &context)?;',
    "valueSchema.kind must be boolean, number, string, enum, or object":
        'validate_choice("valueSchema.kind", &value_schema_kind, &id, &context)?;',
    "layout override property must be slot, visibility, splitRatio, themeToken, inputDefault, actionDefault, or fallback":
        'validate_choice("layoutOverride.property", &property, &property, &context)?;',
    "layout override source must be user-config, active-major-mode, compatible-minor-mode, global-package, or package-default":
        'validate_choice("layoutOverride.source", source, source, &context)?;',
    "slot override value must be left, right, top, or bottom":
        'validate_choice("layoutOverride.slot", slot, slot, &context)?;',
    "visibility override value must be visible, hidden, or collapsed":
        'validate_choice("layoutOverride.visibility", visibility, visibility, &context)?;',
    "fallback override value must be package-default, hide, or ignore":
        'validate_choice("layoutOverride.fallback", fallback, fallback, &context)?;',
}

IF_LINE = re.compile(r'^(\s*)if !(VALID_\w+)\.contains\(&(\w+)\) \{$')


def main() -> int:
    source = PATH.read_text()
    lines = source.split("\n")

    for message, replacement in SITES.items():
        hits = [i for i, line in enumerate(lines) if f'"{message}"' in line]
        if len(hits) != 1:
            print(f"error: message {message!r} matched {len(hits)} lines", file=sys.stderr)
            return 1
        message_line = hits[0]
        # the `if` head is 1-6 lines above the message (rule, id, context.error)
        if_line = None
        for candidate in range(message_line - 1, message_line - 7, -1):
            if IF_LINE.match(lines[candidate]):
                if_line = candidate
                break
        if if_line is None:
            print(f"error: no `if !VALID_*.contains` head above {message!r}", file=sys.stderr)
            return 1
        indent = IF_LINE.match(lines[if_line]).group(1)
        # brace-match the if-block
        depth = 0
        end = if_line
        while end < len(lines):
            depth += lines[end].count("{") - lines[end].count("}")
            if depth == 0:
                break
            end += 1
        if depth != 0:
            print(f"error: unbalanced block for {message!r}", file=sys.stderr)
            return 1
        lines[if_line:end + 1] = [f"{indent}{replacement}"]

    merged = "\n".join(lines)
    anchor = 'const VALID_FALLBACK_BEHAVIORS: &[&str] = &["package-default", "hide", "ignore"];\n'
    if merged.count(anchor) != 1:
        print("error: VALID_FALLBACK_BEHAVIORS anchor not unique", file=sys.stderr)
        return 1
    merged = merged.replace(anchor, anchor + TABLE, 1)

    PATH.write_text(merged)
    print(f"rewrote {len(SITES)} validation sites; ui.rs now {len(merged.splitlines())} lines")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())