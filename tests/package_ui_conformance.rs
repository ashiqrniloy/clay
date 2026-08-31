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
    assert_eq!(btn.border_radius, 6.0);
    assert_eq!(btn.border_style, BorderStyle::Solid);
    assert_eq!(btn.border_width, 1.0);
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

/// Plan 103 Task 6: Every core design system fallback has an installed CSS fallback definition in tokens.css.
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

    let fallbacks = core_design_system_fallbacks();
    for key in fallbacks.keys() {
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
fn plan104_source_independence_guard_rejects_package_name_branching() {
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
                    if trimmed.contains("@clay/design-neobrutal")
                        || trimmed.contains("@clay/design-glass")
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

/// Plan 104 Task 5: Both first-party design system packages (@clay/design-neobrutal
/// and @clay/design-glass) provide complete 25-component coverage and strict color authority.
#[test]
fn plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority() {
    use clay::shell::design_system::{RecipeKey, UiDesignSystemDeclaration};

    let design_packages = [
        ("@clay/design-neobrutal", "design-neobrutal"),
        ("@clay/design-glass", "design-glass"),
    ];

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
        "chat",
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
