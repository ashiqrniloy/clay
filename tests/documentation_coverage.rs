//! Parity ledger guards for Plan 097 Phase 1 (Tauri/React migration).
//!
//! Three deterministic checks:
//! 1. Coverage: every current manual-test step and public Clay JS API ID is
//!    referenced by exactly one ledger row; every protocol message family is
//!    covered; no stale references.
//! 2. Status: `verified` rows require named automated + manual evidence;
//!    `approved-removed` rows require a removal reference; rows are
//!    well-formed (phase, target owner/tests, known status).
//! 3. Native freeze: the frozen native client module set is unchanged unless
//!    Plan 097 records a migration exception.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn ledger() -> Value {
    serde_json::from_str(&read("docs/development/tauri-react-parity-ledger.json"))
        .expect("docs/development/tauri-react-parity-ledger.json must be valid JSON")
}

fn capabilities() -> Vec<Value> {
    ledger()["capabilities"]
        .as_array()
        .cloned()
        .expect("ledger capabilities array")
}

/// Manual test step IDs from a `test-plan` module: leading table cells that
/// look like `<LETTERS><digits>[letter]` (for example `E1`, `F12a`, `K83`).
fn manual_step_ids(module_file: &str) -> Vec<String> {
    let text = read(module_file);
    let mut ids = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if !line.starts_with('|') {
            continue;
        }
        let first = line
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim();
        let valid = {
            let mut chars = first.chars().peekable();
            let mut letters = 0;
            while chars.peek().is_some_and(|c| c.is_ascii_uppercase()) && letters < 2 {
                chars.next();
                letters += 1;
            }
            let mut digits = 0;
            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                chars.next();
                digits += 1;
            }
            if chars.peek().is_some_and(|c| c.is_ascii_lowercase()) {
                chars.next();
            }
            letters >= 1 && digits >= 1 && chars.peek().is_none()
        };
        if valid {
            ids.push(first.to_string());
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Public Clay JS API IDs (`registry_public = true`) from the inventory,
/// parsed tolerantly (the file intentionally stays hand-editable Markdown-
/// adjacent TOML and is parsed the same way by existing doc-coverage suites).
fn public_api_ids() -> Vec<String> {
    let text = read("docs/reference/clay-js-api/api-inventory.toml");
    let mut ids = Vec::new();
    for block in text.split("[[api]]").skip(1) {
        let block = block.split("[[api]]").next().unwrap_or(block);
        let mut id = None;
        let mut public = false;
        for line in block.lines() {
            let line = line.trim();
            if id.is_none() && line.starts_with("id = \"") {
                id = Some(
                    line.trim_start_matches("id = \"")
                        .trim_end_matches('"')
                        .to_string(),
                );
            }
            if line == "registry_public = true" {
                public = true;
            }
        }
        if let Some(id) = id.filter(|_| public) {
            ids.push(id);
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Enum variant names from a `src/protocol` source file.
fn enum_variants(source: &str, enum_name: &str) -> Vec<String> {
    let start = source
        .find(&format!("pub enum {enum_name} {{"))
        .unwrap_or_else(|| panic!("enum {enum_name} not found"));
    let body = &source[start..];
    let end = body.find("\n}\n").expect("enum closing brace");
    let mut variants = Vec::new();
    for line in body[..end].lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(char::is_uppercase) || !line.starts_with("    ") {
            continue;
        }
        let name: String = trimmed
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let rest = trimmed[name.len()..].trim_start();
        if rest.starts_with('{') || rest.starts_with('(') {
            variants.push(name);
        }
    }
    variants.sort();
    variants.dedup();
    variants
}

fn string_list(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|v| v.as_str().expect("string entry").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn parity_ledger_covers_every_manual_step_public_api_and_protocol_family() {
    let caps = capabilities();
    assert!(!caps.is_empty(), "ledger has no capability rows");

    // Manual steps: each referenced exactly once; no stale references.
    let modules: Vec<String> = fs::read_dir(root().join("test-plan"))
        .expect("test-plan dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.chars().next().is_some_and(|c| c.is_ascii_digit()) && n.ends_with(".md"))
        .map(|n| format!("test-plan/{n}"))
        .collect();
    assert!(modules.len() >= 14, "manual test-plan modules went missing");

    let mut seen_steps: BTreeMap<(String, String), String> = BTreeMap::new();
    for cap in &caps {
        let cid = cap["capability_id"].as_str().unwrap();
        for group in cap["manual_steps"].as_array().unwrap_or(&Vec::new()) {
            let module = group["module"].as_str().expect("module path").to_string();
            assert!(
                modules.contains(&module),
                "{cid}: unknown manual module {module}"
            );
            for step in string_list(group, "steps") {
                let key = (module.clone(), step.clone());
                let previous = seen_steps.insert(key.clone(), cid.to_string());
                assert!(
                    previous.is_none(),
                    "step {key:?} referenced twice ({:?} and {cid})",
                    previous
                );
                assert!(
                    manual_step_ids(&module).contains(&step),
                    "{cid}: stale step reference {step} in {module}"
                );
            }
        }
    }
    for module in &modules {
        let referenced: Vec<&String> = seen_steps
            .keys()
            .filter(|(m, _)| m == module)
            .map(|(_, s)| s)
            .collect();
        let expected = manual_step_ids(module);
        assert_eq!(
            referenced.len(),
            expected.len(),
            "{module}: ledger covers {} of {} steps; missing: {:?}",
            referenced.len(),
            expected.len(),
            expected
                .iter()
                .filter(|s| !referenced.contains(s))
                .collect::<Vec<_>>()
        );
    }

    // Public APIs: each referenced exactly once; no stale references.
    let inventory = public_api_ids();
    assert!(
        !inventory.is_empty(),
        "API inventory parse produced nothing"
    );
    let mut seen_apis: BTreeMap<String, String> = BTreeMap::new();
    for cap in &caps {
        let cid = cap["capability_id"].as_str().unwrap();
        for api in string_list(cap, "public_apis") {
            let previous = seen_apis.insert(api.clone(), cid.to_string());
            assert!(
                previous.is_none(),
                "API {api} referenced twice ({:?} and {cid})",
                previous
            );
            assert!(
                inventory.contains(&api),
                "{cid}: API {api} is not registry-public in the inventory"
            );
        }
    }
    let missing_apis: Vec<&String> = inventory
        .iter()
        .filter(|a| !seen_apis.contains_key(*a))
        .collect();
    assert!(
        missing_apis.is_empty(),
        "public APIs without a ledger row: {missing_apis:?}"
    );

    // Protocol families: every Client/Server/Agent variant covered at least once.
    let client = enum_variants(&read("src/protocol/mod.rs"), "ClientMessage");
    let server = enum_variants(&read("src/protocol/mod.rs"), "ServerMessage");
    let agent = enum_variants(&read("src/protocol/agent.rs"), "AgentServerMessage");
    let mut covered_client = Vec::new();
    let mut covered_server = Vec::new();
    for cap in &caps {
        let messages = &cap["protocol_messages"];
        covered_client.extend(string_list(messages, "client"));
        covered_server.extend(string_list(messages, "server"));
        for name in string_list(messages, "agent") {
            assert!(agent.contains(&name), "stale agent message {name}");
        }
    }
    for family in [
        ("client", &client, &covered_client),
        ("server", &server, &covered_server),
    ] {
        let missing: Vec<&String> = family.1.iter().filter(|v| !family.2.contains(v)).collect();
        assert!(
            missing.is_empty(),
            "{} message families without a ledger row: {missing:?}",
            family.0
        );
        let stale: Vec<&String> = family.2.iter().filter(|v| !family.1.contains(v)).collect();
        assert!(
            stale.is_empty(),
            "stale {} messages in ledger: {stale:?}",
            family.0
        );
    }
}

#[test]
fn verified_ledger_rows_require_named_evidence_and_rows_are_well_formed() {
    let statuses = ledger()["status_vocabulary"]
        .as_array()
        .expect("status vocabulary")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for cap in &capabilities() {
        let cid = cap["capability_id"]
            .as_str()
            .expect("capability_id")
            .to_string();
        let status = cap["status"].as_str().expect("status").to_string();
        assert!(statuses.contains(&status), "{cid}: unknown status {status}");
        let phase = cap["migration_phase"].as_u64().unwrap_or(0);
        assert!(
            (2..=12).contains(&phase),
            "{cid}: migration phase {phase} outside plan phases 2-12"
        );
        assert!(
            !cap["target_owner"].as_str().unwrap_or("").is_empty(),
            "{cid}: target_owner required"
        );
        assert!(
            !cap["target_tests"].as_str().unwrap_or("").is_empty(),
            "{cid}: target_tests required"
        );
        assert!(
            !cap["current_owner"]
                .as_array()
                .expect("current_owner array")
                .is_empty()
                || status == "approved-removed",
            "{cid}: current_owner required"
        );
        if status == "verified" {
            let auto = cap["verified_automated"].as_str().unwrap_or("");
            let manual = cap["verified_manual"].as_str().unwrap_or("");
            assert!(
                !auto.is_empty() && !manual.is_empty(),
                "{cid}: verified rows need named verified_automated AND verified_manual evidence"
            );
        }
        if status == "approved-removed" {
            assert!(
                !cap["removal_reference"].as_str().unwrap_or("").is_empty(),
                "{cid}: approved-removed rows need removal_reference"
            );
        }
    }
}

/// Removed Masonry/native-client modules must not return after the React cutover.
const REMOVED_NATIVE_MODULES: &[&str] = &[
    "src/app_driver.rs",
    "src/driver",
    "src/masonry_editor.rs",
    "src/masonry_package_region.rs",
    "src/masonry_pane_document.rs",
    "src/masonry_pane_host.rs",
    "src/masonry_sdui.rs",
    "src/masonry_sdui_region.rs",
    "src/masonry_shell",
    "src/masonry_welcome.rs",
    "src/editor/surface",
    "src/editor/accessibility.rs",
    "src/editor/buffer.rs",
    "src/editor/composition.rs",
    "src/editor/cursor.rs",
    "src/editor/document_session.rs",
    "src/editor/history.rs",
    "src/editor/layout.rs",
    "src/editor/selection.rs",
    "src/editor/snippet.rs",
    "src/editor/viewport.rs",
    "src/shell/primitives.rs",
    "vendor/masonry_core",
    "vendor/accesskit_atspi_common",
    "vendor/accesskit_unix",
];

/// Primitive names from the `docs/reference/primitives/registry.md` category
/// matrix (first table column).
fn registry_primitive_names() -> Vec<String> {
    let text = read("docs/reference/primitives/registry.md");
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if !line.starts_with("| ") {
            continue;
        }
        let first = line
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim();
        // Category rows start with a Capitalized identifier; header/separator
        // rows do not.
        if first.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && first != "Primitive"
        {
            names.push(first.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn primitive_migration_matrix_covers_ledger_and_registry_primitives() {
    let matrix = read("docs/development/tauri-react-primitive-migration.md");
    let registry = registry_primitive_names();
    assert!(
        registry.len() >= 30,
        "registry parse produced only {registry:?}"
    );
    for cap in capabilities() {
        let cid = cap["capability_id"].as_str().expect("capability_id");
        assert!(
            matrix.contains(cid),
            "{cid}: missing from docs/development/tauri-react-primitive-migration.md"
        );
        for primitive in string_list(&cap, "primitives") {
            assert!(
                registry.contains(&primitive),
                "{cid}: primitive {primitive} is not a docs/reference/primitives/registry.md \
                 category; inventing primitives requires its own review/docs/tests"
            );
        }
    }
}

/// DTO deny list for Plan 097 Phase 3+: frontend-facing bridge sources must
/// not expose filesystem/process handles, V8 values, raw ops, or archived
/// bytes. Scans activate automatically once the directories exist.
#[test]
fn frontend_bridge_sources_stay_free_of_forbidden_authority_markers() {
    const FORBIDDEN: &[(&str, &str)] = &[
        ("Deno.core", "V8/op access must stay server-side"),
        ("op_clay_", "raw op names are not frontend API"),
        ("rkyv", "archived-byte access is Rust-only"),
        ("Archived", "archived references must not cross IPC"),
        (
            "std::process::Command in serde",
            "process handles are not DTO data",
        ),
        ("tokio::fs", "frontend DTOs carry no filesystem I/O"),
    ];
    let mut scanned = Vec::new();
    for dir in ["src-tauri/src", "frontend/src"] {
        let dir = root().join(dir);
        if !dir.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        collect_sources(&dir, &mut files);
        // Generated type declarations are excluded: ts-rs writes them from the
        // Rust DTO layer (plan 119 SC-1, `scripts/check-bindings.sh`) and they
        // quote Rust doc comments, so markers like `rkyv` appear as
        // documentation text with no frontend authority behind them. Every
        // hand-written bridge source is still scanned.
        files.retain(|file| !file.to_string_lossy().contains("bridge/generated"));
        scanned.extend(files);
    }
    if scanned.is_empty() {
        // Phase 2/3 have not created the workspace yet; the deny boundary is
        // still pinned by the migration matrix document.
        assert!(
            read("docs/development/tauri-react-primitive-migration.md").contains("DTO deny list"),
            "migration matrix must pin the DTO deny list before bridge code exists"
        );
        return;
    }
    for file in &scanned {
        let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {file:?}: {e}"));
        for (marker, reason) in FORBIDDEN {
            assert!(
                !text.contains(marker),
                "{file:?} contains forbidden marker `{marker}` ({reason})"
            );
        }
    }
}

/// Plan 097: configuration is server-canonical. The frontend must not
/// introduce browser storage (localStorage/sessionStorage/indexedDB/cookies)
/// that could grant authority or override canonical configuration; UI-session
/// state persists only through typed `settings.*` command intents into the
/// server-side `preferences.json` store (which is validated, bounded, and
/// atomic).
#[test]
fn frontend_has_no_browser_storage_authority() {
    const FORBIDDEN: &[(&str, &str)] = &[
        (
            "localStorage",
            "browser storage would split configuration authority; persist via settings.* intents",
        ),
        (
            "sessionStorage",
            "browser storage would split configuration authority; persist via settings.* intents",
        ),
        (
            "indexedDB",
            "browser storage would split configuration authority; it is not a Clay configuration surface",
        ),
        (
            "document.cookie",
            "cookies are not a Clay configuration surface",
        ),
    ];
    let mut scanned = Vec::new();
    for dir in ["src-tauri/src", "frontend/src"] {
        let dir = root().join(dir);
        if !dir.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        collect_sources(&dir, &mut files);
        scanned.extend(files);
    }
    assert!(
        !scanned.is_empty(),
        "frontend/tauri sources must exist before the storage boundary can be pinned"
    );
    for file in &scanned {
        let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {file:?}: {e}"));
        for (marker, reason) in FORBIDDEN {
            assert!(
                !text.contains(marker),
                "{file:?} contains forbidden marker `{marker}` ({reason})"
            );
        }
    }
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e == "rs" || e == "ts" || e == "tsx")
        {
            out.push(path);
        }
    }
}

/// Package-facing `ComponentKind` names from the clay-execution component catalog
/// (`## Package-Facing Component Kinds` table).
fn catalog_component_kinds() -> Vec<String> {
    let text = read(".agents/skills/clay-execution/references/components.md");
    let section = text
        .split("## Package-Facing Component Kinds")
        .nth(1)
        .expect("component kinds section")
        .split("\n## ")
        .next()
        .expect("kinds table block");
    let mut kinds = Vec::new();
    for line in section.lines() {
        if !line.starts_with("| `") {
            continue;
        }
        let kind = line[3..].split('`').next().expect("kind cell").to_string();
        kinds.push(kind);
    }
    kinds.sort();
    kinds.dedup();
    kinds
}

#[test]
fn react_catalog_maps_every_component_kind() {
    let mapping = read("docs/development/react-ui-catalog-mapping.md");
    for kind in catalog_component_kinds() {
        let needle = format!("| `{kind}` |");
        let rows: Vec<&str> = mapping.lines().filter(|l| l.starts_with(&needle)).collect();
        assert_eq!(
            rows.len(),
            1,
            "{kind}: expected exactly one target-renderer row in \
             docs/development/react-ui-catalog-mapping.md, found {}",
            rows.len()
        );
        let cells: Vec<&str> = rows[0].split('|').collect();
        // | Kind | Target renderer | Accessibility contract | Notes |
        assert!(cells.len() >= 6, "{kind}: mapping row needs 4 columns");
        for cell in [cells[2], cells[3]] {
            assert!(
                cell.trim().len() > 3,
                "{kind}: renderer/accessibility cells must be filled"
            );
        }
    }
}

#[test]
fn core_tokens_project_to_css_variables_or_internal_codemirror_values() {
    let mapping = read("docs/development/react-ui-catalog-mapping.md");
    for token in core_theme_token_names() {
        let css_var = format!("--clay-{}", token.replace('.', "-"));
        assert!(
            mapping.contains(&css_var),
            "{token}: missing `{css_var}` projection in \
             docs/development/react-ui-catalog-mapping.md"
        );
    }
    for key in [
        "shellBg",
        "bracketMatch",
        "gutterFgActive",
        "lineHighlight",
        "indentGuide",
        "searchMatch",
    ] {
        assert!(
            mapping.contains(key),
            "editor StyleRegistry key {key} must be listed as internal CodeMirror value"
        );
    }
    assert!(
        mapping.contains("internal: CodeMirror"),
        "mapping must mark editor StyleRegistry values as internal: CodeMirror"
    );
}

/// Core token names from the `tokens.md` implemented-token tables (rows whose
/// first cell is a dotted lowercase token).
fn core_theme_token_names() -> Vec<String> {
    let text = read(".agents/skills/clay-execution/references/tokens.md");
    let mut names = Vec::new();
    for line in text.lines() {
        if !line.starts_with("| `") {
            continue;
        }
        let first = &line[3..];
        let name: String = first.chars().take_while(|c| *c != '`').collect();
        let mut parts = name.split('.');
        let is_token = matches!(parts.next(), Some(domain)
            if domain.chars().all(|c| c.is_ascii_lowercase()) && !domain.is_empty())
            && parts.all(|p| {
                p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
                    && !p.is_empty()
            });
        if is_token && !names.contains(&name) {
            names.push(name);
        }
    }
    names.sort();
    names
}

#[test]
fn removed_native_client_modules_cannot_return() {
    let present: Vec<_> = REMOVED_NATIVE_MODULES
        .iter()
        .filter(|path| root().join(path).exists())
        .collect();
    assert!(
        present.is_empty(),
        "removed native client modules returned: {present:?}"
    );

    let dependency_files = format!(
        "{}\n{}\n{}",
        read("Cargo.toml"),
        read("src-tauri/Cargo.toml"),
        read("Cargo.lock")
    );
    for removed in [
        "name = \"masonry",
        "name = \"vello\"",
        "name = \"parley\"",
        "name = \"winit\"",
        "name = \"accesskit",
        "vendor/masonry_core",
        "vendor/accesskit_atspi_common",
        "vendor/accesskit_unix",
    ] {
        assert!(
            !dependency_files.contains(removed),
            "removed native dependency or patch returned: {removed}"
        );
    }
}

/// Current-state documentation must not claim the removed native client
/// architecture. Historical content is allowed only in explicitly classified
/// files, and each excluded file must carry its historical banner so the
/// exclusion cannot silently widen.
#[test]
fn current_state_docs_reject_removed_native_architecture_terms() {
    // (path, required banner proving the file is classified as historical)
    let historical: &[(&str, &str)] = &[
        (
            "docs/development/accessibility.md",
            "the native Masonry contract below is historical context",
        ),
        (
            "docs/development/security.md",
            "were retired when the native client dependency chains",
        ),
        (
            "docs/development/performance.md",
            "historical native-client record",
        ),
        (
            "docs/development/tauri-react-primitive-migration.md",
            "Primitive Migration Matrix",
        ),
        (
            "docs/development/tauri-react-parity-ledger.md",
            "records the former native Masonry baseline",
        ),
    ];
    for (path, banner) in historical {
        let doc = read(path);
        assert!(
            doc.contains(banner),
            "{path} is excluded from the stale-architecture scan but its \
             historical banner is missing or was reworded: {banner}"
        );
    }

    let forbidden: &[&str] = &[
        "masonry",
        "vello",
        "parley",
        "winit",
        "accesskit",
        "ClayShellWidget",
        "EditorWidget",
        "PaneDocumentView",
        "PackageOverlayHost",
        "EditorSurface",
    ];
    let mut current: Vec<&str> = vec![
        "README.md",
        "docs/index.md",
        "docs/development/architecture-ownership.md",
        "docs/development/build-and-test.md",
        "docs/development/react-ui-catalog-mapping.md",
        "docs/development/ui-observability.md",
        "docs/development/windows.md",
        "docs/development/file-open-save-reload-workflow.md",
        "docs/development/manual-editor-capabilities-test-plan.md",
        "docs/development/manual-file-browser-workflow-bug-contract.md",
        "docs/development/launch-and-gui-smoke.md",
        "docs/reference/ui-components.md",
        "docs/reference/packages/creating-packages.md",
    ];
    for entry in std::fs::read_dir("docs/reference/primitives").expect("primitives dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            current.push(Box::leak(
                path.to_string_lossy().into_owned().into_boxed_str(),
            ));
        }
    }

    for path in &current {
        let doc = read(path);
        let lower = doc.to_lowercase();
        for term in forbidden {
            assert!(
                !lower.contains(&term.to_lowercase()),
                "{path} mentions removed native-client architecture term \
                 `{term}`; rewrite it to the Tauri/React equivalent or move \
                 the content into an explicitly classified historical file"
            );
        }
    }
}

#[test]
fn plan099_reference_docs_are_cross_linked_and_current() {
    let docs_index = read("docs/index.md");
    for link in [
        "[Clay Primitives Reference](reference/primitives/index.md)",
        "[Primitive Registry Schema](reference/primitives/registry.md)",
        "[Incremental Parse and Background Parse Update Strategy](reference/primitives/parse-update-strategy.md)",
        "[Creating Clay Packages](reference/packages/creating-packages.md)",
        "[Performance Fixtures and Baseline Workflow](development/performance.md)",
    ] {
        assert!(
            docs_index.contains(link),
            "docs/index.md must retain current Plan 099 reference link {link:?}"
        );
    }

    let primitive_index = read("docs/reference/primitives/index.md");
    for link in [
        "registry.md",
        "parse-update-strategy.md",
        "rendering-strategy.md",
        "../../development/performance.md",
    ] {
        assert!(
            primitive_index.contains(link),
            "primitive index must link current Plan 099 reference {link:?}"
        );
    }

    let package_guide = read("docs/reference/packages/creating-packages.md");
    for marker in [
        "parse.serverRegisterParseHandler",
        "syntax.serverRegisterSyntaxGrammar",
        "decorations.serverPublishDecorations",
        "diagnostics.serverPublishDiagnostics",
        "folding.serverPublishFoldingRanges",
        "no package-facing",
        "synchronous IPC",
    ] {
        assert!(
            package_guide.contains(marker),
            "creating-packages.md must preserve Plan 099 package boundary marker {marker:?}"
        );
    }

    for (path, stale) in [
        (
            "docs/reference/primitives/parse-update-strategy.md",
            "A future `src/server/parse_coordinator.rs`",
        ),
        (
            "docs/reference/primitives/parse-update-strategy.md",
            "no-decoration-update",
        ),
        (
            "docs/reference/primitives/rendering-strategy.md",
            "Proposed documentation-only shape",
        ),
        (
            "docs/development/performance.md",
            "memoized per-document line",
        ),
    ] {
        assert!(
            !read(path).contains(stale),
            "{path} retains stale current-state documentation {stale:?}"
        );
    }
}

/// Wiki navigation contract: every evergreen wiki page (everything outside
/// `docs/wiki/archive/`) is linked from the master index, every intra-wiki
/// link resolves (archive pages included), and current-state wiki pages name
/// only source/test paths that exist. Archive pages are pull-only history:
/// they are not indexed, but their links must still resolve.
#[test]
fn wiki_navigation_is_complete_and_current_page_paths_resolve() {
    fn walk(dir: &str, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect(dir) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(path.to_str().expect("utf8"), out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }

    let mut pages = Vec::new();
    walk("docs/wiki", &mut pages);
    assert!(pages.len() > 100, "wiki unexpectedly shrank");
    let index = read("docs/wiki/index.md");

    for page in &pages {
        if page == "docs/wiki/index.md" || page.starts_with("docs/wiki/archive/") {
            continue;
        }
        let name = page.rsplit('/').next().expect("filename");
        let linked = index.split('(').any(|chunk| {
            let target = chunk.split(')').next().unwrap_or("");
            target == name || target.ends_with(&format!("/{name}"))
        });
        assert!(linked, "{page} is not linked from docs/wiki/index.md");
    }

    for page in &pages {
        let doc = read(page);
        let base = std::path::Path::new(page).parent().expect("parent");
        for caps in doc.match_indices("](") {
            let rest = &doc[caps.0 + 2..];
            let end = match rest.find(')') {
                Some(end) => end,
                None => continue,
            };
            let target = &rest[..end];
            if target.starts_with("http") || !target.ends_with(".md") || target.is_empty() {
                continue;
            }
            let resolved = base.join(target.split('#').next().unwrap_or(target));
            assert!(
                resolved.exists(),
                "{page} links to missing wiki document `{target}`"
            );
        }
    }

    // Current-state pages must only name existing source/test files.
    let current_pages = [
        "docs/wiki/modules/tauri-desktop-shell.md",
        "docs/wiki/modules/react-client-bridge.md",
        "docs/wiki/modules/frontend-theme-runtime.md",
        "docs/wiki/flows/frontend-edit-synchronization.md",
        "docs/wiki/flows/document-chunked-loading.md",
        "docs/wiki/flows/editor-viewport-render-patch.md",
        "docs/wiki/flows/ag-ui-tauri-stream.md",
    ];
    for page in current_pages {
        let doc = read(page);
        for span in doc.split('`').skip(1).step_by(2) {
            let candidate = span.trim();
            let looks_like_path = candidate.starts_with("src/")
                || candidate.starts_with("frontend/src/")
                || candidate.starts_with("src-tauri/");
            let has_code_ext = [".rs", ".ts", ".tsx"]
                .iter()
                .any(|ext| candidate.ends_with(ext));
            if !(looks_like_path && has_code_ext) {
                continue;
            }
            // Strip trailing anchors like `foo.rs tests`.
            let file = candidate.split([' ', ':']).next().unwrap_or(candidate);
            assert!(
                std::path::Path::new(file).exists(),
                "{page} names nonexistent source path `{candidate}`"
            );
        }
    }
}

/// Plan 125 wiki contract: the pages that described the retired centered
/// projection and the picker dialogs must describe the one composer palette
/// instead (protocol v32 `mode` stages, the shielded credential stage, the halo,
/// default-agent adoption), and no evergreen page may still describe a centered
/// transient surface as live. The removal record itself is the one page allowed
/// to speak about the centered implementation at length.
#[test]
fn plan125_wiki_pages_describe_the_one_palette_surface() {
    let pages = [
        "docs/wiki/modules/control-center.md",
        "docs/wiki/modules/centered-command-centre-surface.md",
        "docs/wiki/modules/transient-menu-session.md",
        "docs/wiki/modules/transient-menu-round-trip.md",
        "docs/wiki/modules/react-command-centre-desktop-workflows.md",
        "docs/wiki/modules/react-shell.md",
        "docs/wiki/modules/slot-aware-package-ui.md",
        "docs/wiki/modules/path-browser.md",
        "docs/wiki/modules/command-registry.md",
        "docs/wiki/modules/clay-agent.md",
        "docs/wiki/modules/react-sdui-package-ui.md",
        "docs/wiki/modules/ui-review-harness.md",
        "docs/wiki/modules/react-tabs-and-splits.md",
        "docs/wiki/modules/react-agui-chat-stream.md",
        "docs/wiki/index.md",
    ];
    // Markers that carry the plan-125 story: the single sheet, its modes, the
    // shield, the halo, the retired origin, and default-agent adoption.
    let markers: [(&str, &str); 8] = [
        (
            "docs/wiki/modules/control-center.md",
            "one session, six modes",
        ),
        (
            "docs/wiki/modules/centered-command-centre-surface.md",
            "removal record",
        ),
        (
            "docs/wiki/modules/transient-menu-session.md",
            "second, centered renderer",
        ),
        (
            "docs/wiki/modules/react-command-centre-desktop-workflows.md",
            "shielded",
        ),
        ("docs/wiki/modules/react-shell.md", "halo"),
        (
            "docs/wiki/modules/slot-aware-package-ui.md",
            "PackageOverlayAnchor::Centered",
        ),
        (
            "docs/wiki/modules/clay-agent.md",
            "adopts the server's default type",
        ),
        (
            "docs/wiki/modules/react-tabs-and-splits.md",
            "tabUncommitted",
        ),
    ];
    for (page, marker) in markers {
        assert!(
            read(page).to_lowercase().contains(&marker.to_lowercase()),
            "{page} is missing the Plan 125 marker {marker:?}"
        );
    }
    for page in pages {
        let doc = read(page);
        assert!(
            doc.contains("plan 125") || doc.contains("Plan 125"),
            "{page} must name the plan that retired the centered surface"
        );
    }

    // No evergreen page may still describe a live centered renderer. These are
    // the exact plan-124-era claims plan 125 falsified; the removal record
    // (centered-command-centre-surface.md) is excluded because it explains them.
    let stale = [
        "Centered `CommandCentre` remains",
        "Centered `CommandCentre` rendering is retained",
        "Centered `CommandCentre` rendering survives",
        "Centered picker/package sessions",
        "Non-palette origins continue through",
    ];
    let mut dirs = vec![
        PathBuf::from("docs/wiki/modules"),
        PathBuf::from("docs/wiki/flows"),
    ];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(root().join(&dir)).expect("wiki dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                dirs.push(path.strip_prefix(root()).expect("relative").to_path_buf());
                continue;
            }
            let relative = path
                .strip_prefix(root())
                .expect("relative")
                .to_string_lossy()
                .into_owned();
            if relative.ends_with("centered-command-centre-surface.md") {
                continue;
            }
            let doc = read(&relative);
            for claim in stale {
                assert!(
                    !doc.contains(claim),
                    "{relative} still claims {claim:?}; plan 125 deleted the centered renderer"
                );
            }
        }
    }
}

/// Plan 127 wiki contract: lane scheduling, the bounded mailbox, heap-limit
/// restoration, and the process-wide trust-domain constraint (review A3) must
/// stay documented on the persistent-runtime pages, and no evergreen wiki page
/// may still state the pre-plan-127 single-worker-per-domain topology. The
/// archive pages are excluded: they are dated records of the topology they
/// describe.
#[test]
fn plan127_wiki_pages_describe_lane_scheduling_and_the_trust_domain_constraint() {
    let markers: [(&str, &str); 11] = [
        (
            "docs/wiki/modules/persistent-runtime-hardening.md",
            "RuntimeLane",
        ),
        (
            "docs/wiki/modules/persistent-runtime-hardening.md",
            "JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY",
        ),
        (
            "docs/wiki/modules/persistent-runtime-hardening.md",
            "remove_near_heap_limit_callback",
        ),
        (
            "docs/wiki/modules/persistent-runtime-hardening.md",
            "not a trust boundary",
        ),
        (
            "docs/wiki/modules/persistent-runtime-hardening.md",
            "plan 127",
        ),
        ("docs/wiki/modules/embedded-js-runtime.md", "latency lane"),
        ("docs/wiki/modules/embedded-js-runtime.md", "plan 127"),
        ("docs/wiki/index.md", "persistent-runtime-hardening.md"),
        (
            "docs/wiki/modules/parse-coordinator.md",
            "general runtime lane",
        ),
        (
            "docs/wiki/modules/completion-snippet-expansion.md",
            "persistent-runtime-hardening.md",
        ),
        (
            "docs/wiki/modules/language-intelligence.md",
            "persistent-runtime-hardening.md",
        ),
    ];
    for (page, marker) in markers {
        assert!(
            read(page).to_lowercase().contains(&marker.to_lowercase()),
            "{page} is missing the Plan 127 marker {marker:?}"
        );
    }

    // The topology plan 127 falsified: one persistent worker per domain, two
    // total. Both the count and the "TWO dedicated worker threads" claim are
    // now wrong (two lanes per domain, four workers per generation) and must
    // not reappear on an evergreen page.
    let stale = [
        "exactly two persistent `JsRuntime`",
        "starts TWO dedicated worker threads",
        "exactly-two-persistent-runtimes topology",
        "the persistent runtime worker and its registered timeout",
    ];
    let mut dirs = vec![
        PathBuf::from("docs/wiki/modules"),
        PathBuf::from("docs/wiki/flows"),
    ];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(root().join(&dir)).expect("wiki dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                dirs.push(path.strip_prefix(root()).expect("relative").to_path_buf());
                continue;
            }
            let relative = path
                .strip_prefix(root())
                .expect("relative")
                .to_string_lossy()
                .into_owned();
            let doc = read(&relative);
            for claim in stale {
                assert!(
                    !doc.contains(claim),
                    "{relative} still claims {claim:?}; plan 127 gave each domain a general and a latency lane"
                );
            }
        }
    }
}

/// Plan 129 wiki contract: the connection page must map every arm family to
/// its handler entry point, describe the dispatch context and the
/// handler-entry authorization boundary, and record the future-size work that
/// came with the extraction, so the page stays usable as the answer to “where
/// is this `ClientMessage` handled?”.
#[test]
fn plan129_wiki_pages_describe_the_dispatch_router_and_future_sizes() {
    let markers: [(&str, &str); 12] = [
        ("docs/wiki/modules/server-ipc-skeleton.md", "Plan 129"),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "handle_connection_loop",
        ),
        ("docs/wiki/modules/server-ipc-skeleton.md", "ConnectionCtx"),
        ("docs/wiki/modules/server-ipc-skeleton.md", "ctx!"),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "documents::handle_save_document",
        ),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "workspace::handle_open_selected_file",
        ),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "tabs::handle_tab_command",
        ),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "menus::handle_menu_activate",
        ),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "runtime::handle_agent_command",
        ),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "Authorization checks live at handler entry points",
        ),
        ("docs/wiki/modules/server-ipc-skeleton.md", "Box::pin"),
        (
            "docs/wiki/modules/server-ipc-skeleton.md",
            "future-size-threshold",
        ),
    ];
    for (page, marker) in markers {
        assert!(
            read(page).to_lowercase().contains(&marker.to_lowercase()),
            "{page} is missing the Plan 129 marker {marker:?}"
        );
    }
    assert!(
        read("docs/wiki/index.md").contains("server-ipc-skeleton.md"),
        "docs/wiki/index.md must link the connection page"
    );
}

/// Plan 130 wiki contract: the daemon page must map the `clay-agent/src/host/*`
/// concern modules behind the class facade, and the server page must describe
/// injected ownership — the lane's op state holds the handle, a hostless lane
/// fails closed, and registration declarations queue per lane before handing
/// over to that lane's host. These are the two pages a developer reads to
/// answer "who owns the agent host, and how does a runtime lane reach it?".
#[test]
fn plan130_wiki_pages_describe_host_modules_and_injected_ownership() {
    let markers: [(&str, &str); 13] = [
        ("docs/wiki/modules/clay-agent.md", "Plan 130"),
        ("docs/wiki/modules/clay-agent.md", "clay-agent/src/host/"),
        ("docs/wiki/modules/clay-agent.md", "host/internals.ts"),
        ("docs/wiki/modules/clay-agent.md", "ClayAgentHost"),
        ("docs/wiki/modules/clay-agent.md", "thin delegate"),
        ("docs/wiki/modules/clay-agent.md", "type-only"),
        ("docs/wiki/modules/clay-agent.md", "process-global"),
        ("docs/wiki/modules/agent-process-manager.md", "Plan 130 A1"),
        (
            "docs/wiki/modules/agent-process-manager.md",
            "AgentHostHandle",
        ),
        (
            "docs/wiki/modules/agent-process-manager.md",
            "set_agent_host",
        ),
        (
            "docs/wiki/modules/agent-process-manager.md",
            "queue_registration_now",
        ),
        (
            "docs/wiki/modules/agent-process-manager.md",
            "agent.unavailable: no agent host is attached",
        ),
        (
            "docs/wiki/modules/agent-process-manager.md",
            "PENDING_REGISTRATION_CAP",
        ),
    ];
    for (page, marker) in markers {
        assert!(
            read(page).to_lowercase().contains(&marker.to_lowercase()),
            "{page} is missing the Plan 130 marker {marker:?}"
        );
    }
    let index = read("docs/wiki/index.md");
    for page in ["modules/clay-agent.md", "modules/agent-process-manager.md"] {
        assert!(index.contains(page), "docs/wiki/index.md must link {page}");
    }
}
