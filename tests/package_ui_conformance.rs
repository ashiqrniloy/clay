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

use clay::editor::theme::validate_active_theme_contrast;
use clay::packages::record::assemble_package_record;
use clay::protocol::ActiveTheme;
use serde_json::json;

const BUNDLED_THEMES: &[(&str, &str)] = &[
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

/// Phase 20.7 task 5: every bundled theme package is inert style-data, delegates
/// the SDUI palette to the core token fallback (zero `designTokens`
/// contributions so every required SDUI token resolves through the documented
/// core fallback — token coverage by construction), and meets WCAG AA on every
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

        // Token-coverage invariant: bundled themes override only editor
        // `textStyles`; they contribute zero SDUI `designTokens`, so every
        // required SDUI token resolves through the documented core fallback.
        // This pins the core SDUI palette as the AA-bearing surface and means a
        // future `designTokens`-bearing theme package enters this matrix with a
        // real palette to validate rather than free-riding on the core fallback.
        assert!(
            record.contributions.design_tokens.is_empty(),
            "{specifier} must contribute no SDUI designTokens (core fallback covers the SDUI palette)"
        );

        // Contrast guard (Plan 068 task 3): the resolved SDUI palette meets WCAG
        // AA on every required pair. With no `designTokens` the snapshot
        // resolves to the core fallback, so this pins the core palette to AA.
        let snapshot = ActiveTheme {
            specifier: specifier.to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        };
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
        "{}/.agents/skills/clay-ui/references/components.md",
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

/// Extract the kind names from the `ComponentKind::parse` match arms in
/// `src/shell/components.rs`. This is the code enum (set C). Only the
/// `impl ComponentKind` block is scanned so `DeferredComponentKind::parse`
/// (e.g. `table`) does not leak in.
fn catalog_enum_kinds() -> Vec<String> {
    let path = format!("{}/src/shell/components.rs", manifest_dir());
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read components.rs ({path}): {err}"));
    let impl_block = src
        .split("impl ComponentKind {")
        .nth(1)
        .expect("components.rs must have an `impl ComponentKind` block");
    let impl_body = impl_block.split("\n}\n").next().unwrap_or(impl_block);
    let mut kinds = Vec::new();
    for line in impl_body.lines() {
        let trimmed = line.trim();
        // Match arms: `"editorView" => Some(Self::EditorView),` — the only lines
        // in `impl ComponentKind` containing `=> Some(Self::` are `parse` arms.
        if !trimmed.contains("=> Some(Self::") {
            continue;
        }
        // Extract the first double-quoted token (the kind string).
        if let (Some(start), Some(end)) = (trimmed.find('"'), trimmed.rfind('"'))
            && start < end
        {
            kinds.push(trimmed[start + 1..end].to_string());
        }
    }
    kinds.sort();
    kinds
}

/// Phase 20.7 task 5: the component catalog is drift-free across the doc table,
/// the `ComponentKind` enum, and the `component_state_palette` paint path. A
/// kind added to the doc without an enum variant (or vice versa), or an enum
/// variant without a `component_state_palette` match arm, fails here. Closes the
/// code-vs-catalog drift gap identified in Plan 068 task 2.
#[test]
fn catalog_is_drift_free_across_doc_enum_and_paint_path() {
    let doc_kinds = catalog_doc_kinds();
    let enum_kinds = catalog_enum_kinds();
    assert_eq!(
        doc_kinds, enum_kinds,
        "components.md implemented kinds must match ComponentKind::parse variants exactly"
    );

    // Every catalog kind must have a `component_state_palette` match arm in the
    // paint path. We check the palette fn body (not the whole file) so a kind
    // string appearing only in an unrelated comment does not satisfy the guard.
    let sdui_path = format!("{}/src/masonry_sdui.rs", manifest_dir());
    let sdui_src =
        fs::read_to_string(&sdui_path).unwrap_or_else(|err| panic!("read masonry_sdui.rs: {err}"));
    let palette_body = sdui_src
        .split("fn component_state_palette(")
        .nth(1)
        .expect("component_state_palette must exist in src/masonry_sdui.rs");
    // The palette fn ends at the next `#[test]`-bounded fn in the test module;
    // for the non-test definition, the fn body ends at the next top-level
    // `pub(crate) fn`/`fn ` at the same indentation. Slice to the closing brace
    // of the fn by finding the next `\n    fn ` (test-helpers are indented) or
    // `\npub(crate) fn ` boundary, whichever comes first.
    let palette_fn_end = palette_body
        .find("\n    fn ")
        .or_else(|| palette_body.find("\npub(crate) fn "))
        .unwrap_or(palette_body.len());
    let palette_fn = &palette_body[..palette_fn_end];

    for kind in &enum_kinds {
        let needle = format!("\"{kind}\"");
        assert!(
            palette_fn.contains(&needle),
            "component_state_palette must have a match arm for catalog kind `{kind}` (paint path drift)"
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
        "{}/.agents/skills/clay-ui/references/components.md",
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
/// arms in `src/shell/theme.rs`) must match the "Core Tokens (implemented)"
/// tables in `references/tokens.md` exactly. A token added to code without a
/// doc row (or vice versa) fails here.
#[test]
fn core_token_catalog_matches_tokens_md() {
    // Code set: `core_theme_value` match arms — `"token" => CoreThemeValue {`.
    let theme_path = format!("{}/src/shell/theme.rs", manifest_dir());
    let theme_src =
        fs::read_to_string(&theme_path).unwrap_or_else(|err| panic!("read theme.rs: {err}"));
    let body = theme_src
        .split("fn core_theme_value(")
        .nth(1)
        .expect("theme.rs must define `core_theme_value`");
    let body = body.split("\n}\n").next().unwrap_or(body);
    let mut code_tokens = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("=> CoreThemeValue") {
            continue;
        }
        if let (Some(start), Some(end)) = (trimmed.find('"'), trimmed.rfind('"'))
            && start < end
        {
            code_tokens.push(trimmed[start + 1..end].to_string());
        }
    }
    code_tokens.sort();
    code_tokens.dedup();

    // Doc set: the "Core Tokens (implemented)" section tables. Token names
    // contain a `.` (e.g. `surface.main`); the "Token Types" table above lists
    // type names (`color-role`, `spacing`) without a dot, so the `.` filter
    // excludes them.
    let tokens_path = format!(
        "{}/.agents/skills/clay-ui/references/tokens.md",
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
// absence. See `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
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
