use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn package_author_docs_pin_semantic_role_and_concrete_font_prohibition() {
    let guide = read("docs/reference/packages/creating-packages.md");
    let primitive = read("docs/reference/primitives/typography.md");

    for marker in [
        "defaultFontRole",
        "Range `fontRole` accepts `monospace` or `proportional`",
        "Component text separately defaults to `ui`",
        "fontFamily",
        "fontSize",
        "font paths/bytes/URLs/downloads",
        "outside paint/input/layout hot paths",
    ] {
        assert!(
            guide.contains(marker),
            "package guide must document {marker}"
        );
    }
    for (path, role) in [
        ("docs/reference/packages/markdown.md", "proportional"),
        ("docs/reference/packages/rust.md", "monospace"),
        ("docs/reference/packages/typescript.md", "monospace"),
        ("docs/reference/packages/javascript.md", "monospace"),
    ] {
        let package = read(path);
        assert!(package.contains(&format!("defaultFontRole: \"{role}\"")));
        assert!(package.contains("Semantic Typography Roles"));
    }
    assert!(primitive.contains("no language-name branches"));
    assert!(primitive.contains("raw `Deno.core.ops`"));
}

#[test]
fn package_loading_doc_linked_from_indexes_and_marks_phase17_ready() {
    let docs_index = read("docs/index.md");
    let primitives_index = read("docs/reference/primitives/index.md");
    let backlog = read("docs/reference/primitives/backlog.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");

    assert!(
        docs_index.contains("reference/primitives/package-loading.md"),
        "docs/index.md must link the Phase 17 package loading reference"
    );
    assert!(
        primitives_index.contains("package-loading.md"),
        "primitives index must link package-loading.md"
    );
    for checklist in [
        "Package manifest validation supports package identity",
        "Package enable/load rejects invalid prefixes",
        "DocumentClassification",
        "MajorModeActivation",
        "CommandDeclaration",
        "Phase 17 explicitly hands off `DecorationRange` and `IncrementalParseUpdate`",
    ] {
        assert!(
            backlog.contains(&format!("- [x] {checklist}")) || backlog.contains(checklist),
            "Phase 17 backlog checklist must mark/readiness-cover {checklist}"
        );
    }
    assert!(package_loading.contains("Install, Enable, and Runtime Boundary"));
    assert!(package_loading.contains("Conflict Handling"));
    assert!(package_loading.contains("Phase 18 Handoff"));
}

#[test]
fn package_loading_keeps_validation_and_parsing_out_of_typing_hot_path() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_service = read("src/packages/service.rs");
    let parse_coordinator = read("src/server/parse_coordinator.rs");

    for phrase in ["typing", "paint", "layout", "scroll", "text-event"] {
        assert!(
            package_loading.contains(phrase),
            "package loading reference must document {phrase} hot-path exclusion"
        );
    }
    assert!(
        package_loading.contains("outside typing, paint, layout, scroll, and text-event handlers"),
        "package loading reference must keep validation/loading outside editor hot paths"
    );
    assert!(
        package_service.contains("none of these operations may be called from typing"),
        "package service source comment should preserve enable/install hot-path policy"
    );
    assert!(
        parse_coordinator.contains("does not wait for parse completion"),
        "parse coordinator must keep parsing off edit acknowledgement/typing paths"
    );
}

#[test]
fn unified_package_authority_model_is_documented() {
    let authority = read("docs/wiki/modules/third-party-runtime-authority.md");
    let security = read("docs/reference/primitives/package-security.md");
    let loading = read("docs/reference/primitives/package-loading.md");
    let permissions = read("src/packages/permissions.rs");
    let decision =
        read("decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md");
    let package_ops = read("src/server/ops/packages.rs");

    for phrase in [
        "one package authority model",
        "Package source (`@clay/*`, npm, GitHub, git URL, tarball, or local path) affects default trust prompts",
        "install != enable != load != runtime execution != package-manager execution != client behavior delivery",
        "Plan 035 replaces this limitation with a source-aware resolver",
        "package-control",
        "package-import",
        "filesystem",
        "network",
        "shell",
        "workspace-mutation",
        "dependsOn",
        "extends",
        "disables",
        "replaces",
        "native-trust | sandboxed | restricted",
    ] {
        assert!(
            authority.contains(phrase),
            "unified authority wiki must document `{phrase}`"
        );
    }

    for phrase in [
        "Unified Package Trust and Authorization Policy",
        "user/admin authorization is the grant",
        "package_authority",
        "runtime_profile = \"native-trust\"",
        "capabilities = [\"mode-registration\", \"package-control\", \"network\"]",
        "Unified Package Capability Model",
        "PackageAuthorizationRecord",
        "PackageService::authorize_package",
        "enable` fails closed",
        "clay.capabilities",
        "clay.permissions` compatibility path",
        "These capabilities are not categorically unavailable to user-installed packages",
        "Powerful Capabilities Require Explicit Grants",
        "not categorically unavailable to user-installed packages",
    ] {
        assert!(
            security.contains(phrase),
            "package security docs must document `{phrase}`"
        );
    }

    for phrase in [
        "Package Sources and Provenance",
        "clay package add github:user/repo",
        "clay package add ./local-package",
        "PackageSourceKind",
        "PackageProvenance",
        "bounded sanitized diagnostics",
        "Unified Disable, Update, Rollback, and Incident Policy",
        "Package control is user-authorized",
        "decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md",
    ] {
        assert!(
            loading.contains(phrase),
            "package loading docs must document `{phrase}`"
        );
    }

    for phrase in [
        "PackageControl",
        "PackageImport",
        "Filesystem",
        "Network",
        "Shell",
        "WorkspaceMutation",
        "NativeUi",
        "ClientRuntime",
        "RawOps",
        "\"filesystem\" => Ok(PackagePermission::Filesystem)",
    ] {
        assert!(
            permissions.contains(phrase),
            "permissions parser must accept target capability `{phrase}`"
        );
    }

    assert!(
        decision.contains("status: approved")
            && decision.contains("explicitly_approved_by_user: true")
            && decision.contains("Clay will use one package authority model"),
        "approved decision log must record unified package authority"
    );
    assert!(
        package_ops.contains("installed, authorized package specifier")
            && package_ops.contains("PackageService")
            && package_ops.contains("PackageLoadEntryAllowlist"),
        "resolver source comments must describe source-aware package loading through PackageService"
    );
}

#[test]
fn package_scoped_disable_rollback_and_revocation_are_documented() {
    let loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let hot_reload = read("docs/wiki/modules/persistent-runtime-hot-reload.md");
    let parse_lifecycle = read("docs/wiki/modules/parse-task-lifecycle.md");
    let review = read("docs/wiki/modules/unified-package-authority-primitive-review.md");
    let service = read("src/packages/service.rs");
    let parse = read("src/server/parse_coordinator.rs");
    let packages_op = read("src/server/ops/packages.rs");
    let js_runtime = read("src/server/js_runtime.rs");
    let package_tests = read("tests/package_loading.rs");
    let parse_tests = read("tests/parse_coordinator.rs");

    for phrase in [
        "PackageRevocationRecord",
        "PackageContributionWithdrawalCounts",
        "commands, behavior manifests, SDUI, parse handlers, decorations, completions, layout, input, state, theme, and diagnostics",
        "rollback keeps the previous valid generation active",
        "ParseCoordinator::cancel_package",
        "PackageLoadEntryAllowlist::revoke_package",
        "never from keypress, paint, layout, scroll, text-event, edit-ack, pointer, or Masonry hot paths",
    ] {
        assert!(
            loading.contains(phrase)
                || wiki.contains(phrase)
                || hot_reload.contains(phrase)
                || parse_lifecycle.contains(phrase)
                || review.contains(phrase),
            "revocation docs must document `{phrase}`"
        );
    }

    for phrase in [
        "PackageRevocationRecord",
        "PackageContributionWithdrawalCounts",
        "revocation_record",
        "revocation_records",
        "revoke_enabled_package",
        "record_revocation_for_record",
        "previous_revocations",
        "previous_package_generation",
    ] {
        assert!(
            service.contains(phrase),
            "service must implement `{phrase}`"
        );
    }

    for phrase in ["cancel_package", "abort_tasks"] {
        assert!(
            parse.contains(phrase),
            "parse coordinator must implement `{phrase}`"
        );
    }
    for phrase in [
        "package_name: Option<String>",
        "record_for_package",
        "revoke_package",
    ] {
        assert!(
            packages_op.contains(phrase),
            "package loadEntry allowlist must implement `{phrase}`"
        );
    }
    assert!(
        js_runtime.contains("package_load_entry_allowlist_revokes_owned_entries"),
        "runtime tests must cover package-owned allowlist revocation"
    );

    for phrase in [
        "package_service_disable_removes_active_contributions",
        "failed_replacement_keeps_previous_generation_active",
    ] {
        assert!(
            package_tests.contains(phrase),
            "package loading test `{phrase}` must exist"
        );
    }
    assert!(
        parse_tests.contains("package_cancel_withdraws_handlers_and_in_flight_parse_work"),
        "parse coordinator must test package-scoped cancellation"
    );
}

#[test]
fn explicit_conflict_resolution_policy_is_documented() {
    let security = read("docs/reference/primitives/package-security.md");
    let loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let review = read("docs/wiki/modules/unified-package-authority-primitive-review.md");
    let conflict = read("src/packages/conflict.rs");
    let service = read("src/packages/service.rs");
    let tests = read("tests/package_conflicts.rs");

    for phrase in [
        "PackageConflictResolutionPolicy",
        "PackageConflictResolutionDiagnostic",
        "UserOverride",
        "PackageReplaces",
        "PackageDisables",
        "deterministic diagnostic fallback",
        "no package wins by load order alone",
    ] {
        assert!(
            security.contains(phrase)
                || loading.contains(phrase)
                || wiki.contains(phrase)
                || review.contains(phrase),
            "conflict docs must document `{phrase}`"
        );
    }

    for phrase in [
        "check_enabled_packages_with_policy",
        "PackageConflictResolutionPolicy",
        "PackageConflictResolutionDiagnostic",
        "PackageConflictResolutionReason",
        "user_override_winner",
    ] {
        assert!(
            conflict.contains(phrase),
            "conflict module must implement `{phrase}`"
        );
    }

    for phrase in [
        "set_conflict_override",
        "conflict_resolution_diagnostics",
        "reconcile_enabled_conflicts",
        "record_package_control_resolution",
    ] {
        assert!(
            service.contains(phrase),
            "package service must apply conflict resolution phrase `{phrase}`"
        );
    }

    for phrase in [
        "duplicate_mode_is_rejected_without_resolution_policy",
        "replacement_wins_with_package_control_grant_and_records_resolution",
        "user_conflict_override_selects_winner_without_package_control",
        "explicit_keybinding_priority_prevents_ambiguous_conflict",
        "same_keybinding_priority_falls_back_to_deterministic_diagnostic",
        "package_cannot_replace_without_package_control_grant",
    ] {
        assert!(
            tests.contains(phrase),
            "conflict behavior test `{phrase}` must exist"
        );
    }
}

#[test]
fn package_graph_relations_and_package_control_are_documented() {
    let security = read("docs/reference/primitives/package-security.md");
    let loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let manifest = read("src/packages/manifest.rs");
    let graph = read("src/packages/graph.rs");
    let service = read("src/packages/service.rs");
    let tests = read("tests/package_graph.rs");

    for phrase in [
        "dependsOn",
        "extends",
        "disables",
        "replaces",
        "PackageGraphRelations",
        "PackageGraphPlan",
        "package-control",
        "MissingGraphTarget",
        "PackageGraphCycle",
        "MissingPackageControlGrant",
    ] {
        assert!(
            security.contains(phrase) || loading.contains(phrase) || wiki.contains(phrase),
            "package graph docs must document `{phrase}`"
        );
    }

    for phrase in [
        "parse_graph_relations",
        "parse_relation_array",
        "PackageGraphRelations",
        "InvalidPackageGraph",
    ] {
        assert!(
            manifest.contains(phrase),
            "manifest parser must implement graph relation phrase `{phrase}`"
        );
    }

    for phrase in [
        "PackageGraphPlan",
        "requires_package_control",
        "activation_targets",
        "controlled_targets",
        "cycle_from_stack",
    ] {
        assert!(
            graph.contains(phrase),
            "graph helper must expose `{phrase}`"
        );
    }

    for phrase in [
        "enable_graph",
        "resolve_graph_targets",
        "ensure_package_control_grant",
        "previous_enabled",
        "PackagePermission::PackageControl",
    ] {
        assert!(
            service.contains(phrase),
            "package service must evaluate graph relation phrase `{phrase}`"
        );
    }

    for phrase in [
        "package_with_package_control_disables_first_party_package",
        "package_extends_target_and_both_remain_active",
        "package_graph_reports_missing_target_deterministically",
        "package_graph_reports_dependency_cycles_deterministically",
        "disables_requires_explicit_package_control_grant",
    ] {
        assert!(
            tests.contains(phrase),
            "package graph behavior test `{phrase}` must exist"
        );
    }
}

#[test]
fn unified_package_authority_primitive_review_is_documented() {
    let review = read("docs/wiki/modules/unified-package-authority-primitive-review.md");
    let wiki_index = read("docs/wiki/index.md");

    assert!(
        wiki_index.contains("modules/unified-package-authority-primitive-review.md"),
        "wiki index must link the unified package authority primitive review"
    );

    for phrase in [
        "PackageSource -> PackageAuthorization -> PackageGraph -> RuntimeGeneration",
        "install != enable != load != runtime execution != package-manager execution != client behavior delivery",
        "PackageManagerBackend",
        "PackageService",
        "validate_manifest_value",
        "assemble_package_record",
        "parse_permission",
        "check_enabled_packages",
        "op_clay_packages_load_package_by_specifier",
        "PackageLoadEntryAllowlist",
        "ClayModuleLoader",
        "ParseCoordinator",
        "src/server/ui.rs",
        "src/shell/layout.rs",
        "Client delivery",
        "native-trust | sandboxed | restricted",
    ] {
        assert!(
            review.contains(phrase),
            "primitive review must inventory existing primitive `{phrase}`"
        );
    }

    for phrase in [
        "PackageSource provenance primitive",
        "PackageAuthorization primitive",
        "PackageGraph primitive",
        "ConflictResolution primitive",
        "PackageLoadEntryRegistry primitive",
        "PackageGenerationRevocation primitive",
        "RuntimeProfile primitive",
        "PackageInspection/Diagnostics primitive",
    ] {
        assert!(
            review.contains(phrase),
            "primitive review must document generic gap `{phrase}`"
        );
    }

    for phrase in [
        "startup, install, enable, load, reload, explicit user command, or background audit work",
        "No package source resolution, package-manager call, authorization prompt, graph traversal, JavaScript evaluation, or configuration evaluation may run from keypress, paint, layout, scroll, text-event, edit-ack, pointer, or Masonry hot paths",
        "Keep package-root confinement",
        "explicit user grants visible, revocable",
        "package-manager metadata diagnostic-only",
        "Do not add source-specific Rust branches",
    ] {
        assert!(
            review.contains(phrase),
            "primitive review must document hot-path/security boundary `{phrase}`"
        );
    }
}

#[test]
fn persistent_runtime_hardening_gate_doc_covers_threat_model() {
    let hardening = read("docs/wiki/modules/persistent-runtime-hardening.md");
    let sandbox_design = read("docs/design/persistent-runtime-sandbox.md");
    let wiki_index = read("docs/wiki/index.md");

    assert!(
        wiki_index.contains("modules/persistent-runtime-hardening.md"),
        "wiki index must link the persistent runtime hardening gate"
    );

    assert!(
        hardening.contains("docs/design/persistent-runtime-sandbox.md"),
        "hardening wiki must link the sandbox design gate"
    );
    assert!(
        sandbox_design.contains("separate-process JavaScript runtime sandbox as a hardening primitive and optional runtime profile"),
        "sandbox design must exist and state its scope"
    );

    for phrase in [
        "unified package authority model",
        "same user-approved capabilities",
        "separate-process sandbox",
        "V8 heap limits",
        "User authorization records",
        "package-control",
        "filesystem",
        "network",
        "shell",
        "WASM",
        "AI mutation",
        "raw-op",
        "native-widget",
        "client-side JavaScript",
        "package-manager execution",
        "These capabilities are grantable to any package source after user authorization",
        "keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers",
        "sanitized `clay.runtime.heap_limit` diagnostics",
    ] {
        assert!(
            hardening.contains(phrase),
            "hardening gate doc must document `{phrase}`"
        );
    }
}

#[test]
fn persistent_runtime_sandbox_design_pins_process_boundary() {
    let design = read("docs/design/persistent-runtime-sandbox.md");
    let hardening = read("docs/wiki/modules/persistent-runtime-hardening.md");

    for phrase in [
        "Parent process owns canonical documents",
        "Child process owns only V8/`deno_core` evaluation",
        "Protocol messages carry inert request/result data only",
        "length-prefixed and bounded",
        "parent-side validation",
        "Per-request timeout kills the child process",
        "Restart creates a fresh child",
        "stable Clay error code",
        "native-trust | sandboxed | restricted",
        "Load resolver-validated package `loadEntry` code from any enabled user-authorized package source",
        "filesystem",
        "network",
        "shell",
        "WASM",
        "package-manager handles",
        "native widget handles",
        "client JavaScript",
        "raw op names",
        "keypress, paint, layout, scroll, text-event, or edit-ack handlers",
    ] {
        assert!(
            design.contains(phrase),
            "sandbox design must pin `{phrase}`"
        );
    }

    assert!(
        hardening.contains("docs/design/persistent-runtime-sandbox.md"),
        "hardening wiki must link the sandbox design"
    );
}

#[test]
fn phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps() {
    let review =
        read("docs/wiki/modules/phase19-persistent-runtime-hot-reload-primitive-review.md");
    let wiki_index = read("docs/wiki/index.md");
    let embedded_runtime = read("docs/wiki/modules/embedded-js-runtime.md");
    let parse_coordinator = read("docs/wiki/modules/parse-coordinator.md");
    let server_ipc = read("docs/wiki/modules/server-ipc-skeleton.md");
    let workspace = read("docs/wiki/modules/server-file-workspace.md");
    let maintenance = read("docs/wiki/modules/maintenance-validation.md");
    let hot_reload_wiki = read("docs/wiki/modules/persistent-runtime-hot-reload.md");

    assert!(
        wiki_index.contains("modules/phase19-persistent-runtime-hot-reload-primitive-review.md"),
        "wiki index must link the Phase 19 hot reload primitive review"
    );
    assert!(
        wiki_index.contains("modules/persistent-runtime-hot-reload.md"),
        "wiki index must link the Phase 19 hot reload implementation page"
    );

    for phrase in [
        "RuntimeGeneration holder",
        "Generation-scoped package state",
        "Generation-scoped parse registrations",
        "Late-result guard",
        "Open-document refresh primitive",
        "Non-GUI trigger",
        "stale handler invalidation",
        "generation swap",
        "No JavaScript should be added to keypress, paint, layout, scroll",
        "module loading through recorded package allowlist entries",
        "current resolver-validated `@clay/*` loading as an implementation limit",
        "no Markdown-specific branch",
    ] {
        assert!(
            review.contains(phrase),
            "Phase 19 hot reload primitive review must document `{phrase}`"
        );
    }

    for phrase in [
        "RuntimeGenerationStore",
        "IpcServer::reload_runtime_generation",
        "keeps the previous generation ID/service active",
        "`current()` before selected-file activation",
    ] {
        assert!(
            embedded_runtime.contains(phrase),
            "embedded runtime wiki must document generation swap implementation phrase `{phrase}`"
        );
    }

    for phrase in [
        "register_handler_for_generation",
        "owning runtime generation ID",
        "aborts old-generation active tasks",
        "task generation still matches the active handler generation",
        "old-runtime-generation task results",
    ] {
        assert!(
            parse_coordinator.contains(phrase),
            "parse coordinator wiki must document generation replacement phrase `{phrase}`"
        );
    }

    for phrase in [
        "refresh_open_documents_after_reload",
        "generic mode classification/activation",
        "bounded parse refresh",
        "no `DocumentOpened`/`DocumentReloaded` full-text snapshots",
    ] {
        assert!(
            server_ipc.contains(phrase),
            "server IPC wiki must document reload open-document refresh phrase `{phrase}`"
        );
    }

    for phrase in [
        "open_document_snapshots",
        "reload-refresh",
        "does not authorize new paths",
    ] {
        assert!(
            workspace.contains(phrase),
            "workspace wiki must document reload snapshot phrase `{phrase}`"
        );
    }

    for phrase in [
        "RuntimeGenerationStore",
        "serialized candidate commit",
        "package cache invalidation",
        "parse-handler generation replacement",
        "open-document refresh",
        "No JavaScript in keypress, paint, layout, scroll, edit acknowledgement, or text-event hot paths",
        "No public Clay JS reload API",
        "sanitized diagnostics",
        "Tests",
    ] {
        assert!(
            hot_reload_wiki.contains(phrase),
            "hot reload implementation wiki must document `{phrase}`"
        );
    }

    for phrase in [
        "trigger_developer_hot_reload",
        "deterministic non-GUI reload trigger",
        "shared reload primitive",
        "does not run during ordinary client event processing",
    ] {
        assert!(
            embedded_runtime.contains(phrase) || maintenance.contains(phrase),
            "docs must document non-GUI reload trigger phrase `{phrase}`"
        );
    }
}

#[test]
fn phase19_load_package_cache_docs_pin_generation_invalidation() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let primitive = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let facade = read("runtime/js/packages.ts");
    let embedded_facade = read("src/server/js_runtime.rs");

    for source in [&package_guide, &primitive, &wiki] {
        for phrase in [
            "runtime generation",
            "`init.js`",
            "globalThis.__clayLoadedPackages",
            "loadEntry",
            "persistent",
        ] {
            assert!(
                source.contains(phrase),
                "package cache docs must document generation invalidation phrase `{phrase}`"
            );
        }
    }

    for source in [&facade, &embedded_facade] {
        assert!(
            source.contains("Per-runtime-generation cache")
                && (source.contains("hot reload invalidates")
                    || source.contains("Hot reload invalidates")),
            "loadPackage facade must explain cache lifetime and hot reload invalidation"
        );
    }
}

#[test]
fn phase19_package_author_docs_cover_reload_runtime_lifecycle() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let markdown = read("docs/reference/packages/markdown.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let runtime = read("docs/wiki/modules/embedded-js-runtime.md");

    for phrase in [
        "runtime generation",
        "empty `globalThis.__clayLoadedPackages` cache",
        "reruns `~/.config/clay/init.js`",
        "Package authors should rebuild all runtime state from `loadEntry`",
        "Failed reloads keep the previous generation active",
        "sanitized diagnostics",
        "generation-scoped",
        "old-runtime-generation parse results",
        "source-aware",
        "user-authorized package",
        "executable callback payload rejection",
        "never in keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers",
    ] {
        assert!(
            [&package_guide, &markdown, &wiki, &runtime]
                .iter()
                .any(|source| source.contains(phrase)),
            "package author/runtime docs must cover Phase 19 reload phrase `{phrase}`"
        );
    }
}

#[test]
fn package_author_docs_cover_generation_local_state_and_rollback() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let facade = read("runtime/js/packages.ts");
    let rust = read("docs/reference/packages/rust.md");
    let typescript = read("docs/reference/packages/typescript.md");
    let javascript = read("docs/reference/packages/javascript.md");

    for phrase in [
        "Package Reload Lifecycle",
        "Generation-local",
        "Explicitly persistent user/workspace state",
        "Unsupported migration hooks",
        "onReload",
        "migrateState",
        "force: true",
        "empty `globalThis.__clayLoadedPackages`",
        "keep the previous generation active",
        "authorizeLanguageServer",
        "does not broaden",
        "viewport-prioritized",
    ] {
        assert!(
            [&package_guide, &package_loading, &wiki, &facade]
                .iter()
                .any(|source| source.contains(phrase)),
            "package reload lifecycle docs must cover `{phrase}`"
        );
    }

    for (path, source, specifier) in [
        ("rust.md", &rust, "@clay/rust"),
        ("typescript.md", &typescript, "@clay/typescript"),
        ("javascript.md", &javascript, "@clay/javascript"),
    ] {
        assert!(
            source.contains(&format!("await loadPackage(\"{specifier}\")")),
            "{path} must preserve one-line loadPackage"
        );
        assert!(
            source.contains("empty `globalThis.__clayLoadedPackages` cache"),
            "{path} must document generation-local cache invalidation"
        );
        assert!(
            source.contains("failed reloads keep the prior")
                || source.contains("Failed reloads keep the prior"),
            "{path} must document rollback keeping the prior generation"
        );
    }

    assert!(
        package_loading.contains("## Package Reload Lifecycle"),
        "package-loading primitive must promote reload lifecycle out of deferrals"
    );
    assert!(
        !package_guide.contains("hot reload remain **Planned/target**"),
        "creating-packages must not still mark hot reload as Planned/target"
    );
    assert!(
        facade.contains("Generation-local only")
            || facade.contains("no package reload callback API"),
        "packages.ts must document generation-local cache and rejected force/callback APIs"
    );
}

#[test]
fn package_default_init_js_loading_documents_one_line_path_or_current_gap() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let facade = read("runtime/js/packages.ts");
    let embedded_facade = read("src/server/js_runtime.rs");
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");

    for source in [&package_guide, &package_loading, &wiki] {
        assert!(
            source.contains("loadPackage(\"@clay/markdown\")"),
            "package loading docs/wiki must preserve the one-line explicit init.js target"
        );
    }

    // Phase 18.5 added the planned inventory entry clay.packages.loadPackage to track
    // the preferred target. Phase 18.6 task 5 ships the loader as a runtime-backed
    // facade export. While the loader is unimplemented the test pins the doc gap;
    // once it ships the gap-phrase assertions are skipped (task 8 updates the docs
    // to describe the implemented loader, then replaces this branch).
    let one_line_loader_is_implemented = facade.contains("export function loadPackage(")
        || facade.contains("export async function loadPackage(")
        || embedded_facade.contains("export function loadPackage(")
        || embedded_facade.contains("export async function loadPackage(");
    assert!(
        one_line_loader_is_implemented,
        "The generic one-line package loader must ship as a runtime-backed facade export (`export ... function loadPackage`)"
    );

    // The planned inventory entry must exist to track the loader.
    assert!(
        inventory.contains("clay.packages.loadPackage"),
        "clay.packages.loadPackage must have a planned inventory entry tracking the preferred one-line target"
    );

    // The loader is implemented (Phase 18.6). The docs must describe the
    // implemented resolver and carried-forward source-aware package work.
    // The old gap phrases are no longer present.
    if one_line_loader_is_implemented {
        // Authoritative reference docs must describe the resolver mechanics.
        for phrase in [
            "Phase 18.6 shipped",
            "PackageService",
            "loadEntry",
            "runtime-backed",
        ] {
            assert!(
                [&package_guide, &package_loading]
                    .iter()
                    .any(|source| source.contains(phrase)),
                "package docs must describe the implemented resolver with phrase `{phrase}`"
            );
        }
        for phrase in ["source-aware", "load-entry allowlist"] {
            assert!(
                package_loading.contains(phrase),
                "package loading reference must describe unified resolver direction `{phrase}`"
            );
        }
        // The implementation wiki summarizes the resolver at a higher level.
        assert!(
            wiki.contains("Phase 18.6 implemented")
                && wiki.contains("loadPackage")
                && wiki.contains("source-aware"),
            "package loading wiki must summarize the Phase 18.6 implementation"
        );

        for source in [&package_guide, &package_loading, &wiki] {
            // Old gap phrases must be gone or replaced.
            assert!(
                !source.contains("generic one-line loader is not implemented yet"),
                "docs must not claim the loader is unimplemented after Phase 18.6"
            );
            assert!(
                !source.contains("generic loader/API gap"),
                "docs must not describe the resolver as a gap after Phase 18.6"
            );
            assert!(
                !source.contains("temporary validation/loading gap"),
                "docs must not describe the resolver as a temporary gap after Phase 18.6"
            );
        }

        // The inventory entry must be promoted to runtime-backed.
        assert!(
            inventory.contains("status = \"runtime-backed\"")
                && inventory.contains("registry_public = true")
                && inventory.contains("op_clay_packages_load_package_by_specifier"),
            "clay.packages.loadPackage inventory entry must be promoted to runtime-backed with concrete paths"
        );

        // The resolver op must be documented as the concrete implementation.
        assert!(
            package_loading.contains("op_clay_packages_load_package_by_specifier")
                && package_loading.contains("src/server/ops/packages.rs"),
            "package loading reference must document the concrete resolver op"
        );
        for source in [&package_guide, &package_loading, &wiki] {
            for phrase in [
                "Phase 18.7",
                "persistent runtime",
                "selected-file open",
                "ParseCoordinator",
                "idempotent",
            ] {
                assert!(
                    source.contains(phrase),
                    "default init.js docs/wiki must cover open-time activation phrase `{phrase}`"
                );
            }
            for forbidden in [
                "copy package manifests",
                "manual primitive registration",
                "representative decoration publication",
                "per-open runtime roots",
            ] {
                assert!(
                    source.contains(forbidden),
                    "default init.js docs/wiki must forbid `{forbidden}`"
                );
            }
        }
        assert!(
            package_loading.contains("source-aware")
                && package_loading.contains("Durable package state")
                && (package_loading.contains("Hot-reload")
                    || package_loading.contains("hot reload")),
            "package loading reference must document implemented source-aware loading and carried-forward durable state work"
        );
        assert!(
            package_loading.contains("serverLoadPackage")
                && package_loading.contains("remains a lower-level validation helper"),
            "package loading reference must reframe serverLoadPackage as a helper, not a gap"
        );
    }
}

#[test]
fn package_default_init_js_user_installed_one_line_path_is_documented_and_verified() {
    // Plan 035 task 8: the one-line init.js default must work for user-installed
    // (non-@clay/*) packages, and init.js itself must grant no capabilities.
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let facade = read("runtime/js/packages.ts");
    let embedded_facade = read("src/server/js_runtime.rs");

    // The one-line path must explicitly cover user-installed source specifiers,
    // not just @clay/markdown.
    for source in [&package_guide, &package_loading, &wiki] {
        assert!(
            source.contains("@vendor/foo") || source.contains("github:user/repo"),
            "default init.js docs must cover a user-installed package one-line example"
        );
    }
    // The facade docstring must explain the one-line init.js path covers
    // bundled and user-installed packages.
    assert!(
        facade.contains("@vendor/foo")
            && facade.contains("github:user/repo")
            && embedded_facade.contains("@vendor/foo"),
        "loadPackage facade must document the one-line path for user-installed packages"
    );
    // init.js grants no capabilities: every powerful capability is a separate
    // user-approved authorization grant, not something init.js can silently
    // confer through loadPackage or other Clay APIs.
    for source in [&package_guide, &package_loading, &wiki] {
        assert!(
            source.contains("grants no capabilities")
                || source.contains("init.js cannot silently grant"),
            "docs must state init.js grants no capabilities on its own"
        );
    }
    // Runtime test must prove the user-installed one-line path through a real
    // init.js config root, not only a controlled module source.
    assert!(
        embedded_facade.contains("load_package_user_installed_default_loads_from_init_js"),
        "runtime tests must cover the user-installed package one-line init.js load"
    );
}

#[test]
fn grammar_package_reference_docs_cover_authoring_primitive_security_and_performance() {
    let docs_index = read("docs/index.md");
    let primitive_index = read("docs/reference/primitives/index.md");
    let registry = read("docs/reference/primitives/registry.md");
    let package_security = read("docs/reference/primitives/package-security.md");
    let parse_strategy = read("docs/reference/primitives/parse-update-strategy.md");
    let rendering_strategy = read("docs/reference/primitives/rendering-strategy.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let syntax_wiki = read("docs/wiki/modules/syntax-grammar-registry.md");

    assert!(
        primitive_index.contains("#phase-1810-authoring-contract-grammar-only-syntax-packages"),
        "primitive index must link the Phase 18.10 grammar authoring contract, not an older anchor"
    );

    let primitive_docs = [
        registry.as_str(),
        package_security.as_str(),
        parse_strategy.as_str(),
        rendering_strategy.as_str(),
        package_guide.as_str(),
        syntax_wiki.as_str(),
    ]
    .join("\n---\n");
    for phrase in [
        "SyntaxGrammarContribution",
        "active major mode",
        "core.code",
        "core.text",
        "first-party",
        "package-root-confined",
        "tree-sitter-wasm",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
        "Background",
        "no-hot-path",
    ] {
        assert!(
            primitive_docs.contains(phrase),
            "grammar primitive docs must mention `{phrase}`"
        );
    }

    for package in ["rust", "typescript", "javascript"] {
        let reference_doc = read(&format!("docs/reference/packages/{package}.md"));
        let package_doc = read(&format!("packages/{package}/docs/index.md"));
        assert!(
            docs_index.contains(&format!("reference/packages/{package}.md")),
            "docs/index.md must link @{package} reference docs"
        );
        for source in [&reference_doc, &package_doc] {
            for phrase in [
                "loadPackage(\"@clay/",
                "not auto-loaded",
                "core.code",
                "parse-document",
                "render-decorations",
                "Vocabulary styleMap",
                "third-party/native grammar artifact loading",
                "keypress, paint, layout, scroll, pointer, or text-event hot paths",
            ] {
                assert!(
                    source.contains(phrase),
                    "{package} docs must mention `{phrase}`"
                );
            }
        }
    }

    // Expanded language packages document Phase 18.14 surfaces.
    for (package, mode_id, command_id, provider_id, status_id) in [
        (
            "rust",
            "rust",
            "rust.toggleLineComment",
            "rust.keywords",
            "rust.status.mode",
        ),
        (
            "typescript",
            "typescript",
            "typescript.toggleLineComment",
            "typescript.keywords",
            "typescript.status.mode",
        ),
        (
            "javascript",
            "javascript",
            "javascript.toggleLineComment",
            "javascript.keywords",
            "javascript.status.mode",
        ),
    ] {
        let reference_doc = read(&format!("docs/reference/packages/{package}.md"));
        let package_doc = read(&format!("packages/{package}/docs/index.md"));
        for source in [&reference_doc, &package_doc] {
            for phrase in [
                "Phase 18.14",
                &format!("Major mode `{mode_id}`"),
                "Behavior manifest",
                &format!("Command `{command_id}`"),
                &format!("Completion provider `{provider_id}`"),
                &format!("Status item `{status_id}`"),
                "Active syntax grammar remains selectable independently",
                "LSP",
            ] {
                assert!(
                    source.contains(phrase),
                    "{package} docs must document Phase 18.14 expansion with `{phrase}`"
                );
            }
        }
    }

    assert!(
        package_guide.contains("serverRegisterSyntaxGrammar")
            && !package_guide.contains("Runtime facade/op remains a later Phase 18.10 task"),
        "package guide should document manifest API dependency without presenting low-level manual registration as end-user setup"
    );
    assert!(
        !package_guide.contains("serverLoadPackage(packageJson) as the ordinary end-user"),
        "package guide must not present serverLoadPackage(packageJson) as ordinary setup"
    );
}

#[test]
fn syntax_grammar_packages_document_explicit_init_js_loading() {
    let load_package_api = read("docs/reference/clay-js-api/packages/load-package.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let syntax_wiki = read("docs/wiki/modules/syntax-grammar-registry.md");
    let package_loading_wiki = read("docs/wiki/modules/package-loading.md");
    let fixture = read("tests/fixtures/configuration/syntax-grammars-init.js");
    let runtime = read("src/server/js_runtime.rs");
    let package_ops = read("src/server/ops/packages.rs");

    for specifier in [
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "@clay/markdown",
    ] {
        for source in [&load_package_api, &package_guide, &syntax_wiki, &fixture] {
            assert!(
                source.contains(&format!("loadPackage(\"{specifier}\")")),
                "explicit init.js docs/fixture must show one-line load for {specifier}"
            );
        }
    }

    for source in [
        &load_package_api,
        &package_guide,
        &syntax_wiki,
        &package_loading_wiki,
    ] {
        assert!(
            source.contains("grants no capabilities")
                || source.contains("does not grant")
                || source.contains("no capabilities of its own"),
            "docs must state loadPackage/init.js does not grant extra authority"
        );
        assert!(
            source.contains("not auto-loaded")
                || source.contains("no automatic language package load")
                || source.contains("explicit"),
            "docs must forbid silent grammar package auto-loading"
        );
    }

    assert!(
        fixture.contains("loadPackage")
            && !fixture.contains("serverLoadPackage")
            && !fixture.contains("serverRegisterSyntaxGrammar")
            && !fixture.contains("Deno.core.ops"),
        "syntax grammar init fixture must use only end-user loadPackage calls"
    );
    assert!(
        runtime.contains("syntax_grammar_packages_default_load_from_init_js"),
        "runtime tests must execute the syntax grammar init.js default path"
    );
    assert!(
        package_ops.contains("\"syntaxGrammars\": record.contributions.syntax_grammars.len()"),
        "loadPackage summary must expose syntax grammar contribution counts"
    );
}

#[test]
fn syntax_grammar_configuration_review_uses_only_documented_clay_js_apis() {
    let configuration = read("docs/reference/clay-js-api/configuration.md");
    let load_package_api = read("docs/reference/clay-js-api/packages/load-package.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let syntax_wiki = read("docs/wiki/modules/syntax-grammar-registry.md");
    let fixture = read("tests/fixtures/configuration/syntax-grammars-init.js");
    let api_inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let generated_registry = read("docs/generated/clay-js-api-registry.json");

    for phrase in [
        "Phase 18.10 syntax grammar configuration review",
        "does **not** promote a new user-facing syntax configuration API",
        "explicit first-party package loading from `~/.config/clay/init.js`",
        "loadPackage(\"@clay/rust\")",
        "loadPackage(\"@clay/typescript\")",
        "loadPackage(\"@clay/javascript\")",
        "No hidden JSON/TOML/ad hoc syntax keys are valid",
        "syntax.preferredGrammar",
        "treeSitter.grammarPath",
        "syntax.styleMap",
        "autoLoadSyntaxPackages",
        "not end-user configuration knobs",
        "not automatic core loading and not an auto-load flag",
        "documented Clay JS API with custom properties",
    ] {
        assert!(
            configuration.contains(phrase),
            "configuration docs must pin syntax grammar configuration boundary: {phrase}"
        );
    }

    assert!(
        [
            "@clay/rust",
            "@clay/typescript",
            "@clay/javascript",
            "@clay/markdown",
        ]
        .iter()
        .all(|specifier| load_package_api.contains(&format!("await loadPackage(\"{specifier}\");"))),
        "loadPackage docs must remain the end-user syntax grammar configuration path"
    );
    assert!(
        package_guide.contains("Do not add hidden JSON/TOML/ad hoc syntax configuration keys")
            && syntax_wiki.contains("User config remains one-line `loadPackage`"),
        "package authoring/wiki docs must reject hidden syntax configuration keys"
    );
    assert!(
        fixture.contains("loadPackage")
            && !fixture.contains("serverRegisterSyntaxGrammar")
            && !fixture.contains("Deno.core.ops")
            && !fixture.contains("syntax.preferredGrammar")
            && !fixture.contains("treeSitter.grammarPath")
            && !fixture.contains("autoLoadSyntaxPackages"),
        "syntax init fixture must use only documented loadPackage calls"
    );

    for forbidden_api in [
        "clay.configuration.setSyntaxGrammar",
        "clay.configuration.setSyntaxStyleMap",
        "clay.configuration.setTreeSitterGrammarPath",
        "syntax.preferredGrammar",
        "treeSitter.grammarPath",
        "autoLoadSyntaxPackages",
    ] {
        assert!(
            !api_inventory.contains(forbidden_api),
            "API inventory must not promote hidden syntax config `{forbidden_api}`"
        );
        assert!(
            !generated_registry.contains(forbidden_api),
            "generated registry must not promote hidden syntax config `{forbidden_api}`"
        );
    }
}

#[test]
fn syntax_engine_configuration_is_documented_or_explicitly_not_needed() {
    let configuration = read("docs/reference/clay-js-api/configuration.md");
    let api_doc = read("docs/reference/clay-js-api/syntax/set-syntax-engine-preference.md");
    let api_inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let generated_registry = read("docs/generated/clay-js-api-registry.json");
    let syntax_wiki = read("docs/wiki/modules/syntax-grammar-registry.md");

    for phrase in [
        "Phase 18.16 syntax engine configuration review",
        "clay.syntax.setSyntaxEnginePreference",
        "No call is needed for normal use",
        "Tier 1 native for first-party Rust, TypeScript, TSX, JavaScript, and Markdown",
        "explicit `wasm` enables the Tier 2 artifact path",
        "explicit `javascript`/`js` suppresses syntax-grammar selection",
        "Hidden JSON/TOML/ad hoc syntax-engine keys remain invalid",
        "syntax.engine",
        "treeSitter.wasmPath",
        "custom_properties` for `target` and `tier`",
        "empty `key_bindings`",
        "Configuration evaluation and preference lookup happen only during startup, package load, document open, reload, or reclassification work",
        "packages cannot silently promote themselves over native tier",
    ] {
        assert!(
            configuration.contains(phrase),
            "configuration docs must pin Phase 18.16 syntax engine configuration boundary: {phrase}"
        );
    }

    for source in [&api_doc, &api_inventory, &generated_registry] {
        for phrase in [
            "setSyntaxEnginePreference",
            "target",
            "tier",
            "native",
            "wasm",
            "javascript",
            "configuration",
        ] {
            assert!(
                source.contains(phrase),
                "syntax engine preference API docs/registry must expose `{phrase}`"
            );
        }
    }

    assert!(
        syntax_wiki.contains("setSyntaxEnginePreference")
            && syntax_wiki.contains("User config remains one-line `loadPackage`"),
        "syntax wiki must connect engine preferences to existing init.js loading contract"
    );
}

#[test]
fn syntax_engine_preference_requires_documented_clay_js_api() {
    let configuration = read("docs/reference/clay-js-api/configuration.md");
    let facade = read("runtime/js/syntax.ts");
    let op = read("src/server/ops/syntax.rs");
    let registry =
        clay::docs::registry::ClayJsApiRegistry::from_generated().expect("load generated registry");
    let entry = registry
        .by_id("clay.syntax.setSyntaxEnginePreference")
        .expect("generated registry exposes syntax engine preference API");

    assert_eq!(entry.js_module, "clay:syntax");
    assert_eq!(entry.js_export, "setSyntaxEnginePreference");
    assert_eq!(entry.key_bindings.len(), 0);
    assert!(entry.permissions.is_empty());
    for property in ["target", "tier"] {
        assert!(
            entry
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "generated registry must preserve syntax preference custom property {property}"
        );
    }
    for tag in ["syntax", "engine", "configuration", "phase18.16"] {
        assert!(
            entry.lookup_tags.iter().any(|lookup_tag| lookup_tag == tag),
            "generated registry must expose lookup tag {tag}"
        );
    }
    for denied in [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package-manager",
        "native-library",
        "WASM artifact",
        "client-side JavaScript",
        "raw-op",
    ] {
        assert!(
            entry.security.contains(denied),
            "syntax engine preference API must deny {denied} authority"
        );
    }

    assert!(facade.contains("op_clay_syntax_set_engine_preference"));
    assert!(op.contains("tier must be native, wasm, or javascript"));
    for forbidden in [
        "syntax.preferredEngine",
        "treeSitter.engine",
        "treeSitter.wasmPath",
        "autoLoadSyntaxPackages",
    ] {
        assert!(
            configuration.contains(forbidden),
            "configuration docs must explicitly reject hidden syntax key {forbidden}"
        );
        assert!(
            !facade.contains(forbidden) && !op.contains(forbidden),
            "hidden syntax key {forbidden} must not appear in runtime facade/op"
        );
    }
}

#[test]
fn phase18_14_language_package_default_init_js_loading_is_documented() {
    // Phase 18.14 expands @clay/rust, @clay/typescript, and @clay/javascript
    // into full language packages while keeping the same one-line explicit
    // init.js default. This test pins the default-loading contract across
    // package reference docs and the authoring guide.
    let rust_ref = read("docs/reference/packages/rust.md");
    let ts_ref = read("docs/reference/packages/typescript.md");
    let js_ref = read("docs/reference/packages/javascript.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");

    for (name, source) in [
        ("rust", &rust_ref),
        ("typescript", &ts_ref),
        ("javascript", &js_ref),
    ] {
        for phrase in [
            "Default `~/.config/clay/init.js` loading is one explicit line",
            "import { loadPackage } from \"clay:packages\";",
            &format!("await loadPackage(\"@clay/{name}\");"),
            "The package is explicit opt-in and is not auto-loaded",
            "Optional customization is exposed through documented Clay/package JS APIs",
        ] {
            assert!(
                source.contains(phrase),
                "{name}.md must document the Phase 18.14 default init.js loading contract with `{phrase}`"
            );
        }
    }

    for phrase in [
        "Phase 18.14 authoring contract: upgrading grammar-only language packages to full language packages",
        "End-user default remains one explicit line per package in `~/.config/clay/init.js`",
        "await loadPackage(\"@clay/rust\");",
        "await loadPackage(\"@clay/typescript\");",
        "await loadPackage(\"@clay/javascript\");",
        "Optional customization is exposed through documented Clay/package JS APIs, not by copying the package manifest into `init.js`",
        "Keep the `syntaxGrammars` block exactly as shipped in Phase 18.10",
        "active syntax grammar remains selectable independently of its active major mode",
        "Add `if mode == \"rust\"` or `if extension == \"ts\"` branches in the Rust client or server core.",
        "LSP",
    ] {
        assert!(
            package_guide.contains(phrase),
            "creating-packages.md must document the Phase 18.14 language-package upgrade loading contract with `{phrase}`"
        );
    }
}

#[test]
fn phase18_18_behavior_manifest_helper_is_documented() {
    let package_guide = read("docs/reference/packages/creating-packages.md");

    for phrase in [
        "clay.behavior.buildCodeEditingManifest",
        "clay.completion.completionTriggerCharactersFromEditorRules",
        "indentSize",
        "lineComment",
        "enter",
        "pairs",
        "electricOutdentCharacters",
        "autocompleteTriggers",
        "validated inert rules",
        "behavior manifest API",
        "Derive `triggerCharacters` from the major-mode behavior manifest",
    ] {
        assert!(
            package_guide.contains(phrase),
            "creating-packages.md must document the generic behavior-manifest helper with `{phrase}`"
        );
    }
}

#[test]
fn package_author_guide_documents_first_party_language_contract() {
    let guide = read("docs/reference/packages/creating-packages.md");
    let vocabulary = read("docs/reference/primitives/syntax-vocabulary.md");

    for phrase in [
        "Phase 18.18 authoring contract: complete first-party language packages",
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "@clay/markdown",
        "serverRegisterSyntaxGrammar",
        "buildCodeEditingManifest",
        "serverRegisterCompletionProvider",
        "serverRegisterComponentContribution",
        "priority 0",
        "Phase 18.19 owns snippet transforms",
        "loadEntry` once per runtime generation",
        "loadPackage` grants no capabilities",
        "first-party engine",
        "Arbitrary third-party grammar/native loading and LSP process authority remain deferred",
        "keypress, paint, layout, scroll, pointer, or text-event hot paths",
    ] {
        assert!(
            guide.contains(phrase),
            "package guide must document first-party language contract `{phrase}`"
        );
    }
    for phrase in [
        "Package styleMap authoring",
        "clay.contributions.syntaxGrammars[].styleMap",
        "closed `TokenType` variant",
        "raw CSS",
        "Creating Clay Packages: complete first-party language packages",
    ] {
        assert!(
            vocabulary.contains(phrase),
            "vocabulary reference must document styleMap authoring `{phrase}`"
        );
    }
}

#[test]
fn package_author_guide_documents_markdown_decoration_preview_split() {
    let guide = read("docs/reference/packages/creating-packages.md");

    for phrase in [
        "Markdown decoration and preview are separate",
        "tree-sitter-md-025",
        "queries/highlights.scm",
        "Tier 3 JavaScript fallback",
        "setSyntaxEnginePreference(\"markdown\", \"javascript\")",
        "no default parser-backed `decorations` contribution",
        "preview remains package-JS SDUI",
        "Fixed panels consume a declared slot; transient overlays do not",
        "Masonry widget",
        "user-over-package precedence",
        "SduiSnapshot",
    ] {
        assert!(
            guide.contains(phrase),
            "package guide must document Markdown decoration/preview split `{phrase}`"
        );
    }
}

#[test]
fn phase18_14_ui_layout_authoring_contract_is_documented() {
    // Phase 18.14 language packages may contribute UI, but only through
    // validated inert declarations. This test pins the package guide language
    // that prevents drift toward Masonry widget creation, raw CSS, or client JS.
    let package_guide = read("docs/reference/packages/creating-packages.md");

    for phrase in [
        "UI/layout authoring contract for language packages",
        "validated, inert declarations",
        "Clay owns the working area",
        "mandatory `main` editor slot",
        "serverRegisterComponentContribution",
        "kind: \"statusItem\"",
        "serverRegisterPanelContribution",
        "defaultVisibility: \"hidden\"",
        "serverRegisterTransientOverlayContribution",
        "serverRegisterThemeToken",
        "Packages never create Masonry widgets",
        "Packages must not",
        "raw CSS",
        "client-side JavaScript",
        "raw `Deno.core.ops`",
        "file-browser roots",
        "Layout overrides and package options",
        "clay.configuration.setPackageOption",
        "clay.ui.serverSetLayoutOverride",
    ] {
        assert!(
            package_guide.contains(phrase),
            "creating-packages.md must document the Phase 18.14 UI/layout authoring contract with `{phrase}`"
        );
    }
}

#[test]
fn phase18_14_configuration_contract_defers_user_tunable_keys() {
    // Phase 18.14 language packages ship package-defined defaults and do not
    // introduce new user-tunable configuration keys. This test pins that
    // contract so future phases cannot silently add ad hoc config paths.
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let rust_doc = read("docs/reference/packages/rust.md");
    let typescript_doc = read("docs/reference/packages/typescript.md");
    let javascript_doc = read("docs/reference/packages/javascript.md");

    for phrase in [
        "Configuration contract for language packages",
        "package-defined defaults",
        "do not introduce new user-tunable configuration keys",
        "clay.configuration.setPackageOption",
        "clay.contributions.packageOptions",
        "`setPackageOption` with ad hoc language-package keys is unsupported and will be rejected by validation.",
    ] {
        assert!(
            package_guide.contains(phrase),
            "creating-packages.md must document the Phase 18.14 configuration contract with `{phrase}`"
        );
    }

    for (name, doc) in [
        ("rust.md", rust_doc),
        ("typescript.md", typescript_doc),
        ("javascript.md", javascript_doc),
    ] {
        for phrase in [
            "## Configuration",
            "package-defined values",
            "No new user-tunable configuration keys",
            "clay.configuration.setPackageOption",
        ] {
            assert!(
                doc.contains(phrase),
                "{name} must document the Phase 18.14 configuration contract with `{phrase}`"
            );
        }
    }
}

#[test]
fn package_default_load_gap_is_decision_log_backed_with_package_owned_fallback() {
    // Phase 18.5 (plans/028 Task 4) defers the generic loadPackage("@clay/*")
    // resolver with a decision-log-backed rationale and ships a clean
    // package-owned fallback entry. This test pins both halves so the gap
    // cannot drift back to fixture-only copied manifests.
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let markdown_docs = read("packages/markdown/docs/index.md");
    let decision_log =
        read("decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md");
    let load_entry = read("packages/markdown/dist/load.js");
    let package_index = read("packages/markdown/dist/index.js");

    // The former first-party resolver deferral is superseded by unified package authority.
    assert!(
        package_loading.contains("2026-06-27-2014-unified-user-authorized-package-authority")
            || wiki.contains("source-aware"),
        "docs must reference the unified package authority direction"
    );
    for phrase in [
        "Defer the generic",
        "loadPackage(\"@clay/*\")",
        "ClayModuleLoader",
        "canonical_local_file",
        "security-critical",
        "markdownLoadMode",
        "Alternatives Considered",
        "explicitly_approved_by_user",
    ] {
        assert!(
            decision_log.contains(phrase),
            "loadPackage deferral decision log must record `{phrase}`"
        );
    }

    // The package-owned fallback entry exists, imports Clay facades directly,
    // and reuses loadMarkdownPackage without an inline manifest.
    assert!(
        load_entry.contains("export async function markdownLoadMode(options = {})"),
        "package load entry must export markdownLoadMode"
    );
    for facade in [
        "import { serverRegisterCommand } from \"clay:commands\"",
        "import { serverActivateMajorMode, serverRegisterModePattern } from \"clay:modes\"",
        "import { serverLoadPackage } from \"clay:packages\"",
        "import { serverRegisterParseHandler } from \"clay:parse\"",
    ] {
        assert!(
            load_entry.contains(facade),
            "package load entry must import Clay facade directly: {facade}"
        );
    }
    assert!(
        load_entry.contains("return loadMarkdownPackage(clay, options);")
            && !load_entry.contains("const markdownPackage = {"),
        "markdownLoadMode must reuse loadMarkdownPackage and must not declare an inline manifest"
    );
    assert!(
        package_index.contains("from \"./load.js\"")
            && package_index.contains("export {")
            && package_index.contains("loadMarkdownPackage")
            && package_index.contains("markdownLoadMode"),
        "package root index must re-export `loadMarkdownPackage` and `markdownLoadMode` \
         from `./load.js` so the `import {{ markdownLoadMode }} from \"@clay/markdown\"` \
         documented fallback resolves (additional re-exported names are allowed)"
    );

    // The documented fallback is concise and uses implemented generic primitives.
    for source in [&package_guide, &markdown_docs] {
        assert!(
            source.contains("import { markdownLoadMode } from \"@clay/markdown\""),
            "docs must show the concise package-owned fallback import"
        );
        assert!(
            source.contains("await markdownLoadMode();"),
            "docs must show the concise package-owned fallback call"
        );
    }
}

#[test]
fn package_author_guide_documents_persistent_parse_and_open_time_contract() {
    let package_guide = read("docs/reference/packages/creating-packages.md");

    for phrase in [
        "Persistent runtime, open-time activation, and parse boundaries",
        "loadPackage(\"@clay/markdown\")",
        "serverRegisterParseHandler",
        "module: parserModule",
        "exportName",
        "server-issued token",
        "PackagePermission::ParseDocument",
        "ParseCoordinator",
        "serverActivateClassifiedMode",
        "no-client-JS",
        "no-hot-path-JS",
        "clay.runtime.timeout",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
        "Generic future-mode shape",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package author guide must document Phase 18.7 parse/open contract phrase `{phrase}`"
        );
    }

    for forbidden in [
        "Per-open runtimes",
        "per-open `dist/` copies",
        "Executable `handler`, `callback`, `onParse`, or `function` fields",
        "Raw `Deno.core.ops` calls",
        "Markdown-only Rust branches",
        "Publishing representative/fake decorations",
        "Client-side JavaScript",
        "direct Masonry widgets",
    ] {
        assert!(
            package_guide.contains(forbidden),
            "package author guide must list forbidden anti-pattern `{forbidden}`"
        );
    }
}

#[test]
fn package_author_guide_documents_native_wasm_js_fallback_routes() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_docs = [
        read("docs/reference/packages/rust.md"),
        read("docs/reference/packages/typescript.md"),
        read("docs/reference/packages/javascript.md"),
        read("docs/reference/packages/markdown.md"),
        read("packages/rust/docs/index.md"),
        read("packages/typescript/docs/index.md"),
        read("packages/javascript/docs/index.md"),
        read("packages/markdown/docs/index.md"),
    ];

    for phrase in [
        "Phase 18.16 authoring contract: tiered syntax engine",
        "Tier 1 — native first-party",
        "Tier 2 — web-tree-sitter WASM",
        "Tier 3 — package JavaScript fallback",
        "setSyntaxEnginePreference",
        "TokenType` + `Modifiers",
        "clay.parse.open_failed",
        "Open is enqueue-only",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
        "tree-sitter build --wasm",
        "grammars/PROVENANCE.md",
        "third-party grammar/native trust is deferred to Phase 23",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package author guide must document syntax engine route `{phrase}`"
        );
    }

    for (index, docs) in package_docs.iter().enumerate() {
        for phrase in [
            "Tier 1",
            "Tier 2",
            "Tier 3",
            "setSyntaxEnginePreference",
            "TokenType",
            "Modifiers",
            "clay.parse.open_failed",
            "confined WASM",
            "third-party/native grammar artifact loading",
            "keypress, paint, layout, scroll, pointer, or text-event hot paths",
        ] {
            assert!(
                docs.contains(phrase),
                "package reference {index} must document syntax engine marker `{phrase}`"
            );
        }
    }
}

#[test]
fn package_customization_uses_documented_configuration_apis() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let configuration_wiki = read("docs/wiki/modules/configuration-runtime.md");

    for source in [&package_guide, &package_loading, &wiki, &configuration_wiki] {
        for phrase in [
            "setPackageOption",
            "serverSetLayoutOverride",
            "documented Clay JS APIs",
            "hidden JSON/TOML/ad hoc",
            "startup",
            "package-load",
            "configuration-change",
            "Masonry",
        ] {
            assert!(
                source.contains(phrase),
                "package customization docs/wiki must mention `{phrase}`"
            );
        }
    }

    for phrase in [
        "layout.defaultVisibility",
        "layout.defaultSlot",
        "input.default",
        "action.default",
        "themeTokenRemap",
        "slot",
        "visibility",
        "themeToken",
    ] {
        assert!(
            package_guide.contains(phrase) || configuration_wiki.contains(phrase),
            "customization docs must cover supported option/override `{phrase}`"
        );
    }
}

/// Plan 030 task "Define and verify the package default init.js loading
/// experience": pins that the one-line default (`loadPackage("@clay/markdown")`)
/// and the hardened lifecycle-script suppression are two distinct paths that do
/// not interfere. First-party `loadPackage` resolves through the registry/
/// `PackageLoadEntryAllowlist` and never invokes the pnpm backend or its
/// lifecycle scripts; `--ignore-scripts` applies only to `clay package add`.
/// This guards against a future regression where the two paths get conflated
/// and the default-load experience breaks because of install-time hardening.
#[test]
fn default_load_path_is_separate_from_lifecycle_script_suppression() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    // One-line default is documented in every package-loading surface.
    for source in [&package_loading, &package_guide, &wiki] {
        assert!(
            source.contains("loadPackage(\"@clay/markdown\")"),
            "package loading docs/wiki must preserve the one-line explicit init.js default"
        );
    }

    // Lifecycle-script suppression is documented as a `clay package add`
    // concern in the authoritative reference + implementation wiki (the
    // end-user package-creation guide intentionally does not surface the
    // install-time flag detail).
    for source in [&package_loading, &wiki] {
        assert!(
            source.contains("--ignore-scripts"),
            "package loading docs/wiki must document the `--ignore-scripts` default"
        );
    }
    assert!(
        package_loading.contains("--allow-scripts")
            && package_loading.contains("CLAY_ALLOW_LIFECYCLE_SCRIPTS"),
        "package loading reference must document the `--allow-scripts` flag and `CLAY_ALLOW_LIFECYCLE_SCRIPTS` env var opt-in"
    );

    // The two paths are distinct: the install/backend paragraph is about
    // `clay package add`, not about first-party `loadPackage`. The docs must
    // keep `clay package add` (backend) text separate from the resolver text.
    assert!(
        package_loading.contains("clay package add <spec>")
            && package_loading.contains("PnpmBackend"),
        "package loading reference must document the `clay package add` backend path separately"
    );
    assert!(
        package_loading.contains("@clay/*") && package_loading.contains("load-entry allowlist"),
        "package loading reference must document the package resolver path as distinct from install"
    );
}

#[test]
fn phase18_3_package_loading_docs_cover_slot_ui_metadata_validation() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &wiki] {
        for phrase in [
            "ui.panels",
            "ui.components",
            "ui.overlays",
            "themeTokens",
            "typed style variables",
            "action targets",
            "same-type core token fallbacks",
            "duplicate fixed slot claims",
            "bounded payload",
        ] {
            assert!(
                source.contains(phrase),
                "package loading docs/wiki must mention Phase 18.3 package UI validation phrase `{phrase}`"
            );
        }
        for prohibition in [
            "raw CSS",
            "client JavaScript",
            "direct Masonry",
            "native handles",
        ] {
            assert!(
                source.contains(prohibition),
                "package loading docs/wiki must preserve package UI non-authority `{prohibition}`"
            );
        }
    }
}

#[test]
fn phase18_4_package_loading_docs_cover_input_state_config_metadata_validation() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_security = read("docs/reference/primitives/package-security.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &package_security, &package_guide, &wiki] {
        for phrase in [
            "input",
            "uiStateScopes",
            "layoutOverrides",
            "packageOptions",
            "registered actions",
            "package-configuration",
            "hidden-key rejection",
            "state-value rejection",
            "duplicate input",
            "duplicate UI state scope",
            "duplicate layout override",
            "duplicate package option",
            "package provenance",
        ] {
            assert!(
                source.contains(phrase),
                "package loading/security docs must mention Phase 18.4 metadata phrase `{phrase}`"
            );
        }
    }
}

#[test]
fn phase18_parse_decoration_apis_are_documented_without_raw_op_exposure() {
    let runtime = read("src/server/js_runtime.rs");
    let decorations = read("runtime/js/decorations.ts");
    let parse = read("runtime/js/parse.ts");
    let package_loading = read("docs/reference/primitives/package-loading.md");

    assert!(runtime.contains("\"clay:decorations\" => Some(CLAY_FACADE_DECORATIONS)"));
    assert!(runtime.contains("\"clay:parse\" => Some(CLAY_FACADE_PARSE)"));
    assert!(decorations.contains("serverPublishDecorations"));
    assert!(parse.contains("serverRegisterParseHandler"));
    assert!(
        package_loading.contains("planned-unavailable errors")
            || read("docs/reference/clay-js-api/api-inventory.toml")
                .contains("clay.decorations.serverPublishDecorations")
    );

    for (path, source) in [
        ("runtime/js/decorations.ts", decorations),
        ("runtime/js/parse.ts", parse),
    ] {
        assert!(
            !source.contains("Deno.core.ops."),
            "{path} must not expose raw Deno.core.ops dot calls"
        );
        for line in source
            .lines()
            .filter(|line| line.trim_start().starts_with("export "))
        {
            assert!(
                !line.contains("op_"),
                "{path} public exports must not expose raw op-shaped names: {line}"
            );
        }
    }
}

#[test]
fn package_author_guide_documents_bounded_diagnostic_publication() {
    let guide = read("docs/reference/packages/creating-packages.md");
    let diagnostics_api =
        read("docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md");
    let primitives = read("docs/reference/primitives/diagnostics.md");

    assert!(guide.contains("### Phase 18.17 range diagnostics publication"));
    assert!(guide.contains("serverPublishDiagnostics"));
    assert!(guide.contains("clay:diagnostics"));
    assert!(guide.contains("DIAGNOSTIC_PAYLOAD_BUDGET_BYTES"));
    assert!(guide.contains("diagnosticError"));
    assert!(guide.contains("language-server process"));
    assert!(guide.contains("../primitives/diagnostics.md"));
    assert!(diagnostics_api.contains("clay.diagnostics.serverPublishDiagnostics"));
    assert!(primitives.contains("serverPublishDiagnostics"));

    for forbidden in [
        "Deno.core.ops.op_clay_diagnostics",
        "diagnostics.enabled",
        "squiggleWidth",
    ] {
        assert!(
            !guide.contains(forbidden),
            "package author guide must not document forbidden surface `{forbidden}`"
        );
    }
}

#[test]
fn package_loading_docs_describe_implemented_resolver_and_carried_forward_deferrals() {
    // Phase 18.6 task 8: after the docs transition, the authoritative docs must
    // describe the implemented resolver (not a gap) and the carried-forward
    // deferrals (non-@clay/*, hot reload, persistent enable state).
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &package_guide, &wiki] {
        assert!(
            source.contains("Phase 18.6 shipped") || source.contains("Phase 18.6 implemented"),
            "docs must describe the Phase 18.6 implementation"
        );
        assert!(
            source.contains("loadPackage"),
            "docs must mention the loadPackage facade"
        );
    }
    assert!(
        [&package_loading, &package_guide, &wiki]
            .iter()
            .any(|source| source.contains("source-aware")),
        "docs must document the source-aware package direction"
    );
    // Source-aware resolution is implemented; durable state remains explicitly carried forward.
    assert!(
        package_loading.contains("source-aware")
            && package_loading.contains("Durable package state")
            && (package_loading.contains("Hot-reload") || package_loading.contains("hot reload")),
        "package loading reference must document implemented source-aware loading and carried-forward durable state work"
    );
    // Old gap language must not appear.
    assert!(
        !package_loading.contains("generic one-line loader is not implemented yet"),
        "package loading reference must not claim the loader is unimplemented"
    );
    assert!(
        !package_loading.contains("generic loader/API gap"),
        "package loading reference must not describe the resolver as a gap"
    );
    assert!(
        !package_loading.contains("temporary validation/loading gap"),
        "package loading reference must not describe the resolver as a temporary gap"
    );
}

#[test]
fn package_loading_docs_describe_source_aware_package_authority_target() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_wiki = read("docs/wiki/modules/package-loading.md");
    let hardening = read("docs/wiki/modules/persistent-runtime-hardening.md");
    let resolver_op = read("src/server/ops/packages.rs");

    for phrase in [
        "npm",
        "GitHub/git",
        "local path",
        "source-aware",
        "user authorization",
        "package-manager metadata does not automatically activate runtime behavior",
    ] {
        assert!(
            package_loading.contains(phrase)
                || package_wiki.contains(phrase)
                || hardening.contains(phrase),
            "docs must document source-aware package authority phrase `{phrase}`"
        );
    }
    assert!(
        resolver_op.contains("installed, authorized package specifier")
            && resolver_op.contains("PackageService")
            && resolver_op.contains("PackageLoadEntryAllowlist"),
        "resolver must describe source-aware package loading through the shared package service path"
    );
}
#[test]
fn clay_packages_load_package_registry_entry_is_runtime_backed() {
    // Phase 18.6 task 8: the inventory entry must be promoted from planned to
    // runtime-backed with concrete paths and registry_public = true.
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let entry = inventory
        .split("[[api]]")
        .find(|block| block.contains("id = \"clay.packages.loadPackage\""))
        .expect("clay.packages.loadPackage inventory entry must exist");

    assert!(
        entry.contains("status = \"runtime-backed\""),
        "loadPackage inventory entry must be runtime-backed, got:\n{entry}"
    );
    assert!(
        entry.contains("registry_public = true"),
        "loadPackage inventory entry must be registry_public, got:\n{entry}"
    );
    // Concrete paths replace the old "planned:" placeholders.
    assert!(
        entry.contains("facade_path = \"runtime/js/packages.ts::loadPackage\""),
        "loadPackage inventory entry must have a concrete facade path"
    );
    assert!(
        entry.contains("src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier"),
        "loadPackage inventory entry must have a concrete deno_op_path"
    );
    assert!(
        entry.contains("src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier"),
        "loadPackage inventory entry must have a concrete backing_rust path"
    );
    assert!(
        entry.contains("src/server/js_runtime.rs::ClayModuleLoader"),
        "loadPackage inventory entry must reference the module-loader gate"
    );
    assert!(
        entry.contains("docs/reference/clay-js-api/packages/load-package.md"),
        "loadPackage inventory entry must point to the dedicated Markdown doc"
    );
}

#[test]
fn phase18_10_code_wiki_documents_final_syntax_implementation() {
    let wiki_index = read("docs/wiki/index.md");
    let syntax_wiki = read("docs/wiki/modules/syntax-grammar-registry.md");
    let parse_wiki = read("docs/wiki/modules/parse-coordinator.md");
    let decoration_wiki = read("docs/wiki/modules/decoration-transport.md");
    let mode_wiki = read("docs/wiki/modules/mode-registry.md");
    let package_wiki = read("docs/wiki/modules/package-loading.md");
    let review = read("docs/wiki/modules/phase18.10-tree-sitter-grammar-primitive-review.md");
    let facade_skeleton = read("docs/wiki/modules/clay-js-facade-skeleton.md");

    for link in [
        "modules/syntax-grammar-registry.md",
        "modules/parse-coordinator.md",
        "modules/decoration-transport.md",
        "modules/mode-registry.md",
        "modules/package-loading.md",
        "modules/phase18.10-tree-sitter-grammar-primitive-review.md",
        "modules/clay-js-facade-skeleton.md",
    ] {
        assert!(
            wiki_index.contains(link),
            "wiki index must link Phase 18.10 implementation page {link}"
        );
    }

    for phrase in [
        "runtime/js/syntax.ts",
        "src/server/ops/syntax.rs",
        "TreeSitterSyntaxHandler",
        "SyntaxGrammarRegistry",
        "serverRegisterSyntaxGrammar",
        "loadPackage(\"@clay/rust\")",
        "loadPackage(\"@clay/typescript\")",
        "loadPackage(\"@clay/javascript\")",
        "parse-document",
        "render-decorations",
        "first-party",
        "package-root-confined",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
        "manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow",
        "tests/fixtures/configuration/syntax-grammars/init.js",
    ] {
        assert!(
            syntax_wiki.contains(phrase),
            "syntax grammar wiki must document final implementation detail `{phrase}`"
        );
    }

    for (name, source, phrases) in [
        (
            "parse coordinator wiki",
            parse_wiki.as_str(),
            [
                "TreeSitterSyntaxHandler",
                "ParseWindowSnapshot",
                "Background",
                "stale",
                "payload-budget",
            ],
        ),
        (
            "decoration transport wiki",
            decoration_wiki.as_str(),
            [
                "Tree-sitter syntax highlighting",
                "DecorationSet",
                "DECORATION_PAYLOAD_BUDGET_BYTES",
                "SyntaxChunkCache",
                "punctuation.definition",
            ],
        ),
        (
            "mode registry wiki",
            mode_wiki.as_str(),
            [
                "Active syntax grammar is separate from active major mode",
                "core.code",
                "active_syntax_grammar",
                "behavior version",
                "manual-smoke",
            ],
        ),
        (
            "package loading wiki",
            package_wiki.as_str(),
            [
                "runtime/js/syntax.ts",
                "clay:syntax",
                "contributions.syntaxGrammars",
                "loadPackage",
                "ordinary end-user config uses one-line",
            ],
        ),
        (
            "primitive review wiki",
            review.as_str(),
            [
                "Final Implementation Status",
                "src/packages/record.rs",
                "src/server/syntax.rs",
                "runtime/js/syntax.ts",
                "tests/syntax_grammar.rs",
            ],
        ),
        (
            "facade skeleton wiki",
            facade_skeleton.as_str(),
            [
                "runtime/js/syntax.ts",
                "clay:syntax.serverRegisterSyntaxGrammar",
                "generated Clay JS API registry",
                "clay:syntax",
                "first-party grammar-only package load entries",
            ],
        ),
    ] {
        for phrase in phrases {
            assert!(
                source.contains(phrase),
                "{name} must document Phase 18.10 final wiki detail `{phrase}`"
            );
        }
    }

    assert!(
        !package_wiki.contains("clay:decorations` and `clay:parse` are importable runtime modules, but their public Phase 18 functions currently delegate to the planned-unavailable op"),
        "package-loading wiki must not retain stale planned-unavailable language for runtime-backed parse/decor/syntax facades"
    );
}

#[test]
fn range_diagnostics_add_no_hidden_configuration_surface() {
    // Phase 18.17: range diagnostics reuse setTheme + setSyntaxEnginePreference;
    // no new diagnostic toggle/geometry/severity preference API.
    let configuration = read("docs/reference/clay-js-api/configuration.md");
    let api_inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let generated_registry = read("docs/generated/clay-js-api-registry.json");
    let budgets = read("src/perf/budgets.rs");
    let diagnostics_facade = read("runtime/js/diagnostics.ts");

    for phrase in [
        "Phase 18.17 range diagnostics configuration review",
        "did **not** promote a new user-facing diagnostic toggle",
        "severity colors come from the active theme",
        "syntax-error publication follows the active syntax engine",
        "diagnosticError",
        "diagnosticWarning",
        "diagnosticInfo",
        "setTheme",
        "setSyntaxEnginePreference",
        "serverPublishDiagnostics",
        "package publication API",
        "diagnostics.enabled",
        "diagnostics.squiggleWidth",
        "Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only",
        "Ordinary keypress, paint, layout, scroll",
    ] {
        assert!(
            configuration.contains(phrase),
            "configuration docs must pin Phase 18.17 diagnostics config boundary: {phrase}"
        );
    }

    assert!(
        diagnostics_facade.contains("serverPublishDiagnostics")
            && !diagnostics_facade.contains("enableDiagnostics")
            && !diagnostics_facade.contains("setDiagnosticsPreference"),
        "diagnostics facade must expose publication only, not user preference setters"
    );

    for forbidden in [
        "clay.configuration.setDiagnostics",
        "clay.diagnostics.setEnabled",
        "clay.diagnostics.enable",
        "clay.diagnostics.setSquiggleWidth",
        "diagnostics.enabled",
        "diagnostics.squiggleWidth",
        "syntaxError.highlight",
        "treeSitter.showErrors",
    ] {
        assert!(
            !api_inventory.contains(forbidden),
            "API inventory must not promote hidden diagnostic config `{forbidden}`"
        );
        assert!(
            !generated_registry.contains(forbidden),
            "generated registry must not promote hidden diagnostic config `{forbidden}`"
        );
    }

    for budget in [
        "DIAGNOSTIC_PAYLOAD_BUDGET_BYTES",
        "DIAGNOSTIC_MAX_SPANS_PER_SET",
        "DIAGNOSTIC_CACHE_BUDGET_BYTES",
    ] {
        assert!(
            budgets.contains(budget),
            "compiled diagnostic budgets must remain server constants: {budget}"
        );
        assert!(
            configuration.contains(budget) || configuration.contains("DIAGNOSTIC_"),
            "configuration review must name compiled diagnostic budgets, not init.js keys"
        );
    }
}

#[test]
fn load_package_introduces_no_hidden_configuration_key() {
    // configuration verification: loadPackage is an explicit action, not a config key.
    let facade = read("runtime/js/packages.ts");
    let resolver_op = read("src/server/ops/packages.rs");

    // The loadPackage facade takes only a specifier; no config/options/setting key.
    assert!(
        facade.contains("specifier: string"),
        "loadPackage facade must accept only specifier, not config/options/setting"
    );
    assert!(
        !facade.contains("defaultPackages"),
        "loadPackage facade must not introduce a defaultPackages config key"
    );

    // The resolver op takes only specifier JSON; no config/preferences/settings key.
    assert!(
        !resolver_op.contains("defaultPackages"),
        "resolver op must not introduce a defaultPackages config key"
    );

    // Package customization uses documented APIs, not loadPackage config keys.
    let package_loading = read("docs/reference/primitives/package-loading.md");
    assert!(
        package_loading.contains("setPackageOption")
            || package_loading.contains("clay.configuration.setPackageOption"),
        "package-loading.md must reference setPackageOption for customization"
    );
}

#[test]
fn first_party_language_packages_add_no_hidden_configuration_surface() {
    let config_review = read("docs/reference/clay-js-api/configuration.md");

    // Verify the Phase 18.18 configuration review section exists
    assert!(
        config_review.contains("Phase 18.18 first-party language package configuration review"),
        "configuration.md must contain Phase 18.18 config review section"
    );
    assert!(
        config_review.contains("did **not** promote a new user-facing `clay:configuration` API"),
        "Phase 18.18 config review must state no new config API was promoted"
    );

    // Verify the section documents existing APIs, not hidden keys
    for phrase in [
        "clay.packages.loadPackage",
        "clay.theme.setTheme",
        "clay.syntax.setSyntaxEnginePreference",
        "clay.behavior.buildCodeEditingManifest",
        "clay.completion.serverRegisterCompletionProvider",
        "clay.configuration.setPackageOption",
    ] {
        assert!(
            config_review.contains(phrase),
            "Phase 18.18 config review must document existing API {phrase}"
        );
    }

    // Verify rejected hidden keys are listed
    for key in [
        "enableRust",
        "language.indentWidth",
        "language.pairs",
        "completion.keywords",
        "markdown.preview",
        "syntax.styleMap",
        "language.behavior",
    ] {
        assert!(
            config_review.contains(key),
            "Phase 18.18 config review must reject hidden key {key}"
        );
    }

    // Package source files must not contain hidden per-language config keys
    for (pkg, file) in [
        ("rust", "packages/rust/dist/load.js"),
        ("typescript", "packages/typescript/dist/load.js"),
        ("javascript", "packages/javascript/dist/load.js"),
        ("markdown", "packages/markdown/dist/load.js"),
    ] {
        let src = read(file);
        for key in ["setPackageOption", "setModePreference", "packageOptions"] {
            assert!(
                !src.contains(key),
                "{pkg} load.js must not introduce config surface {key}"
            );
        }
    }

    // Per-package docs/index.md must reference only existing APIs for config
    for (pkg, doc) in [
        ("rust", read("packages/rust/docs/index.md")),
        ("typescript", read("packages/typescript/docs/index.md")),
        ("javascript", read("packages/javascript/docs/index.md")),
        ("markdown", read("packages/markdown/docs/index.md")),
    ] {
        assert!(
            doc.contains("loadPackage") || doc.contains("clay:packages"),
            "{pkg}/docs/index.md must document loadPackage as the config surface"
        );
        assert!(
            doc.contains("config") || doc.contains("Configuration"),
            "{pkg}/docs/index.md must reference configuration"
        );
    }
}

#[test]
fn first_party_language_packages_ride_existing_clay_js_facades() {
    // Verify that Phase 18.18 first-party language packages use only existing
    // Clay JS facades — no new raw Deno.core.ops or package-specific API surface.
    let existing_facade_imports = [
        "clay:syntax",
        "clay:modes",
        "clay:behavior",
        "clay:commands",
        "clay:completion",
        "clay:ui",
        "clay:packages",
    ];

    for (pkg, file) in [
        ("rust", "packages/rust/dist/load.js"),
        ("typescript", "packages/typescript/dist/load.js"),
        ("javascript", "packages/javascript/dist/load.js"),
        ("markdown", "packages/markdown/dist/load.js"),
    ] {
        let src = read(file);
        // Each package load.js must use at least one existing facade import
        let found = existing_facade_imports.iter().any(|f| src.contains(f));
        assert!(
            found,
            "{pkg} load.js must import at least one existing clay: facade"
        );
        // No raw Deno.core.ops calls
        assert!(
            !src.contains("Deno.core.ops"),
            "{pkg} load.js must not call raw Deno.core.ops"
        );
        assert!(
            !src.contains("Deno.core.opAsync"),
            "{pkg} load.js must not call raw Deno.core.opAsync"
        );
    }

    // Per-package docs index.md must name the facades used
    for (pkg, doc) in [
        ("rust", read("packages/rust/docs/index.md")),
        ("typescript", read("packages/typescript/docs/index.md")),
        ("javascript", read("packages/javascript/docs/index.md")),
        ("markdown", read("packages/markdown/docs/index.md")),
    ] {
        assert!(
            doc.contains("loadPackage") || doc.contains("clay:packages"),
            "{pkg}/docs/index.md must reference loadPackage"
        );
        assert!(
            doc.contains("clay:"),
            "{pkg}/docs/index.md must reference at least one clay: facade"
        );
    }
}

#[test]
fn first_party_language_package_docs_and_registry_are_fresh() {
    // The generated Clay JS API registry must match current Markdown docs.
    // The per-package Markdown reference docs and per-package docs/index.md
    // pages must be linked from the master index.
    let registry_text = read("docs/generated/clay-js-api-registry.json");
    let master_index = read("docs/index.md");

    // Registry must contain entries for all public APIs used by first-party packages
    for api_id in [
        "clay.syntax.serverRegisterSyntaxGrammar",
        "clay.modes.serverRegisterModePattern",
        "clay.behavior.buildCodeEditingManifest",
        "clay.commands.serverRegisterCommand",
        "clay.completion.serverRegisterCompletionProvider",
        "clay.ui.serverRegisterComponentContribution",
        "clay.packages.loadPackage",
    ] {
        assert!(
            registry_text.contains(api_id),
            "generated registry must contain {api_id}"
        );
    }

    // Master index must link all four per-package reference docs
    for (pkg, ref_doc) in [
        ("rust", "reference/packages/rust.md"),
        ("typescript", "reference/packages/typescript.md"),
        ("javascript", "reference/packages/javascript.md"),
        ("markdown", "reference/packages/markdown.md"),
    ] {
        assert!(
            master_index.contains(ref_doc),
            "docs/index.md must link {ref_doc}"
        );
        let ref_content = read(&format!("docs/{ref_doc}"));
        assert!(
            !ref_content.is_empty(),
            "{ref_doc} must exist and be non-empty"
        );
        assert!(
            ref_content.contains(pkg),
            "{ref_doc} must reference package name {pkg}"
        );
    }

    // API inventory must list the public APIs
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    for api_id in [
        "clay.syntax.serverRegisterSyntaxGrammar",
        "clay.completion.serverRegisterCompletionProvider",
        "clay.behavior.buildCodeEditingManifest",
        "clay.packages.loadPackage",
    ] {
        assert!(
            inventory.contains(api_id),
            "api-inventory.toml must contain {api_id}"
        );
    }
}

#[test]
fn package_ui_layout_authoring_contract_is_unified_across_package_sources() {
    // Plan 035 task 9: the package UI/layout authoring contract must apply
    // identically to @clay/* packages and user-installed packages, with native
    // UI / client runtime as explicit capability/API work, not implicit through
    // package source.
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_security = read("docs/reference/primitives/package-security.md");

    // Functional: the guide states user-installed packages may request the same
    // UI/layout/native/client capabilities as Clay packages, with grants and
    // validation.
    for phrase in [
        "Unified UI/layout authoring contract across package sources",
        "identical for `@clay/*` packages and user-installed packages",
        "`@clay/*` only means a package was shipped by Clay",
        "same `clay:ui` facades",
        "same `PackageService` validation",
        "same conflict-resolution policy",
        "User-installed packages may request the same UI/layout/native/client capabilities",
        "explicit user authorization grants",
        "Unified Package Capability Model",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package guide must document the unified UI/layout authoring contract: {phrase}"
        );
    }

    // Security: native UI / client runtime is explicit capability/API work,
    // not implicit through package source.
    for phrase in [
        "Native UI and Client Runtime Are Explicit Capability/API Work",
        "never implicit through package source",
        "explicit capability and API work",
        "native-ui",
        "client-runtime",
        "granted only through an explicit user/admin authorization record",
        "never inferred from the package source kind",
        "A capability grant authorizes a package to use a surface",
        "does not materialize the surface",
        "No UI/layout/security primitive branches on package source",
    ] {
        assert!(
            package_security.contains(phrase),
            "package-security.md must document native-ui/client-runtime as explicit capability/API work: {phrase}"
        );
    }
    // The guide must echo the native-ui/client-runtime explicit-capability rule.
    assert!(
        package_guide.contains("Native UI and client runtime are explicit capability/API work"),
        "package guide must state native UI/client runtime is explicit capability/API work"
    );

    // Performance: UI/layout declarations remain validated load/reload/config
    // work, not paint/layout hot-path JS.
    assert!(
        package_guide.contains("validated load/reload/configuration work")
            && package_guide.contains("no package JavaScript runs in Masonry paint"),
        "package guide must keep UI/layout declarations off paint/layout hot paths"
    );

    // Code quality: no UI/layout primitive branches on package source.
    assert!(
        package_guide.contains("no `if github_package` / `if npm_package`"),
        "package guide must forbid source-specific UI/layout primitive branches"
    );
}

#[test]
fn phase20_multi_document_and_recovery_package_ui_contract_is_documented() {
    // Plan 055: Phase 20 multi-document sessions, dirty/save status, and
    // recovery chrome are Clay-owned. Package authors must see explicit
    // non-goals so they do not invent tabs, native save dialogs, or
    // reconnect loops as package UI.
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let slot_ui_wiki = read("docs/wiki/modules/slot-aware-package-ui.md");
    let phase20_review =
        read("docs/wiki/modules/phase20-daily-editing-product-hardening-primitive-review.md");

    for phrase in [
        "Phase 20 authoring contract: multi-document sessions, dirty/save status, and recovery chrome",
        "Clay-owned",
        "DocumentSessionStore",
        "clientShowOpenDocuments",
        "clientRequestResync",
        "clientDismissRecovery",
        "serverListDocuments",
        "Dirty/save chrome",
        "TransientMenuSession",
        "package paint-path requirements",
        "clipboard-contents APIs",
        "arbitrary file writes",
        "direct native file/save dialogs",
        "Broader package/config/AI authority over those surfaces remains deferred",
        "it does not own tabs, native save dialogs, or reconnect/resync loops",
    ] {
        assert!(
            package_guide.contains(phrase),
            "creating-packages.md must document the Phase 20 multi-document/recovery package UI contract with `{phrase}`"
        );
    }

    assert!(
        slot_ui_wiki.contains("Phase 20 multi-document / dirty-save / recovery chrome"),
        "slot-aware-package-ui wiki must cross-link the Phase 20 package UI contract"
    );
    assert!(
        phase20_review.contains("Phase 20 multi-document / dirty-save / recovery chrome contract"),
        "Phase 20 primitive review must link the package authoring contract"
    );
}

#[test]
fn phase18_11_completion_provider_authoring_contract_documented_in_package_guide() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_security = read("docs/reference/primitives/package-security.md");
    let registry = read("docs/reference/primitives/registry.md");
    let api_reference =
        read("docs/reference/clay-js-api/completion/server-register-completion-provider.md");

    // Functional: package authors learn the registration API, trigger metadata,
    // result item/commit-character shapes, and transient menu reuse.
    for phrase in [
        "Phase 18.11 authoring contract: completion providers",
        "serverRegisterCompletionProvider",
        "clay.contributions.completionProviders",
        "completion-provider",
        "triggerCharacters",
        "wordBoundaryChars",
        "commitCharacters",
        "TransientMenuSession",
        "core.bufferWords",
        "completion.trigger",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package guide must document completion provider authoring contract: {phrase}"
        );
    }

    // End-user loading is explicit one-line loadPackage, not raw ops or copied
    // manifests, and no provider package auto-loads silently.
    assert!(
        package_guide.contains("await loadPackage(\"@vendor/words\")"),
        "package guide must show explicit one-line loadPackage for completion providers"
    );
    assert!(
        package_guide.contains("no provider package auto-loads silently"),
        "package guide must forbid silent completion provider auto-loading"
    );

    // Performance: provider work is UI-reactive/cancellable and off hot paths.
    for phrase in [
        "UiReactivePriority",
        "cancellable",
        "edits locally first",
        "bounded non-blocking channel",
        "keypress-to-local-paint, paint, layout, scroll, pointer, or text-event hot paths",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package guide must document completion hot-path/performance boundary: {phrase}"
        );
    }

    // Security: providers need completion-provider, read only Clay-provided
    // open-document content/windows, and gain no extra authority.
    for phrase in [
        "raw callbacks",
        "raw ops",
        "client-side JavaScript",
        "no filesystem/network/shell/AI/raw-op/native-UI/client-runtime authority",
        "metadata-only",
        "inert text-replacement data",
    ] {
        assert!(
            package_guide.contains(phrase),
            "package guide must document completion security boundary: {phrase}"
        );
    }

    // Code quality: examples use the clay:completion facade and loadPackage,
    // not raw ops or a completion-specific widget tree.
    assert!(
        package_guide
            .contains("import { serverRegisterCompletionProvider } from \"clay:completion\""),
        "package guide must use the clay:completion facade, not raw ops"
    );
    assert!(
        package_guide.contains("do not add a completion-specific Masonry widget tree"),
        "package guide must forbid completion-specific widget scaffolding"
    );

    // Primitive security and registry references stay current.
    assert!(
        package_security.contains("CompletionTriggerAndResult")
            && package_security.contains("metadata-only"),
        "package-security.md must document the metadata-only completion boundary"
    );
    assert!(
        registry.contains("CompletionTriggerAndResult")
            && registry.contains("TransientMenuSession"),
        "primitive registry must keep the completion primitive and transient menu reuse"
    );
    assert!(
        api_reference.contains("clay.completion.serverRegisterCompletionProvider"),
        "completion provider API reference must be linked from the authoring contract"
    );
}

#[test]
fn first_party_language_package_docs_are_indexed_and_complete() {
    let rust_doc = read("packages/rust/docs/index.md");
    let typescript_doc = read("packages/typescript/docs/index.md");
    let javascript_doc = read("packages/javascript/docs/index.md");
    let markdown_doc = read("packages/markdown/docs/index.md");
    let docs_index = read("docs/index.md");

    // Verify all four per-package docs/index.md files exist and contain
    // the required Phase 18.18 full-mode contract elements
    for (name, doc) in [
        ("rust", rust_doc),
        ("typescript", typescript_doc),
        ("javascript", javascript_doc),
        ("markdown", markdown_doc),
    ] {
        assert!(
            doc.contains("Tier 1 native"),
            "{name}/docs/index.md must document Tier 1 native grammar"
        );
        assert!(
            doc.contains("vocabulary styleMap") || doc.contains("styleMap"),
            "{name}/docs/index.md must document vocabulary styleMap"
        );
        assert!(
            doc.contains("Behavior manifest") || doc.contains("behavior manifest"),
            "{name}/docs/index.md must document behavior manifest"
        );
        assert!(
            doc.contains("Completion provider") || doc.contains("completion provider"),
            "{name}/docs/index.md must document completion provider"
        );
        assert!(
            doc.contains("per-language Rust branches") || doc.contains("per-language Rust"),
            "{name}/docs/index.md must state no per-language Rust branches"
        );
    }

    // Verify docs/index.md links per-package pages with correct full-mode descriptions
    assert!(
        docs_index.contains("[@clay/rust Package](reference/packages/rust.md)")
            && docs_index.contains("first-party Rust full-mode package"),
        "docs/index.md must link rust package with full-mode description"
    );
    assert!(
        docs_index.contains("[@clay/typescript Package](reference/packages/typescript.md)")
            && docs_index.contains("first-party TypeScript full-mode package"),
        "docs/index.md must link typescript package with full-mode description"
    );
    assert!(
        docs_index.contains("[@clay/javascript Package](reference/packages/javascript.md)")
            && docs_index.contains("first-party JavaScript full-mode package"),
        "docs/index.md must link javascript package with full-mode description"
    );
    assert!(
        docs_index.contains("[@clay/markdown Package](reference/packages/markdown.md)")
            && docs_index.contains("first-party Markdown full-mode package"),
        "docs/index.md must link markdown package with full-mode description"
    );

    // Verify docs/index.md does NOT contain stale "grammar-only" descriptions
    assert!(
        !docs_index.contains("grammar-only"),
        "docs/index.md must not contain stale 'grammar-only' descriptions for first-party packages"
    );
}

#[test]
fn phase18_20_language_server_grant_boundary_is_documented_and_pinned() {
    let permissions = read("src/packages/permissions.rs");
    let record = read("src/packages/record.rs");
    let service = read("src/packages/service.rs");
    let op = read("src/server/ops/language_server.rs");
    let op_state = read("src/server/ops/mod.rs");
    let package_op = read("src/server/ops/packages.rs");
    let facade = read("runtime/js/language-server.ts");
    let security = read("docs/reference/primitives/package-security.md");
    let authoring = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    assert!(permissions.contains("LanguageServer") && permissions.contains("language-server"));
    for phrase in [
        "LanguageServerContributionDescriptor",
        "inherit_environment",
        "language_servers",
    ] {
        assert!(record.contains(phrase), "record must retain `{phrase}`");
    }
    for phrase in [
        "authorize_language_server",
        "authorize_bundled_defaults",
        "MissingLanguageServerGrant",
        "revoke_language_server_grants",
    ] {
        assert!(service.contains(phrase), "service must enforce `{phrase}`");
    }
    assert!(op_state.contains("configuration_evaluation"));
    for phrase in [
        "authorization_sealed",
        "unknown_workspace_root",
        "resolve_language_server_executable",
    ] {
        assert!(
            op.contains(phrase),
            "authorization op must enforce `{phrase}`"
        );
    }
    assert!(package_op.contains("seal_language_server_authority"));
    assert!(facade.contains("export async function authorizeLanguageServer"));
    for phrase in [
        "Language-Server Authority Boundary",
        "bundled `NativeTrust` defaults exclude `language-server`",
        "starts no process",
        "not an OS filesystem/network/process sandbox",
    ] {
        assert!(
            security.contains(phrase) || authoring.contains(phrase) || wiki.contains(phrase),
            "language-server docs/wiki must preserve `{phrase}`"
        );
    }
}

#[test]
fn package_author_guide_documents_explicit_language_server_authority() {
    let authoring = read("docs/reference/packages/creating-packages.md");
    let security = read("docs/reference/primitives/package-security.md");
    let contract = read("docs/reference/primitives/language-intelligence.md");

    for phrase in [
        "## Phase 18.20 authoring contract: analyzer providers and language-server bridges",
        "serverRegisterLanguageIntelligenceProvider",
        "grant then load",
        "authorizeLanguageServer",
        "loadPackage",
        "LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES",
        "LANGUAGE_SERVER_MAX_SESSIONS",
        "LANGUAGE_SERVER_STDERR_BUDGET_BYTES",
        "fixed",
        "Content-Length",
        "trusted subprocess authority",
        "not a sandbox",
        "SemanticTokens",
        "DefinitionLink",
        "WorkspaceEdit",
        "SignatureHelp",
    ] {
        assert!(
            authoring.contains(phrase) || security.contains(phrase) || contract.contains(phrase),
            "package author/language-server docs must preserve `{phrase}`"
        );
    }

    assert!(
        authoring.contains("Grant-before-load is mandatory"),
        "package author guide must require grant-before-load"
    );
    assert!(
        authoring.contains("workspace root") || authoring.contains("workspaceRootIds"),
        "package author guide must document workspace scope"
    );
    assert!(
        contract.contains("containment") || authoring.contains("containment"),
        "docs must disclose containment semantics"
    );
}

#[test]
fn language_server_configuration_api_is_documented_and_sealed() {
    let api_page = read("docs/reference/clay-js-api/language-server/authorize-language-server.md");
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let index = read("docs/index.md");
    let configuration = read("docs/reference/clay-js-api/configuration.md");

    // API page contains all required frontmatter and sections.
    for marker in [
        "id: clay.language-server.authorizeLanguageServer",
        "js_module: \"clay:language-server\"",
        "js_export: authorizeLanguageServer",
        "op_clay_language_server_authorize",
        "user_facing_name: Authorize Language Server",
        "permissions: [\"language-server\"]",
        "key_bindings: []",
        "custom_properties:",
        "package",
        "contribution",
        "workspaceRootIds",
        "hot_path_policy:",
        "configuration root evaluation only",
        "authorization_sealed",
        "executable_not_found",
        "unknown_workspace_root",
        "duplicate_grant",
        "before loadPackage seals authority",
        "starts no process at grant time",
        "cannot self-grant",
        "lookup_tags:",
        "configuration",
        "deny-by-default",
        "phase18.20",
        "app_visible: true",
        "help_visible: true",
    ] {
        assert!(
            api_page.contains(marker),
            "authorizeLanguageServer API page must document `{marker}`"
        );
    }

    // Inventory entry contains required metadata.
    assert!(inventory.contains("clay.language-server.authorizeLanguageServer"));
    for field in [
        "category = \"language-server\"",
        "status = \"runtime-backed\"",
        "authority = \"configuration-only-grant-before-seal\"",
        "hot_path_policy",
        "sealed before first loadPackage",
        "documentation_path",
        "authorize-language-server.md",
        "custom_properties",
        "permissions = [\"language-server\"]",
        "Deny-by-default",
        "registry_public = true",
    ] {
        assert!(
            inventory.contains(field),
            "authorizeLanguageServer inventory entry must include `{field}`"
        );
    }

    // Index links the page.
    assert!(
        index.contains("authorize-language-server.md"),
        "docs/index.md must link authorizeLanguageServer API page"
    );

    // No hidden env/config key exists.
    let forbidden_env_keys = [
        "CLAY_LANGUAGE_SERVER",
        "CLAY_LSP",
        "languageServerEnabled",
        "languageServerDisable",
        "autoAuthorizeLanguageServer",
        "languageServerDefaultGrant",
    ];
    for key in forbidden_env_keys {
        assert!(
            !api_page.contains(key) && !configuration.contains(key) && !inventory.contains(key),
            "no hidden language-server env/config key `{key}` may exist"
        );
    }

    // The page explicitly rejects hidden keys.
    assert!(api_page.contains("Never expose hidden env vars"));
    assert!(api_page.contains("JSON/TOML keys"));

    // Configuration-only seal.
    for phrase in ["configuration-only", "seals authority", "cannot self-grant"] {
        assert!(
            api_page.contains(phrase) || configuration.contains(phrase),
            "configuration docs must document `{phrase}`"
        );
    }
}
