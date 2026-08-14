use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn documentation_contracts() -> serde_json::Value {
    serde_json::from_str(&read("docs/reference/documentation-contracts.json"))
        .expect("parse docs/reference/documentation-contracts.json")
}

fn contract_entries<'a>(contracts: &'a serde_json::Value, group: &str) -> &'a [serde_json::Value] {
    contracts
        .get(group)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("documentation-contracts.json missing array {group}"))
}

fn required_string<'a>(entry: &'a serde_json::Value, field: &str, group: &str) -> &'a str {
    entry
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            panic!("documentation-contracts.json {group} entry missing {field}: {entry}")
        })
}

fn markdown_files(directory: &str) -> BTreeSet<String> {
    fs::read_dir(root().join(directory))
        .unwrap_or_else(|error| panic!("read {directory}: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            path.strip_prefix(root())
                .expect("documentation path under repository")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn index_links(index_path: &str, document_path: &str) -> bool {
    let index = read(index_path);
    let relative = Path::new(document_path)
        .strip_prefix(Path::new(index_path).parent().unwrap_or(Path::new("")))
        .unwrap_or(Path::new(document_path))
        .to_string_lossy()
        .replace('\\', "/");
    index.contains(&format!("]({relative})")) || index.contains(&format!("]({document_path})"))
}

fn validate_security_markers(path: &str, text: &str, markers: &[serde_json::Value]) -> Vec<String> {
    markers
        .iter()
        .filter_map(|marker| marker.as_str())
        .filter(|marker| !text.contains(marker))
        .map(|marker| format!("{path}: missing security marker {marker:?}"))
        .collect()
}

#[test]
fn documentation_contract_inventory_is_complete_and_indexed() {
    let contracts = documentation_contracts();
    assert_eq!(
        contracts
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "documentation-contracts.json schema_version must be 1"
    );
    assert!(
        read("docs/index.md").contains("reference/documentation-contracts.json"),
        "docs/index.md must link documentation-contracts.json"
    );

    for (group, directory) in [
        ("primitive_documents", "docs/reference/primitives"),
        ("package_documents", "docs/reference/packages"),
    ] {
        let entries = contract_entries(&contracts, group);
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for entry in entries {
            let id = required_string(entry, "id", group);
            let path = required_string(entry, "path", group);
            assert!(ids.insert(id), "{group}: duplicate id {id}");
            assert!(
                paths.insert(path.to_string()),
                "{group}: duplicate path {path}"
            );
            let text = read(path);
            assert!(
                text.starts_with("# "),
                "{path}: document must start with one H1 heading"
            );
            assert!(
                text.contains("\n## "),
                "{path}: document must contain at least one H2 section"
            );

            let indexes = entry
                .get("indexes")
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("{path}: missing indexes array"));
            assert!(!indexes.is_empty(), "{path}: indexes must not be empty");
            for index in indexes {
                let index = index
                    .as_str()
                    .unwrap_or_else(|| panic!("{path}: index path must be a string"));
                assert!(
                    index_links(index, path),
                    "{path}: not linked from required index {index}"
                );
            }
        }
        assert_eq!(
            paths,
            markdown_files(directory),
            "{group}: inventory must enumerate every Markdown file in {directory} exactly once"
        );
    }
}

#[test]
fn primitive_registry_matrix_has_one_complete_row_per_primitive() {
    let registry = read("docs/reference/primitives/registry.md");
    let matrix = registry
        .split_once("## Category Matrix")
        .map(|(_, rest)| rest)
        .expect("registry.md missing Category Matrix")
        .split_once("## Primitive Category Notes")
        .map(|(matrix, _)| matrix)
        .expect("registry.md missing Primitive Category Notes");
    let mut primitives = BTreeSet::new();
    let mut rows = 0;

    for line in matrix.lines().filter(|line| line.starts_with('|')) {
        let fields: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        if fields.first() == Some(&"Primitive")
            || fields
                .iter()
                .all(|field| field.chars().all(|ch| ch == '-' || ch == ' '))
        {
            continue;
        }
        assert_eq!(fields.len(), 15, "registry.md malformed matrix row: {line}");
        let primitive = fields[0].trim_matches('`');
        assert!(
            primitives.insert(primitive.to_string()),
            "registry.md duplicate primitive row {primitive}"
        );
        for (index, field) in fields.iter().enumerate() {
            assert!(
                !field.is_empty(),
                "registry.md primitive {primitive} has empty column {index}"
            );
        }
        assert!(
            matches!(
                fields[14],
                "Exists" | "Extend" | "Exists/Extend" | "New" | "Deferred"
            ),
            "registry.md primitive {primitive} has invalid status {}",
            fields[14]
        );
        rows += 1;
    }
    assert!(
        rows >= 20,
        "registry.md unexpectedly contains only {rows} primitive rows"
    );
}

#[test]
fn narrow_security_markers_are_present_with_actionable_paths() {
    let contracts = documentation_contracts();
    for entry in contract_entries(&contracts, "security_contracts") {
        let path = required_string(entry, "path", "security_contracts");
        let markers = entry
            .get("markers")
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("{path}: missing markers array"));
        assert!(
            !markers.is_empty(),
            "{path}: security marker set must not be empty"
        );
        let errors = validate_security_markers(path, &read(path), markers);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    let errors = validate_security_markers(
        "docs/example.md",
        "# Example\n\nHarmless prose.\n",
        &[serde_json::Value::String("trusted boundary".to_string())],
    );
    assert_eq!(
        errors,
        ["docs/example.md: missing security marker \"trusted boundary\""]
    );
}

#[test]
fn semantically_harmless_prose_is_not_a_contract() {
    let markers = [serde_json::Value::String(
        "authority remains host-owned".to_string(),
    )];
    let first =
        "# Page\n\n## Security\n\nauthority remains host-owned\n\nOriginal explanatory prose.";
    let rewritten = "# Page\n\n## Security\n\nauthority remains host-owned\n\nCompletely rewritten explanation.";
    assert!(validate_security_markers("page.md", first, &markers).is_empty());
    assert!(validate_security_markers("page.md", rewritten, &markers).is_empty());
}

#[test]
fn wiki_index_links_every_wiki_page() {
    fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read wiki entry").path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }

    let wiki_root = root().join("docs/wiki");
    let index = read("docs/wiki/index.md");
    let mut files = Vec::new();
    collect(&wiki_root, &mut files);
    for path in files {
        if path == wiki_root.join("index.md") {
            continue;
        }
        let relative = path
            .strip_prefix(&wiki_root)
            .expect("wiki page under wiki root")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            index.contains(&format!("]({relative})")),
            "docs/wiki/index.md missing link to {relative}"
        );
    }
}

#[test]
fn phase20_1_ui_design_language_primitive_review_is_linked_and_complete() {
    let path = "docs/wiki/modules/phase20.1-ui-design-language-primitive-review.md";
    let review = read(path);

    assert!(
        index_links("docs/wiki/index.md", path),
        "Phase 20.1 primitive review must be linked from the wiki index"
    );
    for primitive in [
        "`ThemeTokenType`",
        "`ThemeTokenResolver`",
        "`SduiThemeStyle`",
        "`StyleRegistry`",
        "`ActiveTheme`",
        "`TypographyRegistry`",
        "`ActiveTypography`",
        "`UiTextVariant`",
        "Component style validation",
        "Package theme-token declarations",
        "Panel/slot geometry",
    ] {
        assert!(
            review.contains(primitive),
            "primitive review missing {primitive}"
        );
    }
    for contract in [
        "## Reusable Capability Before New Code",
        "## Locked Generic Phase 20.1 Gaps",
        "## Additive Compatibility Contract",
        "## Hot-Path Boundary",
        "## Security Boundary",
        "## Phase Boundary",
        "`dimension`, `elevation`, `motion-duration`, `z-level`, and `density`",
        "`UiTypographyHierarchy`",
        "`ResolvedUiTheme`",
        "No new component kind",
    ] {
        assert!(
            review.contains(contract),
            "primitive review missing {contract}"
        );
    }
}

#[test]
fn plan061_runtime_package_authority_rebaseline_matches_source_inventory() {
    fn marked_section<'a>(text: &'a str, name: &str) -> &'a str {
        let start = format!("<!-- plan061-task1-{name}:start -->");
        let end = format!("<!-- plan061-task1-{name}:end -->");
        text.split_once(&start)
            .and_then(|(_, remaining)| remaining.split_once(&end))
            .map(|(section, _)| section)
            .unwrap_or_else(|| panic!("Plan 061 must contain {name} inventory markers"))
    }

    fn assert_exact_inventory(section: &str, values: &BTreeSet<String>, expected: usize) {
        assert_eq!(values.len(), expected, "unexpected source inventory count");
        for value in values {
            let token = format!("`{value}`");
            assert_eq!(
                section.matches(&token).count(),
                1,
                "Plan 061 inventory must classify {token} exactly once"
            );
        }
    }

    let plan = read("plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md");
    let ops_source = read("src/server/ops/mod.rs");
    let mut ops = BTreeSet::new();
    for extension_name in [
        "extension!(\n    clay_runtime_trusted_extension,",
        "extension!(\n    clay_runtime_package_extension,",
    ] {
        let body = ops_source
            .split_once(extension_name)
            .and_then(|(_, remaining)| remaining.split_once("\n);").map(|(body, _)| body))
            .unwrap_or_else(|| panic!("find {extension_name} op list"));
        for line in body.lines().map(str::trim) {
            if let Some(name) = line
                .strip_suffix(',')
                .filter(|name| name.starts_with("op_clay_"))
            {
                ops.insert(name.to_string());
            }
        }
    }
    assert_exact_inventory(marked_section(&plan, "op-inventory"), &ops, 82);

    let facades = read("src/server/facades.rs")
        .lines()
        .filter_map(|line| line.split_once("\"clay:").map(|(_, rest)| rest))
        .filter_map(|rest| rest.split_once('"').map(|(name, _)| format!("clay:{name}")))
        .collect::<BTreeSet<_>>();
    assert_exact_inventory(marked_section(&plan, "facade-inventory"), &facades, 22);

    let mut packages = BTreeSet::new();
    for entry in fs::read_dir(root().join("packages")).expect("read packages directory") {
        let package_json = entry
            .expect("read package entry")
            .path()
            .join("package.json");
        if !package_json.is_file() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(package_json).expect("read package manifest"))
                .expect("parse package manifest");
        if value.get("clay").is_some()
            && let Some(name) = value.get("name").and_then(serde_json::Value::as_str)
        {
            packages.insert(name.to_string());
        }
    }
    let package_section = marked_section(&plan, "package-inventory");
    assert_exact_inventory(package_section, &packages, 14);
    assert_eq!(package_section.matches("`packages/lsp-shared`").count(), 1);
}

/// Every RustSec exception ignored by cargo-audit must be documented with one
/// unexpired owner-reviewed expiry. CI invokes this test by name.
#[test]
fn phase20_1_token_catalog_is_complete_and_matches_core_registry() {
    let theme_source = read("src/shell/theme.rs");
    let tokens_doc = read(".agents/skills/clay-ui/references/tokens.md");

    // Extract every implemented core token name from `core_theme_value`.
    let mut core_tokens = BTreeSet::new();
    for line in theme_source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('"')
            && let Some((name, after)) = rest.split_once('"')
            && after.trim_start().starts_with("=> CoreThemeValue")
            && name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.')
        {
            core_tokens.insert(name.to_string());
        }
    }
    assert!(
        !core_tokens.is_empty(),
        "core token inventory must not be empty"
    );

    // Every implemented core token must appear in the catalog.
    let mut missing = Vec::new();
    for token in &core_tokens {
        if !tokens_doc.contains(&format!("`{token}`")) {
            missing.push(token.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "tokens.md missing implemented core tokens: {}",
        missing.join(", ")
    );

    // The ten typed domains are all documented.
    for token_type in [
        "color-role",
        "spacing",
        "radius",
        "typography",
        "opacity",
        "dimension",
        "elevation",
        "motion-duration",
        "z-level",
        "density",
    ] {
        assert!(
            tokens_doc.contains(token_type),
            "tokens.md missing token type {token_type:?}"
        );
    }

    // All seven semantic typography variants are documented.
    for variant in [
        "typography.body",
        "typography.title",
        "typography.status",
        "typography.display",
        "typography.section",
        "typography.detail",
        "typography.caption",
    ] {
        assert!(
            tokens_doc.contains(variant),
            "tokens.md missing typography variant {variant}"
        );
    }
}

#[test]
fn package_authoring_guide_documents_typed_tokens_and_typography_hierarchy() {
    let guide = read("docs/reference/packages/creating-packages.md");
    for token_type in [
        "color-role",
        "spacing",
        "radius",
        "typography",
        "opacity",
        "dimension",
        "elevation",
        "motion-duration",
        "z-level",
        "density",
    ] {
        assert!(
            guide.contains(token_type),
            "package authoring guide missing token type {token_type:?}"
        );
    }
    assert!(
        guide.contains("UiTypographyHierarchy"),
        "package authoring guide must document the user-owned typography hierarchy"
    );
    assert!(
        guide.contains("designTokens"),
        "package authoring guide must document the designTokens contribution contract"
    );
    for variant in [
        "typography.display",
        "typography.section",
        "typography.detail",
        "typography.caption",
    ] {
        assert!(
            guide.contains(variant),
            "package authoring guide missing typography variant {variant}"
        );
    }
    assert!(
        guide.contains("supply concrete scale ratios"),
        "package authoring guide must reject package-supplied concrete hierarchy scales"
    );
    assert!(
        guide.contains("Phase 20.1"),
        "package authoring guide must record the Phase 20.1 authoring contract"
    );
}

#[test]
fn component_catalog_status_partition_is_current() {
    // Phase 20.8: forward-looking catalog status-partition guard. Replaces the
    // frozen pre-Phase-20.5 snapshot (`planned_phase20_components_remain_marked_planned_or_reserved`)
    // which went stale when Phase 20.5 promoted `dropdown`/`collapse`/`modal` to
    // implemented and added `textInput`. The catalog is the single source of
    // truth; this guard fails if an implemented kind is demoted, a reserved/
    // planned kind is prematurely promoted, or the token consumption markers
    // drift.
    let components = read(".agents/skills/clay-ui/references/components.md");
    let tokens = read(".agents/skills/clay-ui/references/tokens.md");

    // `table` is the only reserved package-facing kind.
    assert!(
        components.contains("| `table` | reserved"),
        "`table` must remain reserved (no first-party need identified)"
    );

    // Phase 20.5 promoted these from reserved to implemented.
    for kind in ["dropdown", "collapse", "modal", "textInput"] {
        let row = format!("| `{kind}` | implemented");
        assert!(
            components.contains(&row),
            "Phase 20.5-promoted kind `{kind}` must be cataloged implemented, not reserved"
        );
    }

    // Composition-only planned surfaces stay planned (no premature promotion).
    for component in [
        "Tooltip",
        "Badge / tag",
        "Toast / notification",
        "`kbd` hint",
        "Icon slot",
    ] {
        let row_marker = format!("| {component} | planned |");
        assert!(
            components.contains(&row_marker),
            "planned composition surface {component} must remain planned, not implemented"
        );
    }

    // Phase 22.3 promoted Tabs from planned to implemented (shell-owned tab bar).
    assert!(
        components.contains("| Tabs | implemented |"),
        "Tabs must be marked implemented after Phase 22.3"
    );

    // Phase 20.3 Split divider and Phase 20.5 promoted composition surfaces are implemented.
    assert!(
        components.contains("| Split divider | implemented |"),
        "Split divider must be marked implemented after Phase 20.3"
    );
    for component in [
        "Pop-up / dialog",
        "Dropdown / select",
        "Multi-select",
        "Text input field",
        "Menu (context / menu bar)",
        "Completion pop-up (uplift)",
        "Command palette",
    ] {
        let row_marker = format!("| {component} | implemented |");
        assert!(
            components.contains(&row_marker),
            "composition surface {component} must be marked implemented after Phase 20.5"
        );
    }

    // Token consumption markers stay aligned with the phases that introduced them.
    assert!(
        tokens.contains("Phase 20.4 component uplift"),
        "tokens.md must mark elevation/motion/density consumption as Phase 20.4"
    );
    assert!(
        tokens.contains("Phase 20.5 overlay/menu component work"),
        "tokens.md must mark z-level consumption as Phase 20.5"
    );
}

#[test]
fn package_guide_documents_phase20_4_uplift() {
    // Plan 065 task 8: creating-packages.md records the Phase 20.4 restyling
    // contract — active-theme routing, state-complete components, spacing
    // rhythm, token-driven status bar insets, and the compatibility guarantee.
    let guide = read("docs/reference/packages/creating-packages.md");
    assert!(
        guide.contains("Phase 20.4 authoring contract"),
        "package authoring guide must have a Phase 20.4 authoring contract section"
    );
    assert!(
        guide.contains("ResolvedUiTheme") && guide.contains("from_ui_theme"),
        "Phase 20.4 section must document active-theme routing through ResolvedUiTheme"
    );
    for state in ["Rest", "Hover", "Active", "Focus", "Disabled"] {
        assert!(
            guide.contains(state),
            "Phase 20.4 section must reference InteractionState {state}"
        );
    }
    assert!(
        guide.contains("spacing.md") && guide.contains("spacing_scale"),
        "Phase 20.4 section must document the spacing rhythm (spacing.md × spacing_scale)"
    );
    assert!(
        guide.contains("status bar") && guide.contains("spacing.sm"),
        "Phase 20.4 section must document token-driven status bar insets"
    );
    assert!(
        guide.contains("Compatibility guarantee"),
        "Phase 20.4 section must state the compatibility guarantee"
    );
    assert!(
        guide.contains("no `ComponentKind`") && guide.contains("token-name change"),
        "Phase 20.4 section must guarantee no ComponentKind/style-variable/token-name change"
    );
}

#[test]
fn clay_ui_catalog_notes_state_completeness() {
    // Plan 065 task 8: components.md notes all five interaction states and the
    // spacing rhythm for each implemented kind.
    let components = read(".agents/skills/clay-ui/references/components.md");
    assert!(
        components.contains("Phase 20.4 interaction-state and spacing rhythm notes"),
        "components.md must have a Phase 20.4 interaction-state/spacing section"
    );
    for kind in [
        "button",
        "list",
        "label",
        "statusItem",
        "panel",
        "overlay",
        "editorView",
        "flex",
        "stack",
        "scroll",
        "portal",
    ] {
        assert!(
            components.contains(kind),
            "components.md must reference implemented kind {kind}"
        );
    }
    for state in ["Rest", "Hover", "Active", "Focus", "Disabled"] {
        assert!(
            components.contains(state),
            "components.md must reference InteractionState {state}"
        );
    }
    assert!(
        components.contains("component_state_color"),
        "components.md must reference the component_state_color state mapping"
    );
    assert!(
        components.contains("spacing.md") && components.contains("spacing_scale"),
        "components.md must document the spacing rhythm"
    );
}

#[test]
fn primitives_reference_documents_component_state_color() {
    // Plan 065 task 8: ui-chrome-primitives.md records the Phase 20.4
    // state-color helpers.
    let doc = read("docs/reference/primitives/ui-chrome-primitives.md");
    assert!(
        doc.contains("component_state_color"),
        "ui-chrome-primitives.md must record the component_state_color helper"
    );
    assert!(
        doc.contains("list_row_fill_color"),
        "ui-chrome-primitives.md must record the list_row_fill_color helper"
    );
    assert!(
        doc.contains("disabled_text_color"),
        "ui-chrome-primitives.md must record the disabled_text_color helper"
    );
    assert!(
        doc.contains("State-color helpers (Phase 20.4)"),
        "ui-chrome-primitives.md must have a Phase 20.4 state-color helpers section"
    );
    // The helper mapping must be documented as token-driven.
    assert!(
        doc.contains("surface.hover")
            && doc.contains("surface.active")
            && doc.contains("surface.disabled"),
        "ui-chrome-primitives.md must document the state→token mapping"
    );
}

/// Every RustSec exception ignored by cargo-audit must be documented with one
/// unexpired owner-reviewed expiry. CI invokes this test by name.
#[test]
fn audit_exceptions_are_documented_and_unexpired() {
    let audit_toml = read(".cargo/audit.toml");
    let security_doc = read("docs/development/security.md");
    let ignored: Vec<_> = audit_toml
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')
                .map(|rest| rest.trim_end_matches(',').trim_end_matches('"'))
        })
        .filter(|id| id.starts_with("RUSTSEC-"))
        .collect();
    assert!(
        !ignored.is_empty(),
        "audit.toml must list ignored advisories explicitly"
    );
    for id in &ignored {
        assert!(
            security_doc.contains(id),
            "ignored advisory {id} missing from docs/development/security.md"
        );
    }

    let today = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("date command available on Linux");
    let today = String::from_utf8(today.stdout)
        .expect("UTF-8 date")
        .trim()
        .to_string();
    let expiries: Vec<_> = security_doc
        .lines()
        .filter_map(|line| line.split_once("**Expiry:**").map(|(_, rest)| rest))
        .map(|rest| {
            rest.trim()
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
                .collect::<String>()
        })
        .collect();
    assert_eq!(
        expiries.len(),
        ignored.len(),
        "each ignored advisory needs exactly one expiry"
    );
    for expiry in expiries {
        assert_eq!(
            expiry.len(),
            10,
            "expiry must be YYYY-MM-DD, got {expiry:?}"
        );
        assert!(
            expiry > today,
            "audit exception expired on {expiry} (today {today})"
        );
    }
}

/// Plan 086 task 6: every current dependency warning is classified in
/// `docs/development/security.md` with path, upstream reference, and recheck
/// date; directly fixed advisories appear only in the remediated table and
/// can never be re-introduced as ignores.
#[test]
fn classified_dependency_warnings_and_remediated_ids_are_documented() {
    let security_doc = read("docs/development/security.md");
    let audit_toml = read(".cargo/audit.toml");

    for id in [
        "RUSTSEC-2025-0141",
        "RUSTSEC-2024-0436",
        "RUSTSEC-2026-0192",
    ] {
        let row = security_doc
            .lines()
            .find(|line| line.contains(id))
            .unwrap_or_else(|| panic!("{id} missing from docs/development/security.md"));
        assert!(
            row.contains("→"),
            "{id} classification row needs the dependency path"
        );
        assert!(
            row.contains("https://"),
            "{id} classification row needs the upstream reference"
        );
        assert!(
            row.contains("recheck") && row.contains("**2026-"),
            "{id} classification row needs a recheck date"
        );
    }

    let classified = security_doc
        .split("## Remediated vulnerabilities")
        .next()
        .expect("remediated section present");
    for id in [
        "RUSTSEC-2026-0221",
        "RUSTSEC-2026-0233",
        "RUSTSEC-2026-0234",
        "RUSTSEC-2026-0235",
    ] {
        assert!(
            !classified.contains(id),
            "{id} must not appear outside the remediated table"
        );
        assert!(
            !audit_toml.contains(id),
            "directly fixed advisory {id} must never be added to .cargo/audit.toml"
        );
    }
}

#[test]
fn documentation_validators_do_not_mutate_files() {
    let contract_path = root().join("docs/reference/documentation-contracts.json");
    let registry_path = root().join("docs/generated/clay-js-api-registry.json");
    let before = (
        fs::read(&contract_path).unwrap(),
        fs::read(&registry_path).unwrap(),
    );
    let _ = documentation_contracts();
    let _ = markdown_files("docs/reference/primitives");
    let after = (
        fs::read(&contract_path).unwrap(),
        fs::read(&registry_path).unwrap(),
    );
    assert_eq!(
        before, after,
        "documentation tests must not mutate source or generated artifacts"
    );
}

#[test]
fn phase20_2_primitive_documentation_exists_and_is_linked() {
    // Plan 063 task 6: verify Phase 20.2 primitive documentation exists and is
    // linked from the primitive index, docs index, and wiki index.
    let primitive_doc = read("docs/reference/primitives/ui-chrome-primitives.md");
    assert!(
        primitive_doc.contains("UI Chrome Primitives (Phase 20.2)"),
        "ui-chrome-primitives.md must document Phase 20.2 primitives"
    );
    assert!(
        primitive_doc.contains("paint_divider"),
        "ui-chrome-primitives.md must list paint_divider primitive"
    );
    assert!(
        primitive_doc.contains("paint_focus_ring"),
        "ui-chrome-primitives.md must list paint_focus_ring primitive"
    );
    assert!(
        primitive_doc.contains("paint_panel_chrome"),
        "ui-chrome-primitives.md must list paint_panel_chrome primitive"
    );
    assert!(
        primitive_doc.contains("paint_scroll_chrome"),
        "ui-chrome-primitives.md must list paint_scroll_chrome primitive"
    );
    assert!(
        primitive_doc.contains("paint_badge"),
        "ui-chrome-primitives.md must list paint_badge primitive"
    );
    assert!(
        primitive_doc.contains("paint_kbd_hint"),
        "ui-chrome-primitives.md must list paint_kbd_hint primitive"
    );
    assert!(
        primitive_doc.contains("paint_icon_slot"),
        "ui-chrome-primitives.md must list paint_icon_slot primitive"
    );
    assert!(
        primitive_doc.contains("paint_tooltip_shell"),
        "ui-chrome-primitives.md must list paint_tooltip_shell primitive"
    );
    assert!(
        primitive_doc.contains("paint_scrim"),
        "ui-chrome-primitives.md must list the Phase 24.4 paint_scrim primitive"
    );

    // Verify the primitive doc is linked from the primitive index.
    let primitive_index = read("docs/reference/primitives/index.md");
    assert!(
        primitive_index.contains("[UI Chrome Primitives](ui-chrome-primitives.md)"),
        "docs/reference/primitives/index.md must link ui-chrome-primitives.md"
    );

    // Verify the primitive doc is linked from the docs index.
    let docs_index = read("docs/index.md");
    assert!(
        docs_index.contains("[UI Chrome Primitives](reference/primitives/ui-chrome-primitives.md)"),
        "docs/index.md must link ui-chrome-primitives.md"
    );

    // Verify the primitive doc is linked from the wiki index.
    let wiki_index = read("docs/wiki/index.md");
    assert!(
        wiki_index.contains(
            "[UI Chrome Primitives Reference](../reference/primitives/ui-chrome-primitives.md)"
        ),
        "docs/wiki/index.md must link ui-chrome-primitives.md"
    );

    // Verify components.md lists all eight primitives plus the Phase 24.4 scrim.
    let components = read(".agents/skills/clay-ui/references/components.md");
    assert!(
        components.contains("## Clay-Native Chrome Primitives (internal)"),
        "components.md must have a Clay-Native Chrome Primitives section"
    );
    for primitive in [
        "paint_divider",
        "paint_focus_ring",
        "paint_panel_chrome",
        "paint_scroll_chrome",
        "paint_badge",
        "paint_kbd_hint",
        "paint_icon_slot",
        "paint_tooltip_shell",
        "paint_scrim",
    ] {
        let row_marker = format!("| `{primitive}` | internal |");
        assert!(
            components.contains(&row_marker),
            "components.md must list {primitive} primitive in the Clay-Native Chrome Primitives section"
        );
    }
}

#[test]
fn phase20_4_core_component_uplift_primitive_review_is_linked_and_complete() {
    // Plan 065 (Phase 20.4) task 12: verify the Phase 20.4 primitive-review
    // wiki page exists, is linked from the wiki index, and records the restyle-
    // only uplift inventory, state helpers, compatibility contract, and phase
    // boundary.
    let path = "docs/wiki/modules/phase20.4-core-component-uplift-primitive-review.md";
    let review = read(path);

    assert!(
        index_links("docs/wiki/index.md", path),
        "Phase 20.4 primitive review must be linked from the wiki index"
    );
    for section in [
        "## Reusable Capability Before New Code",
        "## Locked Generic Phase 20.4 Gaps (closed)",
        "## State-Color Helpers",
        "## Additive Compatibility Contract",
        "## Hot-Path Boundary",
        "## Security Boundary",
        "## Phase Boundary",
    ] {
        assert!(
            review.contains(section),
            "Phase 20.4 primitive review missing {section}"
        );
    }
    for item in [
        "`component_state_color`",
        "`list_row_fill_color`",
        "`disabled_text_color`",
        "`SduiThemeStyle::from_ui_theme`",
        "`theme_style`",
        "`ResolvedUiTheme`",
        "`InteractionState`",
        "`surface.hover`",
        "`surface.active`",
        "`surface.disabled`",
        "`opacity.disabled`",
        "`spacing.md`",
        "`spacing_scale`",
        "`paint_focus_ring`",
        "`scrollbar_interaction_state`",
        "No new component kind",
        "Zero breaking changes",
    ] {
        assert!(
            review.contains(item),
            "Phase 20.4 primitive review missing {item}"
        );
    }
}

#[test]
fn no_component_kind_or_token_renamed() {
    // Phase 20.8: no-rename regression guard for the Phase 20.1/20.4 inventory.
    // Phase 20.5 promoted `dropdown`/`collapse`/`modal` from reserved to
    // implemented and added `textInput`, so the original "11 implemented + 4
    // reserved" partition is now "15 implemented + 1 reserved (`table`)". This
    // guard pins the current partition: every cataloged kind still parses in
    // `components.rs` with the matching status, and the Phase 20.1 state tokens
    // remain core tokens (additive-only). The enum<->catalog-table agreement is
    // also enforced by `tests/package_ui_conformance.rs::catalog_is_drift_free_across_doc_enum_and_paint_path`;
    // this test additionally pins specific kind names and the Phase 20.1 tokens.
    let components_src = read("src/shell/components.rs");
    let components_doc = read(".agents/skills/clay-ui/references/components.md");
    let tokens_src = read("src/shell/theme.rs");
    let tokens_doc = read(".agents/skills/clay-ui/references/tokens.md");

    // 15 implemented ComponentKind entries still parse and are cataloged implemented.
    for kind in [
        "editorView",
        "panel",
        "label",
        "button",
        "list",
        "flex",
        "stack",
        "overlay",
        "scroll",
        "portal",
        "statusItem",
        "dropdown",
        "collapse",
        "modal",
        "textInput",
    ] {
        let marker = format!("\"{kind}\" => Some(Self::");
        assert!(
            components_src.contains(&marker),
            "ComponentKind {kind} must still parse in components.rs (not renamed/removed)"
        );
        let row = format!("| `{kind}` | implemented");
        assert!(
            components_doc.contains(&row),
            "components.md must still catalog {kind} as implemented"
        );
    }
    // `table` is the only reserved kind.
    {
        let kind = "table";
        let marker = format!("\"{kind}\" => Some(Self::");
        assert!(
            components_src.contains(&marker),
            "reserved ComponentKind {kind} must still parse in components.rs"
        );
        let row = format!("| `{kind}` | reserved");
        assert!(
            components_doc.contains(&row),
            "components.md must still catalog {kind} as reserved"
        );
    }

    // Phase 20.1 state tokens are still core tokens (additive-only).
    for token in [
        "surface.hover",
        "surface.active",
        "surface.disabled",
        "text.disabled",
        "accent.primary",
        "border.focus",
        "opacity.disabled",
    ] {
        let core_marker = format!("\"{token}\" => CoreThemeValue");
        assert!(
            tokens_src.contains(&core_marker),
            "core token {token} must still exist in theme.rs (not renamed/removed)"
        );
        assert!(
            tokens_doc.contains(token),
            "tokens.md must still catalog core token {token}"
        );
    }
}

#[test]
fn existing_packages_render_unchanged() {
    // Plan 065 (Phase 20.4) task 11: first-party packages render unchanged —
    // no package source file may reference Phase 20.4 internal Rust paint/state
    // helpers (the compatibility boundary: packages declare inert components
    // and typed tokens only; they cannot reach pub(crate) paint internals). The
    // full integration suite (which loads @clay/* packages) passing green is
    // the runtime half of this gate; this is the static boundary half.
    let package_root = root().join("packages");
    let internal_helpers = [
        "component_state_color",
        "list_row_fill_color",
        "disabled_text_color",
        "from_ui_theme",
        "theme_style",
        "interaction_state",
        "scrollbar_interaction_state",
        "set_pointer_pos",
        "set_focused_action",
        "is_focused",
        "SduiThemeStyle",
    ];
    let mut scanned = 0;
    for entry in fs::read_dir(&package_root).expect("read packages/ directory") {
        let pkg = entry.expect("package entry").path();
        for src in ["src", "dist"] {
            let dir = pkg.join(src);
            if !dir.is_dir() {
                continue;
            }
            for file in walk_js(&dir) {
                scanned += 1;
                let text = fs::read_to_string(&file)
                    .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
                for helper in internal_helpers {
                    assert!(
                        !text.contains(helper),
                        "{} must not reference Phase 20.4 internal Rust helper {helper}",
                        file.display()
                    );
                }
            }
        }
    }
    assert!(scanned > 0, "must scan at least one package source file");
}

fn walk_js(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.extend(walk_js(&p));
            } else if matches!(p.extension().and_then(|e| e.to_str()), Some("js" | "ts")) {
                out.push(p);
            }
        }
    }
    out
}

// --- Phase 20.8: UI reference documentation and agent convention drift gates ---

/// Parse a Markdown table region's rows into (raw-kind-cell, raw-status-cell)
/// pairs. `start` is the section header line that begins the table; the region
/// ends at the next `## ` or `### ` header. Only `|`-delimited rows are
/// returned; header/separator rows are skipped.
fn parse_component_table(text: &str, start_header: &str) -> Vec<(String, String)> {
    let region = text
        .split_once(start_header)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("section {start_header:?} not found"));
    let region = region
        .split_once("\n### ")
        .or_else(|| region.split_once("\n## "))
        .map(|(region, _)| region)
        .unwrap_or(region);
    let mut rows = Vec::new();
    for line in region.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let fields: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if fields.len() < 2 {
            continue;
        }
        // Skip the header row and the `|---|---|` separator row.
        if fields
            .iter()
            .all(|field| field.chars().all(|ch| ch == '-' || ch == ' '))
            || fields.first() == Some(&"Kind")
            || fields.first() == Some(&"Component kind")
        {
            continue;
        }
        rows.push((fields[0].to_string(), fields[1].to_string()));
    }
    rows
}

/// Extract backtick-wrapped kind names from a table cell, splitting grouped
/// rows like `` `stack` / `overlay` `` into [`stack`, `overlay`].
fn kind_names(cell: &str) -> Vec<String> {
    cell.split('/')
        .map(|part| part.trim())
        .filter_map(|part| {
            part.strip_prefix('`')
                .and_then(|rest| rest.strip_suffix('`'))
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn ui_components_page_is_linked_from_master_index() {
    // Phase 20.8 task 3/6: the new UI components navigation page is linked from
    // the master documentation index.
    let index = read("docs/index.md");
    assert!(
        index.contains("](reference/ui-components.md)"),
        "docs/index.md must link reference/ui-components.md under Developer Guides"
    );
}

#[test]
fn ui_components_page_links_catalog_and_token_tables() {
    // Phase 20.8 task 3/6: the UI components page links the authoritative
    // component catalog and token catalog (single source of truth), the chrome
    // primitives reference, the package authoring guide, and the Phase 20.7
    // conformance wiki page.
    let page = read("docs/reference/ui-components.md");
    assert!(
        page.contains("components.md)"),
        "docs/reference/ui-components.md must link the component catalog (components.md)"
    );
    assert!(
        page.contains("tokens.md)"),
        "docs/reference/ui-components.md must link the token catalog (tokens.md)"
    );
    assert!(
        page.contains("primitives/ui-chrome-primitives.md)"),
        "docs/reference/ui-components.md must link the UI chrome primitives reference"
    );
    assert!(
        page.contains("packages/creating-packages.md)"),
        "docs/reference/ui-components.md must link the package authoring guide"
    );
    assert!(
        page.contains("phase20.7-package-ui-conformance-and-aesthetic-guardrails.md)"),
        "docs/reference/ui-components.md must link the Phase 20.7 conformance wiki page"
    );
    assert!(
        page.contains("create-plan/references/clay.md)"),
        "docs/reference/ui-components.md must link the create-plan UI requirements"
    );
}

#[test]
fn creating_packages_status_markers_match_clay_ui_catalog() {
    // Phase 20.8 task 4/6: the `creating-packages.md` Components table status
    // markers agree with the `clay-ui` component catalog partition. A kind
    // marked implemented/reserved in the catalog must be marked implemented/
    // reserved in the guide (not planned/deferred), and vice versa.
    let guide = read("docs/reference/packages/creating-packages.md");
    let catalog = read(".agents/skills/clay-ui/references/components.md");

    let guide_rows = parse_component_table(&guide, "## Components");
    let catalog_rows = parse_component_table(&catalog, "## Package-Facing Component Kinds");

    // Build kind -> (guide_status, catalog_status) from the parsed tables.
    let mut guide_status = std::collections::BTreeMap::new();
    for (kind_cell, status) in &guide_rows {
        for kind in kind_names(kind_cell) {
            guide_status.insert(kind, status.to_lowercase());
        }
    }
    let mut catalog_status = std::collections::BTreeMap::new();
    for (kind_cell, status) in &catalog_rows {
        for kind in kind_names(kind_cell) {
            catalog_status.insert(kind, status.to_lowercase());
        }
    }

    // Every catalog kind must appear in the guide and agree on the
    // implemented/reserved partition.
    for (kind, cat_status) in &catalog_status {
        let guide = guide_status.get(kind).unwrap_or_else(|| {
            panic!("creating-packages.md Components table missing kind `{kind}`")
        });
        let cat_implemented = cat_status.contains("implemented");
        let cat_reserved = cat_status.contains("reserved");
        let guide_implemented = guide.contains("implemented");
        let guide_reserved = guide.contains("reserved");
        assert_eq!(
            cat_implemented, guide_implemented,
            "kind `{kind}` implemented/partition mismatch: catalog={cat_status:?} guide={guide:?}"
        );
        assert_eq!(
            cat_reserved, guide_reserved,
            "kind `{kind}` reserved/partition mismatch: catalog={cat_status:?} guide={guide:?}"
        );
    }
    // The guide must not invent kinds absent from the catalog.
    for kind in guide_status.keys() {
        assert!(
            catalog_status.contains_key(kind),
            "creating-packages.md lists kind `{kind}` absent from the clay-ui catalog"
        );
    }
    // Sanity: the partition is non-trivial (implemented and reserved both present).
    assert!(
        catalog_status
            .values()
            .any(|status| status.contains("implemented"))
            && catalog_status
                .values()
                .any(|status| status.contains("reserved")),
        "catalog partition must contain both implemented and reserved kinds"
    );
}

#[test]
fn create_plan_ui_requirements_name_existing_catalog_files() {
    // Phase 20.8 task 5/6: the create-plan UI requirements reference the catalog
    // files and the UI components navigation page that actually exist.
    let requirements = read(".agents/skills/create-plan/references/clay.md");
    for reference in [
        "components.md",
        "tokens.md",
        "creating-packages.md",
        "ui-components.md",
        ".agents/skills/clay-ui",
    ] {
        assert!(
            requirements.contains(reference),
            "create-plan UI requirements must reference {reference:?} (exists on disk)"
        );
        // Each referenced path that is a concrete file must exist.
        if let Some(path) = match reference {
            "components.md" => Some(".agents/skills/clay-ui/references/components.md"),
            "tokens.md" => Some(".agents/skills/clay-ui/references/tokens.md"),
            "creating-packages.md" => Some("docs/reference/packages/creating-packages.md"),
            "ui-components.md" => Some("docs/reference/ui-components.md"),
            _ => None,
        } {
            assert!(
                root().join(path).is_file(),
                "create-plan UI requirement references {reference:?} but {path} does not exist"
            );
        }
    }
}
