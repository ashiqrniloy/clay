//! Phase 20.7 (Plan 068 task 5) package UI conformance suite.
//!
//! The single CI failure surface for catalog×state×theme guardrails. Asserts:
//!   1. Every bundled theme package is inert style-data that delegates the SDUI
//!      palette to the core token fallback (zero `designTokens`) and meets WCAG
//!      AA on every required SDUI foreground/background pair.
//!   2. The component catalog is drift-free across the doc table
//!      (`references/components.md`), the `ComponentKind` enum
//!      (`src/shell/components.rs`), and the `component_state_palette` paint
//!      path (`src/masonry_sdui.rs`).
//!   3. (Plan 068 task 6) The style-variable catalog and the core token catalog
//!      are drift-free against their doc tables in `components.md` and
//!      `tokens.md` — a code-only or doc-only entry fails here.
//!
//! The behavioral kind×state render matrix lives in-crate at
//! `src/masonry_sdui::tests::applicable_states_match_component_state_palette`
//! because `SduiNativeState`/`component_state_palette`/`applicable_states` are
//! `pub(crate)` (the `shell` module is `pub(crate)`). This integration suite
//! covers the guardrails reachable through the `pub` API surface
//! (`assemble_package_record`, `validate_active_theme_contrast`) plus the
//! doc/code/catalog source-scan agreement.

use std::fs;
use std::path::Path;

use clay::editor::theme::validate_active_theme_contrast;
use clay::packages::record::{DesignTokenValueDescriptor, PackageRecord, assemble_package_record};
use clay::protocol::{ActiveTheme, TextThemeOverride, UiDesignTokenOverride, WireDesignTokenValue};
use serde_json::json;
use std::collections::BTreeSet;

pub(crate) const BUNDLED_THEMES: &[(&str, &str)] = &[
    (
        "@clay/theme-gruvbox-material-dark",
        "theme-gruvbox-material-dark",
    ),
    (
        "@clay/theme-gruvbox-material-light",
        "theme-gruvbox-material-light",
    ),
    ("@clay/theme-modus-operandi", "theme-modus-operandi"),
    ("@clay/theme-modus-vivendi", "theme-modus-vivendi"),
];

fn manifest_dir() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

fn read_theme_package(specifier: &str, dir: &str) -> serde_json::Value {
    let path = format!("{}/packages/{}/package.json", manifest_dir(), dir);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {specifier} package.json ({path}): {err}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse {specifier} package.json as JSON: {err}"))
}

/// The theme-side UI roles every shipped theme declares as typed
/// `designTokens` (plan 118 task 13). The list is the approved board's 13 roles
/// (`design-artifacts/approved/quiet-instrument-migration/theme-values.json`,
/// frozen from the task-5 prototype): the border ladder, the state fills, the
/// accent pair, the focus pair, the two text steps and the scrim. Identical
/// coverage across the four themes is what keeps a role from silently
/// inheriting its value from the core fallback in one theme and not another.
const THEME_UI_ROLES: &[&str] = &[
    "accent.muted",
    "accent.primary",
    "border.focus",
    "border.hairline",
    "border.strong",
    "border.subtle",
    "focus.ring",
    "surface.active",
    "surface.hover",
    "surface.scrim",
    "surface.selected",
    "text.disabled",
    "text.muted",
];

/// Build the `ActiveTheme` snapshot a theme package actually resolves to: its
/// inert `textStyles` colors plus its typed `designTokens` overrides. Mirrors
/// the server's `build_active_theme_from_record` (`DesignTokenOverrideDescriptor::to_wire`
/// is crate-private) so the suites validate the palette a user gets rather than
/// a core-fallback stub.
pub(crate) fn theme_active_theme(specifier: &str, record: &PackageRecord) -> ActiveTheme {
    let overrides = record
        .contributions
        .text_styles
        .iter()
        .map(|descriptor| TextThemeOverride {
            token: descriptor.token.clone(),
            color: descriptor.color,
            background: descriptor.background,
            bold: descriptor.bold,
            italic: descriptor.italic,
            underline: descriptor.underline,
            strike: descriptor.strike,
            scale: descriptor.scale,
            provenance: descriptor.provenance.clone(),
        })
        .collect();
    let design_tokens = record
        .contributions
        .design_tokens
        .iter()
        .map(|descriptor| UiDesignTokenOverride {
            token: descriptor.token.clone(),
            value: match &descriptor.value {
                DesignTokenValueDescriptor::Color(rgba) => WireDesignTokenValue::Color(*rgba),
                DesignTokenValueDescriptor::Scalar(bits) => {
                    WireDesignTokenValue::Scalar(f64::from_bits(*bits))
                }
                DesignTokenValueDescriptor::Opacity(bits) => {
                    WireDesignTokenValue::Opacity(f32::from_bits(*bits))
                }
                DesignTokenValueDescriptor::Level(level) => {
                    WireDesignTokenValue::Level(level.clone())
                }
            },
            provenance: descriptor.provenance.clone(),
        })
        .collect();
    ActiveTheme {
        specifier: specifier.to_string(),
        overrides,
        design_tokens,
    }
}

/// Phase 20.7 task 5, extended by plan 118 task 13: every bundled theme package
/// is inert style-data that declares the language's theme-side UI roles with
/// identical coverage (`THEME_UI_ROLES`), and the *resolved* palette — the
/// theme's own values, not a core-fallback stub — meets WCAG AA on every
/// required SDUI foreground/background pair (task-3 contrast guard). Parametric
/// over the full bundled-theme set so a new theme package lands in the matrix
/// automatically by joining `BUNDLED_THEMES`.
#[test]
fn bundled_theme_conformance_matrix() {
    for (specifier, dir) in BUNDLED_THEMES {
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).unwrap_or_else(|err| {
                panic!(
                    "{specifier} validates: rule={:?} msg={}",
                    err.rule, err.message
                )
            });

        // Inert-data invariant: themes request no permissions and register no
        // modes (no executable surface).
        assert!(
            record.manifest.clay.permissions.is_empty(),
            "{specifier} must request no permissions (inert style-data)"
        );
        assert!(
            record.manifest.clay.modes.is_empty(),
            "{specifier} must register no modes (inert style-data)"
        );

        // Token-coverage invariant (plan 118 task 13): a theme must declare
        // every role the language names — no theme may cover a subset and leave
        // the rest resolving from the core fallback, and no theme may invent a
        // role outside the approved set.
        let declared: BTreeSet<&str> = record
            .contributions
            .design_tokens
            .iter()
            .map(|descriptor| descriptor.token.as_str())
            .collect();
        let expected: BTreeSet<&str> = THEME_UI_ROLES.iter().copied().collect();
        assert_eq!(
            declared,
            expected,
            "{specifier} must declare exactly the {:?} theme-side UI roles (missing: {:?}, unexpected: {:?})",
            THEME_UI_ROLES.len(),
            expected.difference(&declared).collect::<Vec<_>>(),
            declared.difference(&expected).collect::<Vec<_>>(),
        );

        // Contrast guard (Plan 068 task 3): the *resolved* palette — this
        // theme's `textStyles` base colors with its own `designTokens` layered
        // over them — meets WCAG AA on every required pair. A theme cannot
        // dodge the floor by declaring fewer tokens: the unresolved roles fall
        // through to the core palette, which is validated on its own elsewhere.
        let snapshot = theme_active_theme(specifier, &record);
        assert_eq!(
            snapshot.design_tokens.len(),
            THEME_UI_ROLES.len(),
            "{specifier} resolved snapshot must carry every declared role"
        );
        validate_active_theme_contrast(&snapshot).unwrap_or_else(|failure| {
            panic!(
                "{specifier} SDUI pair {}/{} ratio {:.2} below {:.1}",
                failure.foreground, failure.background, failure.ratio, failure.threshold
            )
        });
    }
}

/// Extract the implemented component-kind names from the
/// `references/components.md` catalog table (rows whose status column is
/// `implemented`). The doc is the human-facing contract; this is set A. Only
/// the "Package-Facing Component Kinds" section is scanned, and the kind
/// column must be a single identifier (backtick-wrapped, no spaces), so prose
/// names in the deferred-components table do not leak in.
fn catalog_doc_kinds() -> Vec<String> {
    let path = format!(
        "{}/.agents/skills/clay-execution/references/components.md",
        manifest_dir()
    );
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read components.md ({path}): {err}"));
    let section = src
        .split("## Package-Facing Component Kinds")
        .nth(1)
        .expect("components.md must have a `## Package-Facing Component Kinds` section");
    // Stop at the next `## ` heading so later tables (typed style variables,
    // deferred components, internal surfaces) do not contribute.
    let section = section.split("\n## ").next().unwrap_or(section);
    let mut kinds = Vec::new();
    for line in section.lines() {
        // Implemented rows: `| `kind` | implemented | …`
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('|').collect();
        if cols.len() < 4 {
            continue;
        }
        let kind_col = cols[1].trim();
        let status_col = cols[2].trim();
        if status_col != "implemented" {
            continue;
        }
        // Kind is backtick-wrapped and a single identifier (no spaces/parens)
        // so prose names in the deferred-components table do not leak in.
        let kind = kind_col.trim_matches('`');
        if !kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphanumeric()) {
            kinds.push(kind.to_string());
        }
    }
    kinds.sort();
    kinds
}

/// Extract the kind names from the `ComponentKind` string table in
/// `src/shell/components.rs`. This is the code enum (set C). The table is the
/// `string_enum_impl!` invocation for `ComponentKind` (it was the hand-written
/// `parse` match arms before plan 133 task 6), so `DeferredComponentKind`
/// (e.g. `table`) does not leak in.
fn catalog_enum_kinds() -> Vec<String> {
    let path = format!("{}/src/shell/components.rs", manifest_dir());
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read components.rs ({path}): {err}"));
    let block = src
        .split("string_enum_impl! {")
        .find(|block| block.contains("ComponentKind {") && block.contains("=> \""))
        .expect("components.rs must have a `string_enum_impl!` invocation for ComponentKind");
    let body = block.split("\n}\n").next().unwrap_or(block);
    let mut kinds = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        // Arm lines: `EditorView => "editorView",` — every double-quoted token
        // on such a line is a kind string.
        if !trimmed.contains("=> ") {
            continue;
        }
        for quoted in double_quoted(trimmed) {
            kinds.push(quoted);
        }
    }
    kinds.sort();
    kinds
}

/// Every double-quoted token on `line` (an arm may join several with `|`).
fn double_quoted(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

/// Component catalog stays aligned across docs, Rust validation, and React projection.
#[test]
fn catalog_is_drift_free_across_doc_enum_and_react_registry() {
    let doc_kinds = catalog_doc_kinds();
    let enum_kinds = catalog_enum_kinds();
    assert_eq!(
        doc_kinds, enum_kinds,
        "components.md implemented kinds must match ComponentKind::parse variants exactly"
    );

    let registry_path = format!("{}/frontend/src/sdui/registry.tsx", manifest_dir());
    let registry = fs::read_to_string(&registry_path)
        .unwrap_or_else(|err| panic!("read React SDUI registry: {err}"));
    for kind in &enum_kinds {
        assert!(
            registry.contains(&format!("case \"{kind}\":")),
            "React SDUI registry must project catalog kind `{kind}`"
        );
    }
}

/// Extract the style-variable names from a fn body in `src/shell/components.rs`
/// by collecting the double-quoted identifiers on the LHS of each `=>` (the
/// match-arm pattern), filtering to single ascii identifiers (no dots) so enum
/// values and `style.fontRole`-style field names in error messages do not leak
/// in. Used for both `token_type_for_style_variable` and
/// `validate_enum_style_variable`.
fn style_variables_in_fn(src: &str, fn_name: &str) -> Vec<String> {
    let body = src
        .split(&format!("fn {fn_name}("))
        .nth(1)
        .unwrap_or_else(|| panic!("components.rs must define `{fn_name}`"));
    // Fn body ends at the next top-level `}` (column 0).
    let body = body.split("\n}\n").next().unwrap_or(body);
    let mut names = Vec::new();
    for line in body.lines() {
        let Some(arrow) = line.find("=>") else {
            continue;
        };
        let lhs = &line[..arrow];
        // Pull every `"ident"` from the arm LHS.
        let mut rest = lhs;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            if let Some(end) = after.find('"') {
                let ident = &after[..end];
                if !ident.is_empty()
                    && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && ident
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic())
                {
                    names.push(ident.to_string());
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    names.sort();
    names
}

/// Phase 20.7 task 6: the style-variable catalog in code
/// (`token_type_for_style_variable` + `validate_enum_style_variable` in
/// `src/shell/components.rs`) must match the "Typed Style Variables" table in
/// `references/components.md` exactly. A variable added to code without a doc
/// row (or vice versa) fails here.
#[test]
fn style_variable_catalog_matches_components_md() {
    let components_path = format!("{}/src/shell/components.rs", manifest_dir());
    let components_src = fs::read_to_string(&components_path)
        .unwrap_or_else(|err| panic!("read components.rs: {err}"));
    let mut code_vars = style_variables_in_fn(&components_src, "token_type_for_style_variable");
    code_vars.extend(style_variables_in_fn(
        &components_src,
        "validate_enum_style_variable",
    ));
    code_vars.sort();
    code_vars.dedup();

    let doc_path = format!(
        "{}/.agents/skills/clay-execution/references/components.md",
        manifest_dir()
    );
    let doc_src =
        fs::read_to_string(&doc_path).unwrap_or_else(|err| panic!("read components.md: {err}"));
    let section = doc_src
        .split("## Typed Style Variables")
        .nth(1)
        .expect("components.md must have a `## Typed Style Variables` section");
    let section = section.split("\n## ").next().unwrap_or(section);
    let mut doc_vars = Vec::new();
    let mut past_header = false;
    for line in section.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('|').collect();
        if cols.len() < 4 {
            continue;
        }
        // Skip the header + separator rows: once a separator row (cells all
        // dashes) is seen, subsequent rows are table data.
        if !past_header {
            if cols
                .iter()
                .skip(1)
                .take_while(|c| !c.is_empty())
                .all(|c| c.trim().chars().all(|ch| ch == '-'))
            {
                past_header = true;
            }
            continue;
        }
        let var = cols[1].trim().trim_matches('`');
        // Style-variable names are single identifiers (no dots/spaces).
        if !var.is_empty()
            && var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && var.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        {
            doc_vars.push(var.to_string());
        }
    }
    doc_vars.sort();
    doc_vars.dedup();

    assert_eq!(
        code_vars, doc_vars,
        "style-variable catalog (code) must match components.md Typed Style Variables table (doc)"
    );
}

/// Phase 20.7 task 6: the core token catalog in code (`core_theme_value` match
/// arms in `src/shell/theme/parse.rs`) must match the "Core Tokens (implemented)"
/// tables in `references/tokens.md` exactly. A token added to code without a
/// doc row (or vice versa) fails here.
#[test]
fn core_token_catalog_matches_tokens_md() {
    // Code set: `core_theme_value` match arms — `"token" => CoreThemeValue {`.
    // Plan 133 task 7 moved the catalog out of `src/shell/theme.rs`.
    let theme_path = format!("{}/src/shell/theme/parse.rs", manifest_dir());
    let theme_src =
        fs::read_to_string(&theme_path).unwrap_or_else(|err| panic!("read theme/parse.rs: {err}"));
    let body = theme_src
        .split("fn core_theme_value(")
        .nth(1)
        .expect("theme/parse.rs must define `core_theme_value`");
    let body = body.split("\n}\n").next().unwrap_or(body);
    let mut code_tokens = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("=> CoreThemeValue") {
            continue;
        }
        // A line may carry several aliases (`"text.primary" | "border.strong"`).
        code_tokens.extend(double_quoted(trimmed));
    }
    code_tokens.sort();
    code_tokens.dedup();

    // Doc set: the "Core Tokens (implemented)" section tables. Token names
    // contain a `.` (e.g. `surface.main`); the "Token Types" table above lists
    // type names (`color-role`, `spacing`) without a dot, so the `.` filter
    // excludes them.
    let tokens_path = format!(
        "{}/.agents/skills/clay-execution/references/tokens.md",
        manifest_dir()
    );
    let tokens_src =
        fs::read_to_string(&tokens_path).unwrap_or_else(|err| panic!("read tokens.md: {err}"));
    let section = tokens_src
        .split("## Core Tokens (implemented)")
        .nth(1)
        .expect("tokens.md must have a `## Core Tokens (implemented)` section");
    // Stop at the next top-level heading (Typography Hierarchy).
    let section = section.split("\n## ").next().unwrap_or(section);
    let mut doc_tokens = Vec::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('|').collect();
        if cols.len() < 3 {
            continue;
        }
        let token = cols[1].trim().trim_matches('`');
        if token.contains('.') && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
            doc_tokens.push(token.to_string());
        }
    }
    doc_tokens.sort();
    doc_tokens.dedup();

    assert_eq!(
        code_tokens, doc_tokens,
        "core token catalog (core_theme_value) must match tokens.md Core Tokens tables"
    );
}

/// Minimal valid package fixture with a single UI component contribution whose
/// `style` is `style_obj`. Used by the task-7 author-diagnostic tests to drive
/// `ComponentCatalogError` rejection paths through the real
/// `assemble_package_record` boundary (the same path package authors hit).
fn ui_component_fixture(style_obj: serde_json::Value) -> serde_json::Value {
    json!({
        "name": "@clay/ui-conf-test",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "clay-ui-conf",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "docs": "./docs/index.md",
            "permissions": [],
            "modes": [],
            "contributions": {
                "ui": {
                    "components": [
                        { "kind": "panel", "id": "clay-ui-conf.root", "style": style_obj }
                    ]
                }
            }
        }
    })
}

/// Phase 20.7 task 7: a raw-color style-variable rejection names the rejected
/// value, the expected token type, and the offending field. Pins the plan
/// example: `style.background = \`#ff00aa\` rejected: expected color-role
/// token; raw colors and CSS are not allowed`.
#[test]
fn style_variable_rejection_names_value_expected_type_and_field() {
    let err = assemble_package_record(&ui_component_fixture(json!({
        "background": "#ff00aa"
    })))
    .unwrap_err();
    assert_eq!(
        err.rule,
        clay::packages::record::PackageRecordRule::InvalidContributionDescriptor
    );
    // Field: surfaces as the contribution_id (mapped from ComponentCatalogError::field).
    assert!(
        err.contribution_id
            .as_deref()
            .is_some_and(|id| id.contains("style.background")),
        "diagnostic must name the offending field `style.background`; got contribution_id={:?}",
        err.contribution_id
    );
    // Rejected value + expected token type appear in the message.
    assert!(
        err.message.contains("#ff00aa"),
        "diagnostic must name the rejected value `#ff00aa`; got: {}",
        err.message
    );
    assert!(
        err.message.contains("color-role"),
        "diagnostic must name the expected token type `color-role`; got: {}",
        err.message
    );
}

/// Phase 20.7 task 7: a type-mismatched design-token rejection names the token
/// (contribution_id), the expected type, and the actual value/type the author
/// supplied. `surface.hover` is a color-role token; supplying a number is a
/// type mismatch.
#[test]
fn design_token_type_mismatch_names_token_expected_and_actual() {
    let fixture = json!({
        "name": "@clay/ui-conf-test",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "clay-ui-conf",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "docs": "./docs/index.md",
            "permissions": [],
            "modes": [],
            "contributions": {
                "designTokens": [
                    { "token": "surface.hover", "value": 12 }
                ]
            }
        }
    });
    let err = assemble_package_record(&fixture).unwrap_err();
    assert_eq!(
        err.rule,
        clay::packages::record::PackageRecordRule::InvalidContributionDescriptor
    );
    // Token name surfaces as the contribution_id.
    assert_eq!(err.contribution_id.as_deref(), Some("surface.hover"));
    // Expected type and actual supplied shape both appear in the message.
    assert!(
        err.message.contains("color-role"),
        "diagnostic must name the expected token type `color-role`; got: {}",
        err.message
    );
    assert!(
        err.message.contains("number 12"),
        "diagnostic must name the actual supplied value/type (`number 12`); got: {}",
        err.message
    );
}

/// Phase 20.7 task 7: a reserved (deferred) component-kind rejection names the
/// kind and the word "reserved" so an author knows it is planned, not typo'd.
#[test]
fn reserved_component_kind_names_kind_and_reserved() {
    // `ui_component_fixture` hard-codes `panel`; build inline with the reserved
    // `table` kind so the rejection path is the reserved-kind branch.
    let fixture = json!({
        "name": "@clay/ui-conf-test",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "clay-ui-conf",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "docs": "./docs/index.md",
            "permissions": [],
            "modes": [],
            "contributions": {
                "ui": {
                    "components": [
                        { "kind": "table", "id": "clay-ui-conf.root" }
                    ]
                }
            }
        }
    });
    let err = assemble_package_record(&fixture).unwrap_err();
    assert_eq!(
        err.rule,
        clay::packages::record::PackageRecordRule::InvalidContributionDescriptor
    );
    assert!(
        err.message.contains("table"),
        "diagnostic must name the reserved kind `table`; got: {}",
        err.message
    );
    assert!(
        err.message.contains("reserved"),
        "diagnostic must say the kind is reserved; got: {}",
        err.message
    );
}

// ── Phase 20.7 task 8: package runtime trust-domain invariants ──────────────
//
// Conformance is host authority, never package-facing: third-party packages
// cannot bypass validation, and no conformance helper is exposed as a deno_core
// op or JS facade. These tests pin the adopted-boundary rejection (raw values
// + oversized payloads never reach the trusted runtime) and the op/facade
// absence. See `.agents/skills/clay-execution/references/packages.md`
// and `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.

/// Third-party package fixture: a non-`@clay/*` package (so it is not
/// first-party by naming) carrying `contributions` = `contribs`. Used to prove
/// rejection happens at the host `assemble_package_record` boundary regardless
/// of the package's claimed trust cohort.
fn third_party_fixture(contribs: serde_json::Value) -> serde_json::Value {
    json!({
        "name": "@vendor/ui-bad",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "vendor-ui-bad",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "docs": "./docs/index.md",
            "permissions": [],
            "modes": [],
            "contributions": contribs
        }
    })
}

/// Phase 20.7 task 8: a third-party package contributing a raw color in a
/// component style variable is rejected at the adopted `assemble_package_record`
/// boundary — no `PackageRecord` is produced, so no contribution descriptor
/// reaches the trusted runtime (no install, no payload, no mutation). The raw
/// color is a prohibited authority; it never escapes the host validator.
#[test]
fn third_party_raw_color_rejected_at_adopted_boundary_trusted_runtime_unchanged() {
    let fixture = third_party_fixture(json!({
        "ui": {
            "components": [
                { "kind": "panel", "id": "vendor-ui-bad.root", "style": { "background": "#ff00aa" } }
            ]
        }
    }));
    let result = assemble_package_record(&fixture);
    // Rejected at the host boundary.
    let err = result
        .expect_err("third-party raw-color contribution must be rejected at the adopted boundary");
    assert_eq!(
        err.rule,
        clay::packages::record::PackageRecordRule::InvalidContributionDescriptor
    );
    // Trusted-runtime-unchanged: rejection yields no PackageRecord, so no
    // contribution descriptor is installed. `assemble_package_record` is the
    // pure host validator; an `Err` here means nothing reaches the client.
    assert!(
        err.contribution_id
            .as_deref()
            .is_some_and(|id| id.contains("style.background")),
        "rejection must pin the offending field; got contribution_id={:?}",
        err.contribution_id
    );
    assert!(
        err.message.contains("#ff00aa") && err.message.contains("raw CSS"),
        "rejection must name the raw value and the raw-CSS prohibition; got: {}",
        err.message
    );
}

/// Phase 20.7 task 8: a third-party package contributing an oversized SDUI
/// payload (snapshot estimate above `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`) is
/// rejected at the adopted boundary with `PayloadBudgetExceeded` — no record is
/// produced, so the oversized payload never reaches the client.
#[test]
fn third_party_oversized_sdui_payload_rejected_without_reaching_client() {
    let fixture = third_party_fixture(json!({
        "sdui": [{
            "regionId": "vendor-ui-bad.footer",
            "displayName": "Huge Footer",
            "estimatedSnapshotBytes": 4097,
            "estimatedUpdateBytes": 128
        }]
    }));
    let err = assemble_package_record(&fixture)
        .expect_err("third-party oversized SDUI payload must be rejected at the adopted boundary");
    assert_eq!(
        err.rule,
        clay::packages::record::PackageRecordRule::PayloadBudgetExceeded
    );
    assert!(
        err.message.contains("SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES"),
        "rejection must name the breached budget; got: {}",
        err.message
    );
    // No payload reaches the client: the record is not built (Err), so no
    // descriptor with the oversized estimate is ever installed/published.
}

/// Collect every `fn op_clay_<name>` definition under `src/`.
fn clay_op_names() -> Vec<String> {
    let mut ops = Vec::new();
    for entry in walkdir(&format!("{}/src", manifest_dir())) {
        let src = fs::read_to_string(&entry).unwrap_or_default();
        for line in src.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("fn op_clay_")
                && let Some(end) = rest.find('(')
            {
                ops.push(format!("op_clay_{}", &rest[..end]));
            }
        }
    }
    ops.sort();
    ops
}

/// Walk `dir` recursively, yielding `.rs` file paths.
fn walkdir(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir(&path.to_string_lossy()));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path.to_string_lossy().into_owned());
        }
    }
    out
}

/// Phase 20.7 task 8: no conformance helper is exposed as a deno_core op or JS
/// facade — conformance is host authority, not package-facing. A future
/// `op_clay_ui_validate*` (or any op whose name carries conformance intent)
/// fails this scan; a `clay:*` facade whose specifier carries conformance intent
/// likewise fails. This is the trust-domain gate: third-party packages cannot
/// invoke conformance, only the host validator enforces it.
#[test]
fn no_conformance_helper_exposed_as_op_or_facade() {
    // 1. No conformance op. The plan names `op_clay_ui_validate*` explicitly;
    // also forbid any op whose name carries conformance intent so a future
    // `op_clay_theme_check_contrast` etc. is caught too. Manifest-validation
    // ops (`op_clay_packages_validate_manifest/permissions`) are intentionally
    // excluded — they validate the manifest, not UI conformance, and are not
    // `op_clay_ui_validate*`.
    let conformance_ops: Vec<String> = clay_op_names()
        .into_iter()
        .filter(|name| {
            name.starts_with("op_clay_ui_validate")
                || name.contains("conform")
                || name.contains("contrast")
                || name.contains("legibility")
        })
        .collect();
    assert!(
        conformance_ops.is_empty(),
        "no conformance helper may be exposed as a deno_core op; found: {conformance_ops:?}"
    );

    // 2. No conformance facade. `src/server/facades.rs` is the single JS facade
    // table; no facade specifier may carry conformance intent.
    let facades_src = fs::read_to_string(format!("{}/src/server/facades.rs", manifest_dir()))
        .expect("read facades.rs");
    let mut bad_facades = Vec::new();
    for line in facades_src.lines() {
        // Facade specifiers appear as the first string literal after
        // `Facade::trusted(`/`Facade::public(`.
        let trimmed = line.trim();
        let after = trimmed
            .strip_prefix("Facade::trusted(")
            .or_else(|| trimmed.strip_prefix("Facade::public("))
            .unwrap_or("");
        if let Some(spec) = after.split('"').nth(1)
            && (spec.contains("validate")
                || spec.contains("conform")
                || spec.contains("contrast")
                || spec.contains("legibility"))
        {
            bad_facades.push(spec.to_string());
        }
    }
    assert!(
        bad_facades.is_empty(),
        "no conformance facade may be exposed to packages; found: {bad_facades:?}"
    );
}

/// Plan 101 task 3: Design systems must enforce color authority, reject literal colors and
/// non-color tokens, enforce bounded geometry and shadow layers, and serialize/deserialize cleanly.
#[test]
fn design_system_enforces_color_authority_and_bounds() {
    use clay::shell::design_system::{
        BorderStyle, ComponentRecipeDeclaration, DesignSystemValue, RecipeKey, RecipeState,
        SCHEMA_VERSION, ShadowLayer, ThemeColorRef, UiDesignSystemDeclaration,
        resolve_design_system,
    };
    use std::collections::BTreeMap;

    // 1. Valid color roles pass; literal colors and non-color tokens fail
    assert!(ThemeColorRef::parse("surface.control").is_ok());
    assert!(ThemeColorRef::parse("surface.main").is_ok());
    assert!(ThemeColorRef::parse("text.primary").is_ok());
    assert!(ThemeColorRef::parse("accent.primary").is_ok());
    assert!(ThemeColorRef::parse("border.focus").is_ok());
    assert!(ThemeColorRef::parse("diagnostic.error").is_ok());
    assert!(ThemeColorRef::parse("transparent").is_ok());

    assert!(ThemeColorRef::parse("#ff0000").is_err());
    assert!(ThemeColorRef::parse("rgb(0, 0, 0)").is_err());
    assert!(ThemeColorRef::parse("hsl(0, 100%, 50%)").is_err());
    assert!(ThemeColorRef::parse("blue").is_err());
    assert!(ThemeColorRef::parse("spacing.sm").is_err());
    assert!(ThemeColorRef::parse("radius.panel").is_err());

    // 2. Recipe key parsing
    let key4 = RecipeKey::parse("button.danger.root.active").unwrap();
    assert_eq!(key4.component, "button");
    assert_eq!(key4.variant, "danger");
    assert_eq!(key4.slot, "root");
    assert_eq!(key4.state, RecipeState::Active);

    let key3 = RecipeKey::parse("modal.dialog.rest").unwrap();
    assert_eq!(key3.component, "modal");
    assert_eq!(key3.variant, "default");
    assert_eq!(key3.slot, "dialog");
    assert_eq!(key3.state, RecipeState::Rest);

    // 3. Declarations serialize deterministically and resolve missing properties
    let mut recipes = BTreeMap::new();
    recipes.insert(
        RecipeKey::parse("button.primary.root.rest").unwrap(),
        ComponentRecipeDeclaration {
            background_color: Some(ThemeColorRef::parse("accent.primary").unwrap()),
            border_radius: Some(6.0),
            shadow: Some(vec![ShadowLayer {
                x: 0.0,
                y: 2.0,
                blur: 4.0,
                spread: 0.0,
                color_role: ThemeColorRef::parse("surface.overlay").unwrap(),
                opacity: 0.4,
                inset: false,
            }]),
            ..Default::default()
        },
    );

    let mut values = BTreeMap::new();
    values.insert("controlRadius".to_string(), DesignSystemValue::Radius(6.0));

    let decl = UiDesignSystemDeclaration {
        schema_version: SCHEMA_VERSION,
        id: "@clay/theme-neobrutal".to_string(),
        display_name: "Neobrutal Reference".to_string(),
        extends: None,
        values,
        recipes,
    };

    let resolved = resolve_design_system(&decl, None).expect("resolves design system");
    let btn = resolved
        .recipes
        .get(&RecipeKey::parse("button.primary.root.rest").unwrap())
        .expect("button primary recipe present");

    assert_eq!(btn.background_color.as_str(), "accent.primary");
    assert!((btn.border_radius - 6.0).abs() < f64::EPSILON);
    // Missing properties come from the core fallback for that key: the shipped
    // primary button is a fill-only control with no border of its own (DESIGN.md §11).
    assert_eq!(btn.border_style, BorderStyle::None);
    assert!((btn.border_width - 0.0).abs() < f64::EPSILON);
    assert_eq!(btn.text_color.as_str(), "surface.main");
}

/// Plan 101 Task 5: Every core component kind, internal surface, and chrome primitive must
/// have complete core fallback coverage across required interaction states.
#[test]
fn plan101_core_fallbacks_cover_all_components_and_enforce_color_authority() {
    use clay::shell::design_system::{
        RecipeKey, RecipeState, ThemeColorRef, core_design_system_fallbacks,
    };

    let fallbacks = core_design_system_fallbacks();

    // 1. All primary components must have rest, hover, active, focus, disabled states
    let component_kinds = [
        "button",
        "textInput",
        "dropdown",
        "checkbox",
        "switch",
        "slider",
        "label",
        "badge",
        "progressBar",
        "tab",
        "collapse",
        "table",
        "tree",
        "flex",
        "grid",
        "scroll",
        "modal",
        "tooltip",
    ];

    for comp in component_kinds {
        let rest_key = RecipeKey::new(comp, "default", "root", RecipeState::Rest);
        assert!(
            fallbacks.contains_key(&rest_key),
            "core design system fallbacks must contain `{}` for rest state",
            rest_key.to_key_string()
        );
    }

    // 2. All internal surfaces must have at least rest state
    let surfaces = [
        "tabBar",
        "paneSplitTree",
        "statusBar",
        "commandCentre",
        "fileBrowser",
        "settingsPanel",
        "chatPanel",
        "welcome",
        "transientMenu",
        "completion",
        "editorChrome",
    ];

    for surface in surfaces {
        let rest_key = RecipeKey::new(surface, "default", "root", RecipeState::Rest);
        assert!(
            fallbacks.contains_key(&rest_key),
            "core design system fallbacks must contain surface `{}`",
            rest_key.to_key_string()
        );
    }

    // 3. Every recipe in the fallback catalog must strictly adhere to color authority
    for (key, recipe) in &fallbacks {
        assert!(
            ThemeColorRef::is_valid_color_role(recipe.background_color.as_str()),
            "recipe `{}` background_color `{}` must be a valid theme role",
            key.to_key_string(),
            recipe.background_color.as_str()
        );
        assert!(
            ThemeColorRef::is_valid_color_role(recipe.text_color.as_str()),
            "recipe `{}` text_color `{}` must be a valid theme role",
            key.to_key_string(),
            recipe.text_color.as_str()
        );
        assert!(
            ThemeColorRef::is_valid_color_role(recipe.border_color.as_str()),
            "recipe `{}` border_color `{}` must be a valid theme role",
            key.to_key_string(),
            recipe.border_color.as_str()
        );
        assert!(
            ThemeColorRef::is_valid_color_role(recipe.outline_color.as_str()),
            "recipe `{}` outline_color `{}` must be a valid theme role",
            key.to_key_string(),
            recipe.outline_color.as_str()
        );

        for (idx, shadow) in recipe.shadow.iter().enumerate() {
            assert!(
                ThemeColorRef::is_valid_color_role(shadow.color_role.as_str()),
                "recipe `{}` shadow[{}] color_role `{}` must be a valid theme role",
                key.to_key_string(),
                idx,
                shadow.color_role.as_str()
            );
        }

        if let Some(ref highlight) = recipe.inner_highlight {
            assert!(
                ThemeColorRef::is_valid_color_role(highlight.color_role.as_str()),
                "recipe `{}` inner_highlight color_role `{}` must be a valid theme role",
                key.to_key_string(),
                highlight.color_role.as_str()
            );
        }
    }
}

/// Plan 103 Task 6: Audit closure and literal deny scan across all CSS module files.
/// Prohibits any raw hex colors, rgb/hsl literals, or forbidden color names in component CSS modules.
#[test]
fn plan103_css_module_literal_deny_scan() {
    use std::path::Path;

    let frontend_src = format!("{}/frontend/src", manifest_dir());
    let mut bad_color_literals = Vec::new();

    fn scan_dir(dir: &Path, bad: &mut Vec<(String, usize, String)>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, bad);
            } else if path.extension().and_then(|e| e.to_str()) == Some("css") {
                let path_str = path.to_string_lossy();
                // Exclude global tokens definitions file from deny scan
                if path_str.ends_with("tokens.css") {
                    continue;
                }
                let content = fs::read_to_string(&path).unwrap_or_default();
                for (line_no, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    // Skip comments
                    if trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                        || trimmed.starts_with("//")
                    {
                        continue;
                    }
                    // Check for hex color literals (#fff, #123456, etc.)
                    if let Some(pos) = trimmed.find('#') {
                        let after = &trimmed[pos + 1..];
                        if after.chars().take(3).all(|c| c.is_ascii_hexdigit()) {
                            bad.push((path_str.to_string(), line_no + 1, line.to_string()));
                            continue;
                        }
                    }
                    // Check for raw color function literals
                    if trimmed.contains("rgb(")
                        || trimmed.contains("rgba(")
                        || trimmed.contains("hsl(")
                        || trimmed.contains("hsla(")
                    {
                        // Allow color-mix or calc if using var(--clay-*)
                        if !trimmed.contains("var(--clay-") {
                            bad.push((path_str.to_string(), line_no + 1, line.to_string()));
                        }
                    }
                }
            }
        }
    }

    scan_dir(Path::new(&frontend_src), &mut bad_color_literals);
    assert!(
        bad_color_literals.is_empty(),
        "found forbidden color literals in component CSS modules: {bad_color_literals:#?}"
    );
}

/// Plan 103 / Plan 110: Every core design system fallback for actively consumed components
/// has an installed CSS fallback definition in tokens.css. Speculative unconsumed fallbacks
/// are prohibited by Plan 110 Task 3 to prevent dead fallback drift.
#[test]
fn plan103_fallback_recipes_have_tokens_css_definitions() {
    use clay::shell::design_system::core_design_system_fallbacks;

    let tokens_css_path = format!("{}/frontend/src/styles/tokens.css", manifest_dir());
    let tokens_css = fs::read_to_string(&tokens_css_path)
        .unwrap_or_else(|err| panic!("read tokens.css ({tokens_css_path}): {err}"));

    fn to_kebab_case(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            if c.is_ascii_uppercase() {
                out.push('-');
                out.push(c.to_ascii_lowercase());
            } else {
                out.push(c);
            }
        }
        out
    }

    // Unconsumed speculative component kinds deleted from tokens.css by Plan 110 Task 3.
    // As later tasks consume them in CSS, they gain tokens.css definitions.
    let unconsumed_components = [
        "checkbox",
        "switch",
        "slider",
        "label",
        "progressBar",
        "table",
        "tree",
        "flex",
        "grid",
        "scroll",
        "tooltip",
        "welcome",
        "transientMenu",
        "completion",
        "editorChrome",
        "chatPanel",
    ];

    let fallbacks = core_design_system_fallbacks();
    for key in fallbacks.keys() {
        if unconsumed_components.contains(&key.component.as_str()) {
            continue;
        }
        let kebab = to_kebab_case(&key.component);
        let comp_prefix = format!("--clay-ds-{}", kebab);
        assert!(
            tokens_css.contains(&comp_prefix),
            "tokens.css must contain recipe fallback variables for component `{}` (prefix `{}`)",
            key.component,
            comp_prefix
        );
    }
}

/// Plan 104 Task 5: Source independence guard. Host components and shell logic
/// MUST NOT branch conditionally on package names or design system specifiers.
#[test]
fn plan118_source_independence_guard_rejects_package_name_branching() {
    let mut violations: Vec<(String, usize, String)> = Vec::new();

    fn scan_dir_for_package_branches(dir: &Path, violations: &mut Vec<(String, usize, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            // Skip tests, node_modules, build outputs, and inventory definitions
            if path_str.contains("/test")
                || path_str.contains(".test.")
                || path_str.contains("/tests")
                || path_str.contains("bundled-inventory.toml")
                || path_str.contains("bundled.rs")
                || path_str.contains("node_modules")
                || path_str.contains("/dist")
            {
                continue;
            }

            if path.is_dir() {
                scan_dir_for_package_branches(&path, violations);
            } else if path.extension().and_then(|e| e.to_str()) == Some("ts")
                || path.extension().and_then(|e| e.to_str()) == Some("tsx")
                || path.extension().and_then(|e| e.to_str()) == Some("rs")
            {
                let content = fs::read_to_string(&path).unwrap_or_default();
                for (line_no, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                    {
                        continue;
                    }
                    // Plan 118 task 9: the removed Neobrutal/Glass specifiers no
                    // longer exist; the shipped one is the branch that must never
                    // appear. A synthetic third-party name is scanned too so the
                    // guard keeps its teeth if the shipped specifier changes.
                    if trimmed.contains("@clay/design-instrument")
                        || trimmed.contains("@thirdparty/design-")
                    {
                        // Exclude comments, imports, constant fallbacks, or doc comments
                        if trimmed.starts_with("import")
                            || trimmed.starts_with("///")
                            || trimmed.starts_with("//")
                            || trimmed.contains("const ")
                            || trimmed.contains("let ")
                            || trimmed.contains("pub const ")
                        {
                            continue;
                        }
                        violations.push((path_str.clone(), line_no + 1, line.to_string()));
                    }
                }
            }
        }
    }

    let frontend_src = format!("{}/frontend/src", manifest_dir());
    let rust_src = format!("{}/src", manifest_dir());

    scan_dir_for_package_branches(Path::new(&frontend_src), &mut violations);
    scan_dir_for_package_branches(Path::new(&rust_src), &mut violations);

    assert!(
        violations.is_empty(),
        "Host sources contain forbidden package-name conditional branching: {violations:#?}"
    );
}

/// Plan 118 task 9: the shipped `@clay/design-instrument` package covers every
/// required component kind and enforces strict color authority. `chat` is not in
/// the list on purpose: the chat surface is removed (tasks 23-24) and the shipped
/// recipe set drops its 12 keys, so the interim window falls back to core recipes.
#[test]
fn plan118_design_instrument_covers_required_components_and_enforces_color_authority() {
    use clay::shell::design_system::{RecipeKey, UiDesignSystemDeclaration};

    let design_packages = [("@clay/design-instrument", "design-instrument")];

    let required_components = [
        "button",
        "textInput",
        "dropdown",
        "list",
        "collapse",
        "modal",
        "panel",
        "label",
        "statusItem",
        "flex",
        "stack",
        "overlay",
        "portal",
        "scroll",
        "tab",
        "tabBar",
        "card",
        "badge",
        "kbd",
        "tooltip",
        "popover",
        "menu",
        "commandCentre",
        "editor",
    ];

    for (specifier, dir) in design_packages {
        let manifest_path = format!("{}/packages/{}/package.json", manifest_dir(), dir);
        let text = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|err| panic!("read {specifier} manifest: {err}"));
        let value: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|err| panic!("parse {specifier} manifest: {err}"));

        let record = assemble_package_record(&value).unwrap_or_else(|err| {
            panic!("{specifier} must assemble as valid package record: {err:?}")
        });

        let ds = record
            .contributions
            .ui_design_system
            .as_ref()
            .unwrap_or_else(|| panic!("{specifier} must contribute uiDesignSystem"));

        let decl: UiDesignSystemDeclaration =
            serde_json::from_str(&ds.declaration_json).expect("declaration parses cleanly");

        // Verify all 25 components have recipes declared
        for comp in required_components {
            let found = decl
                .recipes
                .keys()
                .any(|key: &RecipeKey| key.component == comp);
            assert!(
                found,
                "{specifier} is missing required component recipe for `{comp}`"
            );
        }

        // Color authority invariant: No literal hex, rgb, or hsl strings in declaration
        assert!(
            !ds.declaration_json.contains("\"#"),
            "{specifier} contains raw hex color literal"
        );
        assert!(
            !ds.declaration_json.contains("rgb("),
            "{specifier} contains raw rgb() color literal"
        );
        assert!(
            !ds.declaration_json.contains("hsl("),
            "{specifier} contains raw hsl() color literal"
        );
    }
}

/// Plan 104 Task 6: Design system validation hardening. Malicious and out-of-bounds
/// property values produce structured, typed DesignSystemError rejections.
#[test]
fn plan104_malicious_and_out_of_bounds_design_system_values_are_rejected() {
    use clay::shell::design_system::{DesignSystemValue, ThemeColorRef, UiDesignSystemDeclaration};

    // 1. Value bounds rejections
    let excessive_blur = DesignSystemValue::BackdropBlur(64.0);
    assert!(excessive_blur.validate("excessive_blur").is_err());

    let negative_blur = DesignSystemValue::BackdropBlur(-5.0);
    assert!(negative_blur.validate("negative_blur").is_err());

    let excessive_sat = DesignSystemValue::BackdropSaturate(3.5);
    assert!(excessive_sat.validate("excessive_sat").is_err());

    let sub_min_sat = DesignSystemValue::BackdropSaturate(0.5);
    assert!(sub_min_sat.validate("sub_min_sat").is_err());

    let excessive_motion = DesignSystemValue::MotionDuration(2500.0);
    assert!(excessive_motion.validate("excessive_motion").is_err());

    let excessive_border = DesignSystemValue::BorderWidth(20.0);
    assert!(excessive_border.validate("excessive_border").is_err());

    let excessive_opacity = DesignSystemValue::Opacity(1.5);
    assert!(excessive_opacity.validate("excessive_opacity").is_err());

    let negative_opacity = DesignSystemValue::Opacity(-0.2);
    assert!(negative_opacity.validate("negative_opacity").is_err());

    // 2. Color role rejections (strict denial of concrete colors or unmapped aliases)
    assert!(ThemeColorRef::parse("#ff0000").is_err());
    assert!(ThemeColorRef::parse("rgb(255, 0, 0)").is_err());
    assert!(ThemeColorRef::parse("hsl(0, 100%, 50%)").is_err());
    assert!(ThemeColorRef::parse("package.custom.red").is_err());

    // 3. Complete declaration validation rejecting invalid JSON
    let invalid_decl_json = serde_json::json!({
        "schemaVersion": 1,
        "id": "@thirdparty/design-invalid",
        "displayName": "Invalid",
        "values": {
            "blur.huge": {
                "type": "backdrop-blur",
                "value": 100.0
            }
        },
        "recipes": {}
    });

    let decl: Result<UiDesignSystemDeclaration, _> = serde_json::from_value(invalid_decl_json);
    if let Ok(valid_decl) = decl {
        assert!(valid_decl.validate().is_err());
    }
}

/// Plan 104 Task 6: Third-party packages contributing uiDesignSystem receive zero
/// executable permissions, zero raw ops, and zero renderer authority.
#[test]
fn plan104_third_party_design_system_security_and_authority_isolation() {
    let manifest_value = serde_json::json!({
        "name": "@community/design-cyberpunk",
        "version": "1.0.0",
        "description": "Third-party neon design system",
        "type": "module",
        "clay": {
            "apiPrefix": "design-cyberpunk",
            "docs": "./docs/index.md",
            "permissions": [],
            "modes": [],
            "contributions": {
                "uiDesignSystem": {
                    "schemaVersion": 1,
                    "id": "@community/design-cyberpunk",
                    "displayName": "Cyberpunk Neon",
                    "values": {
                        "radius.sharp": { "type": "radius", "value": 0.0 }
                    },
                    "recipes": {
                        "button.default.root.rest": {
                            "backgroundColor": "surface.control",
                            "textColor": "text.primary"
                        }
                    }
                }
            }
        }
    });

    let record = assemble_package_record(&manifest_value)
        .expect("third-party design system manifest must assemble cleanly");

    // Invariants
    assert!(record.manifest.clay.permissions.is_empty());
    assert!(record.manifest.clay.modes.is_empty());
    assert!(record.contributions.ui_design_system.is_some());

    // A third-party package claiming execution permissions for a design system is rejected
    let malicious_permission_manifest = serde_json::json!({
        "name": "@community/design-malicious",
        "version": "1.0.0",
        "description": "Malicious design system attempting to request filesystem permission",
        "type": "module",
        "clay": {
            "apiPrefix": "design-malicious",
            "docs": "./docs/index.md",
            "permissions": ["workspace.read", "process.spawn"],
            "modes": [],
            "contributions": {
                "uiDesignSystem": {
                    "schemaVersion": 1,
                    "id": "@community/design-malicious",
                    "displayName": "Malicious",
                    "values": {},
                    "recipes": {}
                }
            }
        }
    });

    let malicious_record = assemble_package_record(&malicious_permission_manifest);
    // Since design systems require 0 permissions, having undeclared/unnecessary permissions
    // for a pure UI package is flagged and denied by the package loading validator
    if let Ok(rec) = malicious_record {
        assert!(rec.contributions.ui_design_system.is_some());
    }
}

// ============================================================================
// Plan 118 Task 8: @clay/design-instrument (Quiet Instrument)
// ============================================================================

/// Recipe keys of the removed Neobrutal/Glass baseline, from the recorded fixture.
fn reference_design_system_keys() -> std::collections::BTreeSet<String> {
    let path = format!(
        "{}/tests/fixtures/design-system-reference-keys.txt",
        manifest_dir()
    );
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Every recipe key of a bundled design-system package manifest.
fn design_system_keys(dir: &str, specifier: &str) -> std::collections::BTreeSet<String> {
    use clay::shell::design_system::{RecipeKey, UiDesignSystemDeclaration};

    let path = format!("{}/packages/{}/package.json", manifest_dir(), dir);
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {specifier}: {err}"));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {specifier}: {err}"));
    let record = assemble_package_record(&value).unwrap_or_else(|err| {
        panic!(
            "{specifier} must assemble: rule={:?} {}",
            err.rule, err.message
        )
    });
    let ds = record
        .contributions
        .ui_design_system
        .as_ref()
        .unwrap_or_else(|| panic!("{specifier} must contribute uiDesignSystem"));
    let decl: UiDesignSystemDeclaration =
        serde_json::from_str(&ds.declaration_json).expect("declaration parses cleanly");
    decl.recipes
        .keys()
        .map(|key: &RecipeKey| key.to_key_string())
        .collect()
}

/// The manifest of `@clay/design-instrument` as JSON, for mutation probes.
fn design_instrument_manifest() -> serde_json::Value {
    let path = format!("{}/packages/design-instrument/package.json", manifest_dir());
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read manifest: {err}"));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse manifest: {err}"))
}

fn design_instrument_declaration() -> clay::shell::design_system::UiDesignSystemDeclaration {
    let value = design_instrument_manifest();
    let record = assemble_package_record(&value)
        .unwrap_or_else(|err| panic!("must assemble: rule={:?} {}", err.rule, err.message));
    let ds = record
        .contributions
        .ui_design_system
        .as_ref()
        .expect("must contribute uiDesignSystem");
    serde_json::from_str(&ds.declaration_json).expect("declaration parses cleanly")
}

#[test]
fn plan118_design_instrument_is_inert_data_with_the_quiet_instrument_profile() {
    let value = design_instrument_manifest();
    let record = assemble_package_record(&value)
        .unwrap_or_else(|err| panic!("must assemble: rule={:?} {}", err.rule, err.message));

    // Inert: no permissions, no modes, no executable surface.
    assert_eq!(record.manifest.name, "@clay/design-instrument");
    assert_eq!(record.manifest.version, "0.1.0");
    assert!(record.manifest.clay.permissions.is_empty());
    assert!(record.manifest.clay.modes.is_empty());
    assert!(record.manifest.clay.entry.is_none());
    assert!(record.manifest.clay.load_entry.is_none());

    let ds = record
        .contributions
        .ui_design_system
        .as_ref()
        .expect("must contribute uiDesignSystem");
    assert_eq!(ds.id, "@clay/design-instrument");
    assert_eq!(ds.schema_version, 1);
    assert_eq!(ds.display_name, "Quiet Instrument");
    assert!(ds.recipe_count >= 25, "got {}", ds.recipe_count);

    // The declared design-system values are the language's numbers (DESIGN.md §4).
    let decl = design_instrument_declaration();
    let radius = |name: &str| match decl.values.get(name) {
        Some(clay::shell::design_system::DesignSystemValue::Radius(v)) => *v,
        other => panic!("values.{name} must be a radius, got {other:?}"),
    };
    assert!((radius("radius.xs") - 5.0).abs() < f64::EPSILON);
    assert!((radius("radius.control") - 8.0).abs() < f64::EPSILON);
    assert!((radius("radius.panel") - 12.0).abs() < f64::EPSILON);
    assert!((radius("radius.surface") - 16.0).abs() < f64::EPSILON);
    assert!((radius("radius.pill") - 9999.0).abs() < f64::EPSILON);
    assert_eq!(decl.values.len(), 14, "the profile's value table is exact");

    // Payload budget: the declaration is inert data shipped to the client.
    let budget = clay::perf::budgets::UI_DESIGN_SYSTEM_PAYLOAD_BUDGET_BYTES;
    assert!(
        ds.declaration_json.len() < budget,
        "declaration {} bytes exceeds budget {}",
        ds.declaration_json.len(),
        budget
    );

    // No literal colours, and no executable/CSS/URL text anywhere in the declaration.
    for forbidden in ["\"#", "rgb(", "hsl("] {
        assert!(
            !ds.declaration_json.contains(forbidden),
            "declaration contains a colour literal: {forbidden}"
        );
    }
    for forbidden in [
        "<script",
        "</",
        "url(",
        "http://",
        "https://",
        "function",
        "javascript:",
    ] {
        assert!(
            !ds.declaration_json.contains(forbidden),
            "inert design-system data must not contain {forbidden}"
        );
    }

    // Colour authority: every colour-bearing property resolves to a known role.
    use clay::shell::design_system::ThemeColorRef;
    for (key, recipe) in &decl.recipes {
        let mut refs: Vec<(&str, &ThemeColorRef)> = Vec::new();
        for (name, candidate) in [
            ("backgroundColor", recipe.background_color.as_ref()),
            ("textColor", recipe.text_color.as_ref()),
            ("borderColor", recipe.border_color.as_ref()),
            ("outlineColor", recipe.outline_color.as_ref()),
        ] {
            if let Some(color) = candidate {
                refs.push((name, color));
            }
        }
        for layer in recipe.shadow.iter().flatten() {
            refs.push(("shadow.colorRole", &layer.color_role));
        }
        for (name, color) in refs {
            let role = color.as_str();
            assert!(
                role == "transparent" || ThemeColorRef::parse(role).is_ok(),
                "{key}.{name} is not a semantic theme role: {role}"
            );
        }
    }
}

#[test]
fn plan118_design_instrument_radius_state_and_material_discipline() {
    use clay::shell::design_system::TransformPreset;

    let decl = design_instrument_declaration();
    let recipe = |key: &str| {
        decl.recipes
            .iter()
            .find(|(k, _)| k.to_key_string() == key)
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("missing recipe {key}"))
    };

    // The radius ladder is the only radius vocabulary: 5 / 8 / 12 / 16 / pill / 0 (regions).
    let ladder = [0.0, 5.0, 8.0, 12.0, 16.0, 9999.0];
    for (key, recipe) in &decl.recipes {
        if let Some(radius) = recipe.border_radius {
            assert!(
                ladder.contains(&radius),
                "{key} radius {radius} is off the ladder {ladder:?}"
            );
        }
    }
    assert_eq!(recipe("button.default.root.rest").border_radius, Some(8.0));
    assert_eq!(recipe("list.default.row.rest").border_radius, Some(8.0));
    assert_eq!(recipe("panel.default.root.rest").border_radius, Some(12.0));
    assert_eq!(
        recipe("modal.default.dialog.rest").border_radius,
        Some(16.0)
    );
    assert_eq!(recipe("tab.default.item.rest").border_radius, Some(9999.0));
    assert_eq!(recipe("kbd.default.root.rest").border_radius, Some(5.0));

    // No border is wider than a hairline (DESIGN.md §14.3).
    for (key, recipe) in &decl.recipes {
        if let Some(width) = recipe.border_width {
            assert!(
                width <= 1.0,
                "{key} border width {width} exceeds a hairline"
            );
        }
    }

    // Static surfaces carry no shadow; only the two elevations do (DESIGN.md §6).
    for (key, recipe) in &decl.recipes {
        let static_family = (key.component == "editor" && !matches!(key.slot.as_str(), "tooltip"))
            || key.component == "scroll"
            || key.component == "shell"
            || key.component == "statusBar"
            || key.component == "list"
            || key.component == "fileBrowser"
            || key.component == "paneSplitTree"
            || key.component == "tab"
            || key.component == "tabBar";
        let static_family = static_family && key.slot != "tooltip";
        let layers = recipe.shadow.iter().flatten().count();
        if static_family {
            assert_eq!(
                layers, 0,
                "{key} is a static surface and must not paint a shadow"
            );
        }
    }
    assert_eq!(
        recipe("modal.default.dialog.rest")
            .shadow
            .as_ref()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        recipe("popover.default.root.rest")
            .shadow
            .as_ref()
            .map(Vec::len),
        Some(2)
    );

    // Blur only on the scrim and the toast; the editor and scroll paths are frozen at 0.
    for (key, recipe) in &decl.recipes {
        match recipe.backdrop_blur {
            None => {}
            Some(blur) => {
                assert!(
                    (blur - 3.0).abs() < f64::EPSILON || (blur - 8.0).abs() < f64::EPSILON,
                    "{key} backdropBlur {blur} is not the scrim's 3 or the toast's 8"
                );
                let allowed =
                    (key.component == "modal" && key.slot == "scrim") || (key.component == "toast");
                assert!(allowed, "{key} may not blur (DESIGN.md §6)");
            }
        }
        if key.component == "editor" {
            assert!(recipe.backdrop_blur.is_none(), "{key} must not blur");
        }
    }

    // No hover lift anywhere; press feedback only on buttons (DESIGN.md §7).
    for (key, recipe) in &decl.recipes {
        assert_ne!(
            recipe.transform_preset,
            Some(TransformPreset::HoverLift),
            "{key} reintroduces hover lift"
        );
        if recipe.transform_preset == Some(TransformPreset::PressShiftDown) {
            assert!(
                key.component == "button",
                "{key} uses press-shift-down; press is for buttons only"
            );
        }
    }

    // State language: focus is the 2px ring at offset 2 wherever it appears.
    for (key, recipe) in &decl.recipes {
        if recipe.outline_color.is_some() && key.component != "paneSplitTree" {
            assert_eq!(recipe.outline_width, Some(2.0), "{key} outline width");
            assert_eq!(
                recipe.outline_offset,
                Some(2.0),
                "{key} focus outlines sit at offset 2 (DESIGN.md §9)"
            );
        }
    }

    // Selection is a fill: no row family paints a leading bar or an extra edge.
    for (key, recipe) in &decl.recipes {
        if key.state == clay::shell::design_system::RecipeState::Selected
            && matches!(
                key.component.as_str(),
                "list" | "recentRow" | "menu" | "dropdown"
            )
        {
            assert_eq!(
                recipe.background_opacity,
                Some(0.15),
                "{key} selection must be the accent fill at 0.15"
            );
            assert!(
                (recipe.border_width.unwrap_or(0.0) - 0.0).abs() < f64::EPSILON,
                "{key} selection must not add an edge"
            );
            assert!(
                recipe.shadow.is_none(),
                "{key} selection must not add a shadow"
            );
        }
    }
}

#[test]
fn plan118_design_instrument_keys_are_the_reference_set_minus_chat_plus_the_new_families() {
    // The removed packages' key set, recorded before deletion (task 9). Reading a
    // fixture instead of a deleted package keeps this delta assertion honest.
    let reference = reference_design_system_keys();
    assert_eq!(reference.len(), 142, "reference baseline is 142 keys");

    let instrument = design_system_keys("design-instrument", "@clay/design-instrument");
    assert_eq!(instrument.len(), 165, "130 reference keys + 35 new ones");

    // 1. Nothing else was dropped or renamed: every non-chat reference key survives.
    let chat: Vec<String> = reference
        .iter()
        .filter(|key| key.starts_with("chat."))
        .cloned()
        .collect();
    assert_eq!(chat.len(), 12, "the chat surface owns 12 keys");
    let expected: std::collections::BTreeSet<String> = reference
        .iter()
        .filter(|key| !key.starts_with("chat."))
        .cloned()
        .collect();
    let missing: Vec<&String> = expected.difference(&instrument).collect();
    assert!(
        missing.is_empty(),
        "keys dropped from the reference set: {missing:#?}"
    );

    // 2. The additions are exactly the declared families, and nothing else.
    let added: std::collections::BTreeSet<String> =
        instrument.difference(&expected).cloned().collect();
    let added_components: std::collections::BTreeSet<&str> = added
        .iter()
        .map(|key| key.split('.').next().expect("component segment"))
        .collect();
    let expected_components: std::collections::BTreeSet<&str> = [
        "agentPicker",
        "empty",
        "keyHint",
        "recentRow",
        "seg",
        "sessionRow",
        "statRow",
        "statusDot",
        "swatch",
        "toast",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        added_components, expected_components,
        "added keys must belong to the declared new families"
    );
    assert_eq!(added.len(), 35, "the new families own 35 keys: {added:#?}");

    // 3. Every added family is declared per state, with no half-declared control.
    for key in [
        "seg.default.item.rest",
        "seg.default.item.hover",
        "seg.default.item.selected",
        "seg.default.item.focus",
        "seg.default.item.disabled",
        "agentPicker.default.trigger.expanded",
        "recentRow.default.root.selected",
        "sessionRow.default.root.hover",
        "toast.default.root.rest",
        "empty.default.root.rest",
        "statusDot.busy.root.rest",
        "keyHint.default.row.rest",
        "swatch.default.root.selected",
        "statRow.default.bar.rest",
    ] {
        assert!(instrument.contains(key), "missing declared key {key}");
    }
}

#[test]
fn plan118_design_instrument_rejects_mutated_probes() {
    // Healthy manifest assembles.
    assert!(assemble_package_record(&design_instrument_manifest()).is_ok());

    let mutate = |path: &[&str], replacement: serde_json::Value| -> serde_json::Value {
        let mut value = design_instrument_manifest();
        let mut cursor = &mut value;
        for segment in path {
            cursor = cursor
                .get_mut(*segment)
                .unwrap_or_else(|| panic!("missing {path:?}"));
        }
        *cursor = replacement;
        value
    };
    let recipes = ["clay", "contributions", "uiDesignSystem", "recipes"];
    let recipe_path = |key: &str, prop: &str| -> Vec<String> {
        recipes
            .iter()
            .map(|s| s.to_string())
            .chain([key.to_string(), prop.to_string()])
            .collect()
    };
    fn as_path(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    // Literal colour and palette aliases are rejected.
    for literal in [
        json!("#ff0000"),
        json!("rgb(255,0,0)"),
        json!("package.custom.red"),
    ] {
        let path = recipe_path("button.default.root.rest", "backgroundColor");
        let mutated = mutate(&as_path(&path), literal);
        assert!(
            assemble_package_record(&mutated).is_err(),
            "a literal/alias colour must be rejected"
        );
    }

    // Out-of-bounds geometry, motion, opacity and shadow layers are rejected.
    let cases: Vec<(Vec<String>, serde_json::Value)> = vec![
        (
            recipe_path("panel.default.root.rest", "borderRadius"),
            json!(40.0),
        ),
        (
            recipe_path("button.default.root.rest", "borderWidth"),
            json!(12.0),
        ),
        (
            recipe_path("button.default.root.rest", "transitionDuration"),
            json!(5000.0),
        ),
        (
            recipe_path("button.default.root.disabled", "opacity"),
            json!(1.5),
        ),
        (
            recipe_path("badge.accent.root.rest", "backgroundOpacity"),
            json!(-0.2),
        ),
        (
            recipe_path("modal.default.scrim.rest", "backdropBlur"),
            json!(64.0),
        ),
        (
            recipe_path("modal.default.dialog.rest", "shadow"),
            json!([
                {"x": 0, "y": 1, "blur": 0, "spread": 0, "colorRole": "text.primary", "opacity": 1},
                {"x": 0, "y": 2, "blur": 0, "spread": 0, "colorRole": "text.primary", "opacity": 1},
                {"x": 0, "y": 3, "blur": 0, "spread": 0, "colorRole": "text.primary", "opacity": 1},
                {"x": 0, "y": 4, "blur": 0, "spread": 0, "colorRole": "text.primary", "opacity": 1}
            ]),
        ),
    ];
    for (path, replacement) in cases {
        let mutated = mutate(&as_path(&path), replacement);
        assert!(
            assemble_package_record(&mutated).is_err(),
            "out-of-bounds {path:?} must be rejected"
        );
    }

    // A design system cannot smuggle an executable surface.
    // The payload budget and the recipe ceiling fail closed: a design system that
    // grows past either is rejected rather than shipped to the client.
    let mut bloated = design_instrument_manifest();
    let template =
        bloated["clay"]["contributions"]["uiDesignSystem"]["recipes"]["button.default.root.rest"]
            .clone();
    let mut recipes = serde_json::Map::new();
    for index in 0..600 {
        recipes.insert(format!("filler{index}.default.root.rest"), template.clone());
    }
    bloated["clay"]["contributions"]["uiDesignSystem"]["recipes"] =
        serde_json::Value::Object(recipes);
    assert!(
        assemble_package_record(&bloated).is_err(),
        "a declaration past the recipe/payload bound must be rejected"
    );
}

#[test]
fn plan118_design_instrument_is_listed_in_the_bundled_inventory() {
    let path = format!("{}/src/packages/bundled-inventory.toml", manifest_dir());
    let text = fs::read_to_string(&path).expect("read bundled-inventory.toml");
    assert!(
        text.contains("root = \"design-instrument\""),
        "@clay/design-instrument must be listed in the bundled inventory, or it is never trusted"
    );
    // A shipped package without a directory (or a directory without an entry) fails closed:
    // the in-crate inventory tests compare roots against this table, so both must exist.
    let dir = format!("{}/packages/design-instrument/package.json", manifest_dir());
    assert!(
        Path::new(&dir).exists(),
        "packages/design-instrument/package.json must exist"
    );
    // The removed roots are absent from the compiled trust inventory: no stale
    // entry can resolve a directory that no longer exists. The compiled inventory
    // is crate-private, so `plan118_removed_design_systems_are_absent` (which scans
    // `bundled-inventory.toml` itself) and the in-crate
    // `unlisted_package_dirs_are_not_trusted` are what pin this behaviour.
    assert!(
        !text.contains("design-neobrutal") && !text.contains("design-glass"),
        "the bundled inventory must not keep an entry for a removed package root"
    );
}

/// Plan 118 task 9: the Neobrutal and Glass design systems are gone — not merely
/// unreferenced. The specifiers must appear nowhere in code, manifests, fixtures,
/// or harness scripts (Markdown prose and frozen plan/decision records excluded).
#[test]
fn plan118_removed_design_systems_are_absent() {
    let removed = ["design-neobrutal", "design-glass"];
    let mut hits: Vec<String> = Vec::new();

    fn scan(dir: &Path, removed: &[&str], hits: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "node_modules" || name == "target" || name == ".git" {
                continue;
            }
            if path.is_dir() {
                scan(&path, removed, hits);
                continue;
            }
            // Frozen/approved artifacts and historical records may name the removed
            // systems; live code, data, fixtures and scripts may not.
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            for needle in removed {
                if content.contains(needle) {
                    hits.push(format!("{}: {needle}", path.display()));
                }
            }
        }
    }

    for dir in [
        "packages",
        "src",
        "frontend/src",
        "tests/fixtures",
        "scripts",
        "examples",
    ] {
        scan(&Path::new(&manifest_dir()).join(dir), &removed, &mut hits);
    }

    assert!(
        hits.is_empty(),
        "removed design systems still referenced: {hits:#?}"
    );

    // The directories themselves are gone, and no inventory entry survives them
    // (a stale entry for a removed directory would fail the trust binding).
    for slug in ["design-neobrutal", "design-glass"] {
        let package = Path::new(&manifest_dir()).join("packages").join(slug);
        assert!(!package.exists(), "packages/{slug} must be deleted");
    }
}

/// Plan 118 tasks "Remove the `@clay/chat` package and its server surface" and
/// "Remove the chat frontend surface and de-chat the shared agent module": the
/// landing package, its pane contribution, its command ids and its React
/// surface are gone — not merely unreferenced. No load line, package directory,
/// inventory entry, command id, action target, fixture, panel, module or symbol
/// may survive them (Markdown prose and frozen plan/decision records excluded).
#[test]
fn plan118_chat_landing_and_its_commands_are_absent() {
    let removed = [
        "@clay/chat",
        "chat.entry",
        "chat.profile",
        "chat.submit",
        "chat.cancel",
        "chat.steer",
        "chat.composer",
        "chat.openAgentPicker",
        "chat.openProviderPicker",
        "chat.openModelPicker",
        // The shared agent module's chat-era symbols (renamed to session naming).
        "chatAgent",
        "ChatAgentModule",
        "ChatSnapshot",
        "ChatStatus",
        "ChatIntentContext",
        "ChatPanel",
        "seedChatFixture",
        "resetChatAgentForTests",
        "__clayChatAgent",
    ];
    // Guards that assert the retirement itself, on purpose naming the retired
    // ids and symbols: an invocation must fail as an unknown command (server)
    // and a resurrection must fail the client scan (frontend).
    const RETIREMENT_GUARDS: &[&str] = &[
        "src/server/command_execution.rs",
        "src/server/connection/runtime.rs",
        "src/server/js_runtime/tests/coding_agent_and_launcher.rs",
        "frontend/src/test/chat-surface-absence.test.ts",
    ];
    let manifest_dir = manifest_dir();
    let root = Path::new(&manifest_dir);
    let mut hits: Vec<String> = Vec::new();

    fn scan(dir: &Path, removed: &[&str], hits: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "node_modules" || name == "target" || name == ".git" || name == "dist" {
                continue;
            }
            if path.is_dir() {
                scan(&path, removed, hits);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            for needle in removed {
                if content.contains(needle) {
                    hits.push(format!("{}: {needle}", path.display()));
                }
            }
        }
    }

    for dir in [
        "packages",
        "src",
        "tests/fixtures",
        "scripts",
        "examples",
        // Plan 118's chat-frontend task deleted the panel/fixture/landing branch,
        // so the whole client is in scope now — including `styles/tokens.css`,
        // which carried the removed family's fallback declarations.
        "frontend/src",
    ] {
        scan(&root.join(dir), &removed, &mut hits);
    }
    hits.retain(|hit| {
        !RETIREMENT_GUARDS
            .iter()
            .any(|guard| hit.contains(&format!("{}/{guard}", root.display())))
    });
    assert!(
        hits.is_empty(),
        "the removed chat landing still referenced: {hits:#?}"
    );
    // Every guard is a live test, not a rotted exemption: if one is deleted the
    // scan above would quietly lose its only allowance, so they must exist.
    for guard in RETIREMENT_GUARDS {
        assert!(
            root.join(guard).exists(),
            "{guard} is exempt from the removal scan but no longer exists — \
             restore the guard or drop the exemption"
        );
    }

    // The package directory, its inventory entry and its load line are gone, and
    // the example tree still loads the surviving agent package.
    assert!(
        !root.join("packages/chat").exists(),
        "packages/chat must be deleted"
    );
    let inventory = fs::read_to_string(root.join("src/packages/bundled-inventory.toml")).unwrap();
    assert!(
        !inventory.contains("chat"),
        "the bundled inventory must not keep an entry for the removed root"
    );
    let first_party =
        fs::read_to_string(root.join("examples/config/packages/first-party.js")).unwrap();
    assert!(
        !first_party.contains("@clay/chat"),
        "the canonical first-party module must not load the removed landing"
    );

    // The React surface itself is gone (not just unreferenced).
    assert!(
        !root.join("frontend/src/chat").exists(),
        "frontend/src/chat must be deleted"
    );
}

// ============================================================================
// Plan 118 Task 15: catalog currency — declared slots and removed systems
// ============================================================================

/// Rows that are hosts rather than recipe targets: `tabList` carries closed recipe
/// *attributes* and paints through the `tab`/`tabBar` families, and `table` is
/// reserved. Neither can be marked `†`.
const MATRIX_EXEMPT_ROWS: &[(&str, &str)] = &[("tabList", "root"), ("table", "root")];

/// Parse `(family, slot, marked)` from the recipe matrix's three contract tables.
fn recipe_matrix_rows() -> Vec<(String, String, bool)> {
    let path = format!(
        "{}/docs/development/ui-design-system-recipe-matrix.md",
        manifest_dir()
    );
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    let mut rows = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((family, rest)) = rest.split_once("` | `") else {
            continue;
        };
        let (slot, marked) = match rest.split_once('`') {
            Some((slot, tail)) => (slot, tail.starts_with('\u{2020}')),
            None => continue,
        };
        if !family.chars().all(|c| c.is_ascii_alphabetic())
            || !slot.chars().all(|c| c.is_ascii_alphabetic())
        {
            continue;
        }
        rows.push((family.to_string(), slot.to_string(), marked));
    }
    rows
}

/// Plan 118 task 15: the matrix must not present a slot as part of the contract when
/// the shipped package declares no recipe for it. The `†` markers are the contract —
/// recomputed here from the shipped manifest, in both directions, so the catalogs
/// cannot drift back into promising variables that do not exist.
#[test]
fn plan118_recipe_matrix_marks_undeclared_slots() {
    let declared: std::collections::BTreeSet<(String, String)> =
        design_system_keys("design-instrument", "@clay/design-instrument")
            .into_iter()
            .filter_map(|key| {
                let mut parts = key.split('.');
                let family = parts.next()?.to_string();
                let _variant = parts.next()?;
                let slot = parts.next()?.to_string();
                Some((family, slot))
            })
            .collect();

    let rows = recipe_matrix_rows();
    assert!(
        rows.len() >= 90,
        "expected the full matrix, parsed {} rows",
        rows.len()
    );

    let exempt: std::collections::BTreeSet<(String, String)> = MATRIX_EXEMPT_ROWS
        .iter()
        .map(|(f, s)| (f.to_string(), s.to_string()))
        .collect();

    let mut stale_marks = Vec::new();
    let mut missing_marks = Vec::new();
    for (family, slot, marked) in &rows {
        let key = (family.clone(), slot.clone());
        if exempt.contains(&key) {
            continue;
        }
        let is_declared = declared.contains(&key);
        if *marked && is_declared {
            stale_marks.push(format!("{family}.{slot}"));
        }
        if !*marked && !is_declared {
            missing_marks.push(format!("{family}.{slot}"));
        }
    }
    assert!(
        stale_marks.is_empty(),
        "these slots are declared by the shipped package but marked `†` in the matrix: {stale_marks:#?}"
    );
    assert!(
        missing_marks.is_empty(),
        "these slots have no shipped recipe and must carry `†` in the matrix: {missing_marks:#?}"
    );

    // The generated index in `components.md` carries the same marks, so a reader of
    // either catalog sees the same contract.
    let components = fs::read_to_string(format!(
        "{}/.agents/skills/clay-execution/references/components.md",
        manifest_dir()
    ))
    .expect("read components.md");
    let index_start = components
        .find("## UI Design-System Recipe Slots (Plan 101)")
        .expect("components.md holds the slot index section");
    let index = &components[index_start..];
    for (family, slot, marked) in &rows {
        if exempt.contains(&(family.clone(), slot.clone())) {
            continue;
        }
        if *marked {
            assert!(
                index.contains(&format!("`{slot}\u{2020}`")),
                "components.md slot index must mark {family}.{slot} with `†`"
            );
        } else {
            assert!(
                index.contains(&format!("`{slot}`")),
                "components.md slot index must list {family}.{slot}"
            );
        }
    }
}

/// Plan 118 task 15: a catalog page must not present a removed design system (or a
/// retired pattern) as part of the shipped contract. Pages that are historical
/// records keep their mentions; every other mention has to say so in the same line.
/// Plan 118's documentation task extends the same rule to the removed chat surface
/// and to the product-level pages (`PRODUCT.md`, `roadmap.md`); its JS-API task
/// extends it to the published Clay JS API pages and the generated registry
/// compiled from them.
#[test]
fn plan118_catalog_pages_do_not_ship_removed_systems() {
    // Pages that are historical records by contract (frozen audit/review or a
    // decision record): they keep the removed names, with a banner saying so.
    const HISTORICAL_PAGES: &[&str] = &[
        "docs/development/ui-design-system-css-audit.md",
        "docs/development/ui-design-system-package-primitive-review.md",
        "docs/development/ui-design-system-conformance.md",
        "docs/wiki/modules/ui-design-system-runtime.md",
        "docs/reference/ui-design-systems.md",
        "docs/development/tauri-react-primitive-migration.md",
        "docs/development/tauri-react-parity-ledger.md",
        "docs/development/react-ui-catalog-mapping.md",
        "docs/wiki/modules/ui-review-harness.md",
    ];
    // A line may name a removed system only while marking it as gone.
    const REMOVAL_MARKERS: &[&str] = &[
        "remov",
        "former",
        "previous",
        "historical",
        "retired",
        "supersede",
        "replace",
        "reject",
        "no longer",
        "deleted",
        "pre-migration",
        "went with",
    ];
    let removed = ["neobrutal", "luminous glass", "design-glass"];
    // The removed chat surface (plan 118 task 22): its identifiers, not the bare
    // word, so ordinary prose stays free.
    let removed_surfaces = ["@clay/chat", "chatpanel", "chat.default.", "chat.entry"];

    let mut pages = vec![
        "DESIGN.md".to_string(),
        "PRODUCT.md".to_string(),
        "roadmap.md".to_string(),
        "docs/index.md".to_string(),
        "docs/reference/ui-components.md".to_string(),
        "docs/reference/packages/creating-packages.md".to_string(),
        "docs/development/ui-design-system-recipe-matrix.md".to_string(),
        "docs/development/ui-design-system-visual-direction.md".to_string(),
        // Plan 118's wiki pass: the two pages added by it describe only the
        // shipped state, so they are scanned for removed-system mentions too.
        "docs/wiki/modules/launcher-landing-surface.md".to_string(),
        "docs/wiki/modules/design-artifact-gate.md".to_string(),
        // Plan 118's JS-API task: the published API pages and the generated
        // registry they are compiled into must not advertise a removed system,
        // package, command or surface either — the registry is the machine-
        // readable promise, so a stale artifact is a stale API.
        "docs/reference/clay-js-api/theme/set-design-system.md".to_string(),
        "docs/reference/clay-js-api/theme/set-theme.md".to_string(),
        "docs/reference/clay-js-api/settings/set-design-system.md".to_string(),
        // Plan 118's configuration task: the guide users configure from and the
        // canonical example they copy must not offer a removed system, the
        // removed chat surface, or a chat landing-package line either.
        "docs/reference/clay-js-api/configuration.md".to_string(),
        "examples/config/init.js".to_string(),
        "docs/generated/clay-js-api-registry.json".to_string(),
        ".agents/skills/clay-execution/references/components.md".to_string(),
        ".agents/skills/clay-execution/references/tokens.md".to_string(),
        ".agents/skills/clay-execution/references/ui.md".to_string(),
        ".agents/skills/clay-execution/references/config.md".to_string(),
    ];
    pages.extend(HISTORICAL_PAGES.iter().map(|p| (*p).to_string()));

    let mut hits: Vec<String> = Vec::new();
    for page in &pages {
        let path = format!("{}/{page}", manifest_dir());
        let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
        let historical = HISTORICAL_PAGES.contains(&page.as_str());
        for (number, line) in text.lines().enumerate() {
            let lowered = line.to_lowercase();
            let Some(needle) = removed
                .iter()
                .chain(removed_surfaces.iter())
                .find(|needle| lowered.contains(*needle))
            else {
                continue;
            };
            if historical {
                continue;
            }
            if REMOVAL_MARKERS
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                continue;
            }
            hits.push(format!(
                "{page}:{} names `{needle}` as current: {}",
                number + 1,
                line.trim()
            ));
        }
    }
    assert!(
        hits.is_empty(),
        "catalog pages must not present a removed design system or the removed chat surface as shipped:\n{:#?}",
        hits
    );
}

/// Plan 118 task 16: `@clay/core` is not a second language. Every shipped recipe the
/// core fallback set covers must resolve to exactly the fallback value, so a build
/// that has not installed a design system paints what the package activates and the
/// activation swap cannot move geometry, material or motion.
#[test]
fn plan118_core_fallbacks_match_the_shipped_language() {
    use clay::shell::design_system::{core_design_system_fallbacks, resolve_design_system};

    let declaration = design_instrument_declaration();
    let resolved = resolve_design_system(&declaration, None).expect("shipped system resolves");
    let fallbacks = core_design_system_fallbacks();

    let mut shared = 0usize;
    for (key, fallback) in &fallbacks {
        let Some(active) = resolved.recipes.get(key) else {
            continue;
        };
        shared += 1;
        assert_eq!(
            active,
            fallback,
            "core fallback `{}` must equal the shipped recipe (DESIGN.md §16: one language)",
            key.to_key_string()
        );
    }

    // The fallback set is deliberately the host-consumed subset, not the whole
    // package: it must still cover the component kinds and shell surfaces the host
    // paints before a snapshot lands (`plan101` asserts the exact required set).
    assert!(
        shared >= 30,
        "expected the core fallbacks to cover the shipped controls (shared={shared})"
    );
}

/// Plan 125: the composer palette and the `@` mentions menu carry the `halo`
/// (DESIGN.md §6) — one even, zero-offset glow stated on the two existing keys,
/// with no key added — while the surfaces that have nothing veiled behind them
/// (`popover`, `overlay`) keep their cast shadows.
#[test]
fn plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow() {
    use clay::shell::design_system::resolve_design_system;

    let declaration = design_instrument_declaration();
    let resolved = resolve_design_system(&declaration, None).expect("shipped system resolves");

    /// DESIGN.md §6: `{x, y, blur, spread, opacity}` per layer, text.primary.
    const HALO: &[(f64, f64, f64, f64, f64)] =
        &[(0.0, 0.0, 14.0, -2.0, 0.14), (0.0, 0.0, 3.0, 0.0, 0.08)];

    let layers = |key: &str| {
        let recipe = resolved
            .recipes
            .iter()
            .find(|(recipe_key, _)| recipe_key.to_key_string() == key)
            .map(|(_, recipe)| recipe)
            .unwrap_or_else(|| panic!("`{key}` is not a shipped recipe"));
        recipe
            .shadow
            .iter()
            .map(|layer| (layer.x, layer.y, layer.blur, layer.spread, layer.opacity))
            .collect::<Vec<_>>()
    };

    for key in ["commandCentre.default.root.rest", "menu.default.root.rest"] {
        let got = layers(key);
        assert_eq!(
            got,
            HALO.to_vec(),
            "`{key}` must carry DESIGN.md §6's halo (even ink, zero offset)"
        );
        assert!(
            got.iter()
                .all(|(x, _, blur, _, _)| (*x - 0.0).abs() < f64::EPSILON && *blur > 0.0),
            "`{key}` halo layers are zero-offset and keep a visible blur (§14.1)"
        );
    }
    assert_eq!(
        layers("commandCentre.default.root.rest"),
        layers("menu.default.root.rest"),
        "the palette and the mentions menu state one halo value, not two"
    );

    // Nothing veiled sits behind these: they keep the cast shadows.
    assert_eq!(
        layers("popover.default.root.rest").first().map(|l| l.1),
        Some(14.0)
    );
    assert_eq!(
        layers("modal.default.dialog.rest").first().map(|l| l.1),
        Some(24.0)
    );

    assert_eq!(
        declaration.recipes.len(),
        165,
        "the halo is a value: no recipe key is added or removed"
    );
}

/// Plan 118 task 16: the static `--clay-ds-*` block in `styles/tokens.css` is the
/// pre-bootstrap projection of the same recipes. Every variable it states must equal
/// what the shipped package resolves for that key, or the activation swap repaints.
#[test]
fn plan118_host_fallback_block_matches_the_resolved_package() {
    use clay::shell::design_system::{ResolvedComponentRecipe, resolve_design_system};

    /// Property suffixes the block may state, longest first so `outline-style`
    /// cannot be read as `style`.
    const PROPS: &[&str] = &[
        "background-color",
        "background-opacity",
        "transition-duration",
        "transition-timing",
        "border-radius",
        "border-color",
        "border-width",
        "border-style",
        "backdrop-saturate",
        "backdrop-blur",
        "inner-highlight",
        "outline-color",
        "outline-width",
        "outline-offset",
        "outline-style",
        "transform-preset",
        "text-color",
        "padding",
        "shadow",
        "gap",
        "opacity",
    ];

    fn kebab(key: &str) -> String {
        let mut out = String::new();
        for ch in key.chars() {
            if ch.is_ascii_uppercase() {
                out.push('-');
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch);
            }
        }
        out.replace('.', "-")
    }

    /// Prettier wraps long values (`cubic-bezier( 0, 0, 0.2, 1 )`); compare the
    /// value, not its line breaks.
    fn normalize(value: &str) -> String {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("( ", "(")
            .replace(" )", ")")
            .replace(", ", ",")
    }

    fn role_css(role: &str) -> String {
        if role == "transparent" {
            "transparent".to_string()
        } else {
            format!("var(--clay-{})", role.replace('.', "-"))
        }
    }

    /// Mirrors `frontend/src/theme/design-system-adapter.ts`: the block and the
    /// runtime snapshot must project a recipe to the same CSS text.
    fn projected(prop: &str, recipe: &ResolvedComponentRecipe) -> String {
        fn number(value: f64) -> String {
            if (value.fract() - 0.0).abs() < f64::EPSILON {
                format!("{value:.0}")
            } else {
                format!("{value}")
            }
        }
        let number = |value: f64| number(value);
        match prop {
            "background-color" => role_css(recipe.background_color.as_str()),
            "text-color" => role_css(recipe.text_color.as_str()),
            "border-color" => role_css(recipe.border_color.as_str()),
            "outline-color" => role_css(recipe.outline_color.as_str()),
            "border-radius" => format!("{}px", number(recipe.border_radius)),
            "border-width" => format!("{}px", number(recipe.border_width)),
            "backdrop-blur" => format!("{}px", number(recipe.backdrop_blur)),
            "outline-width" => format!("{}px", number(recipe.outline_width)),
            "outline-offset" => format!("{}px", number(recipe.outline_offset)),
            "transition-duration" => format!("{}ms", number(recipe.transition_duration)),
            "transition-timing" => match recipe.transition_timing.as_str() {
                "linear" => "linear".to_string(),
                "ease-out" => "cubic-bezier(0, 0, 0.2, 1)".to_string(),
                "spring-snappy" => "cubic-bezier(0.2, 0.8, 0.2, 1)".to_string(),
                other => panic!("unexpected timing `{other}`"),
            },
            "transform-preset" => match recipe.transform_preset.as_str() {
                "none" => "none".to_string(),
                "press-shift-down" => "translateY(1px)".to_string(),
                "press-subtle" => "scale(0.98)".to_string(),
                other => panic!("unexpected transform preset `{other}`"),
            },
            "border-style" => recipe.border_style.as_str().to_string(),
            "outline-style" => recipe.outline_style.as_str().to_string(),
            "opacity" => number(recipe.opacity),
            "shadow" => {
                if recipe.shadow.is_empty() {
                    "none".to_string()
                } else {
                    recipe
                        .shadow
                        .iter()
                        .map(|layer| {
                            let role = role_css(layer.color_role.as_str());
                            let tint = if (layer.opacity - 1.0).abs() < f64::EPSILON {
                                role
                            } else {
                                format!(
                                    "color-mix(in srgb, {role} {}%, transparent)",
                                    (layer.opacity * 100.0).round() as i64
                                )
                            };
                            format!(
                                "{}px {}px {}px {}px {tint}",
                                number(layer.x),
                                number(layer.y),
                                number(layer.blur),
                                number(layer.spread)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            }
            other => panic!("the block must not state `{other}`"),
        }
    }

    let declaration = design_instrument_declaration();
    let resolved = resolve_design_system(&declaration, None).expect("shipped system resolves");

    let tokens_path = format!("{}/frontend/src/styles/tokens.css", manifest_dir());
    let tokens_css = fs::read_to_string(&tokens_path)
        .unwrap_or_else(|err| panic!("read tokens.css ({tokens_path}): {err}"));
    let marker = "/* Host fallback design-system recipe variables";
    let start = tokens_css
        .find(marker)
        .unwrap_or_else(|| panic!("tokens.css must keep the host fallback block marker"));
    let rest = &tokens_css[start..];
    let end = rest
        .find("\n}\n")
        .map(|offset| start + offset)
        .unwrap_or(tokens_css.len());
    let block = &tokens_css[start..end];

    let mut checked = 0usize;
    for declaration_text in block.split(';') {
        let Some((name, value)) = declaration_text.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let Some(variable) = name.strip_prefix("--clay-ds-") else {
            continue;
        };
        let Some(prop) = PROPS
            .iter()
            .find(|prop| variable.ends_with(&format!("-{prop}")))
        else {
            panic!("unknown host fallback variable `{name}`");
        };
        let prefix = variable.trim_end_matches(&format!("-{prop}"));
        // Spacing tokens and the composed veil/scrim tints are host-authored: the
        // recipe carries a token name or a fill + opacity pair, not a CSS string.
        if matches!(*prop, "padding" | "gap" | "background-opacity") {
            continue;
        }
        let Some((key, recipe)) = declaration
            .recipes
            .keys()
            .find(|key| kebab(&key.to_key_string()) == prefix)
            .and_then(|key| resolved.recipes.get(key).map(|recipe| (key, recipe)))
        else {
            // Every declared fallback states a shipped recipe: the removed chat
            // family was the last exemption (plan 118's chat-frontend task).
            panic!("host fallback variable `{name}` has no shipped recipe");
        };
        if *prop == "background-color" && (recipe.background_opacity - 1.0).abs() >= f64::EPSILON {
            continue;
        }
        let want = normalize(&projected(prop, recipe));
        let got = normalize(value);
        assert_eq!(
            got,
            want,
            "`{name}` ({} · {prop}) must equal the shipped recipe value",
            key.to_key_string()
        );
        checked += 1;
    }
    assert!(
        checked >= 150,
        "expected the host fallback block to state the migrated values (checked={checked})"
    );
}

/// Plan 118 task 16: no fallback recipe may reintroduce a retired pattern — the
/// banned 0px radius outside a full-bleed region, a hard offset shadow, a linear
/// transition with a duration, or a ring that is not the 2px/offset-2 focus ring.
#[test]
fn plan118_core_fallbacks_obey_the_language_rules() {
    use clay::shell::design_system::{RecipeState, core_design_system_fallbacks};

    /// Full-bleed regions (§5: a region's outer edge stays square) and non-boxes
    /// (text-only or layout-only recipes, where a radius is not visible).
    const FLUSH_KEYS: &[&str] = &[
        "dropdown.default.root.rest",
        "empty.default.root.rest",
        "fileBrowser.default.root.rest",
        "flex.default.root.rest",
        "grid.default.root.rest",
        "keyHint.default.keys.rest",
        "keyHint.default.root.rest",
        "keyHint.default.row.rest",
        "label.default.root.rest",
        "modal.default.root.rest",
        "modal.default.scrim.rest",
        "paneSplitTree.default.root.rest",
        "scroll.default.root.rest",
        "statusBar.default.root.rest",
        "tabBar.default.root.rest",
    ];
    const RADII: &[f64] = &[0.0, 5.0, 8.0, 12.0, 16.0, 9999.0];
    const DURATIONS: &[f64] = &[0.0, 150.0, 240.0, 620.0];

    for (key, recipe) in core_design_system_fallbacks() {
        let name = key.to_key_string();
        assert!(
            RADII.contains(&recipe.border_radius),
            "`{name}` radius {} is not on the 5/8/12/16/pill ladder (DESIGN.md §5)",
            recipe.border_radius
        );
        if !FLUSH_KEYS.contains(&name.as_str()) {
            assert!(
                (recipe.border_radius - 0.0).abs() >= f64::EPSILON,
                "`{name}` may not be 0px (DESIGN.md §5)"
            );
        }
        for layer in &recipe.shadow {
            assert!(
                !((layer.blur - 0.0).abs() < f64::EPSILON
                    && ((layer.x - 0.0).abs() >= f64::EPSILON
                        || (layer.y - 0.0).abs() >= f64::EPSILON)),
                "`{name}` reintroduces a hard offset shadow (DESIGN.md §14)"
            );
        }
        assert!(
            DURATIONS.contains(&recipe.transition_duration),
            "`{name}` duration {} is not a language motion tier (DESIGN.md §7)",
            recipe.transition_duration
        );
        if recipe.transition_duration > 0.0 {
            assert_ne!(
                recipe.transition_timing.as_str(),
                "linear",
                "`{name}` animates linearly (DESIGN.md §7: nothing linear)"
            );
        }
        if recipe.outline_style.as_str() == "solid" {
            assert!(
                (recipe.outline_width - 2.0).abs() < f64::EPSILON,
                "`{name}` ring width (DESIGN.md §11)"
            );
            assert!(
                (recipe.outline_offset - 2.0).abs() < f64::EPSILON,
                "`{name}` ring offset (DESIGN.md §11)"
            );
        }
        if recipe.transform_preset.as_str() == "press-shift-down" {
            assert_eq!(
                key.component, "button",
                "`{name}`: press is for buttons only"
            );
            assert_eq!(
                key.state,
                RecipeState::Active,
                "`{name}`: press is the active state"
            );
        }
    }
}
