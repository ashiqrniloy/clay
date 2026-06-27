use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
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
        sandbox_design
            .contains("This document defines the separate-process JavaScript runtime sandbox gate"),
        "sandbox design must exist and state its scope"
    );

    for phrase in [
        "first-party `@clay/*`",
        "Non-`@clay/*` package execution is blocked by default",
        "separate-process JavaScript runtime sandbox",
        "V8 heap limits",
        "approved decision log",
        "No approved decision log means no non-`@clay/*` runtime execution",
        "filesystem",
        "network",
        "shell",
        "WASM",
        "AI mutation",
        "raw-op",
        "native-widget",
        "client-side JavaScript",
        "package-manager execution",
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
        "filesystem outside parent-provided open document data",
        "network",
        "shell",
        "WASM",
        "AI mutation",
        "package-manager execution",
        "native-widget handles",
        "client-side JavaScript",
        "raw-op / raw `Deno.core.ops` public authority",
        "keypress, paint, layout, scroll, text-event, or edit-ack handlers",
        "approved decision log",
        "Non-`@clay/*` package execution is not allowed by this design alone",
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
fn third_party_runtime_authority_policy_is_documented() {
    let policy = read("docs/wiki/modules/third-party-runtime-authority.md");
    let wiki_index = read("docs/wiki/index.md");

    assert!(
        wiki_index.contains("modules/third-party-runtime-authority.md"),
        "wiki index must link the third-party runtime authority page"
    );

    for phrase in [
        "install != enable != load != runtime execution != package-manager execution != client behavior delivery",
        "PackageService::install",
        "PackageService::enable",
        "op_clay_packages_load_package_by_specifier",
        "FirstPartyLoadEntryAllowlist",
        "RuntimeSandboxSupervisor",
        "Non-`@clay/*` package execution stays deny-by-default",
        "package-manager installation/metadata records do not grant runtime-execution authority",
        "left-pad",
        "@scope/pkg",
        "URLs, local path, traversal",
        "Trust and identity",
        "Registry and integrity",
        "Permission model",
        "Production sandbox",
        "Rollback and incident response",
        "Executable gates",
        "keypress, paint, layout, scroll, text-event",
        "No approved decision log means no non-`@clay/*` runtime execution",
        "filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, and workspace mutation remain denied",
    ] {
        assert!(
            policy.contains(phrase),
            "third-party runtime authority policy must document `{phrase}`"
        );
    }
}

#[test]
fn third_party_trust_identity_policy_is_documented() {
    let security = read("docs/reference/primitives/package-security.md");
    let authority = read("docs/wiki/modules/third-party-runtime-authority.md");

    for phrase in [
        "Third-Party Trust and Identity Policy",
        "Non-`@clay/*` packages are untrusted by default",
        "Clay metadata in `package.json` proves only that a package claims a Clay contract",
        "trusted_package",
        "name = \"@vendor/example\"",
        "version = \"1.2.3\"",
        "registry = \"https://registry.npmjs.org/\"",
        "integrity = \"sha512-...\"",
        "clay_prefix = \"example\"",
        "source_kind = \"npm-registry\"",
        "publisher = \"vendor\"",
        "clay_api_compatibility = \"^0.1\"",
        "Accepted source kinds are `npm-registry` first",
        "namespace hijacks, typosquats, unsigned or untrusted sources",
        "Trust records grant identity only",
        "PackageRecord`, `PackageService`, and conflict checks already carry package name, version, `apiPrefix`",
        "do not yet store trusted third-party source records, publisher identity, registry provenance, integrity evidence",
        "keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths",
    ] {
        assert!(
            security.contains(phrase),
            "package security docs must document trust identity phrase `{phrase}`"
        );
    }

    for phrase in [
        "Trust and Identity Policy",
        "Non-`@clay/*` packages require an explicit trust record",
        "Package `name`, resolved `version`, `registry` or source location, package-manager `integrity`, `clay_prefix`, `source_kind`, `publisher`/owner, and `clay_api_compatibility` are the identity tuple",
        "Accepted source kinds start at `npm-registry`",
        "Bare names, custom scopes, URLs, local paths, tarballs, git sources, aliases, registry redirects, ambiguous local paths, unknown publishers, namespace hijacks, typosquats, incompatible Clay API ranges, missing signatures/provenance, conflicting prefixes, and conflicting contribution IDs fail closed",
        "Existing `PackageRecord`, `PackageService`, and conflict primitives carry package name/version/prefix/contribution provenance",
        "Trust records grant identity only",
        "approved decision log",
    ] {
        assert!(
            authority.contains(phrase),
            "authority wiki must document trust identity phrase `{phrase}`"
        );
    }
}

#[test]
fn third_party_registry_integrity_policy_is_documented() {
    let reference = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let manager = read("src/packages/manager.rs");

    for phrase in [
        "Registry and Integrity Verification Policy",
        "Clay delegates registry access, package fetching, dependency resolution, version ranges, lockfile writing, integrity verification, caching, and offline store behavior to the npm-compatible package manager",
        "Clay does not implement a registry client",
        "requested_spec = \"@vendor/example@1.2.3\"",
        "resolved_version = \"1.2.3\"",
        "integrity = \"sha512-...\"",
        "lockfile = \"pnpm-lock.yaml\"",
        "tarball = \"https://registry.npmjs.org/@vendor/example/-/example-1.2.3.tgz\"",
        "offline_cache_key = \"@vendor/example/1.2.3\"",
        "pnpm add --ignore-scripts <pkg>@<version>",
        "Package-manager stdout, stderr, exit code, `package.json`, lockfile text, and registry metadata are diagnostic/provenance inputs only",
        "Diagnostics copied from package-manager output must be sanitized",
        "Offline/cache installs are allowed only when cached metadata still matches the trusted resolved version and integrity digest",
        "Updates are treated as new identities",
        "never runs from keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths",
        "do not yet persist a source/integrity provenance record, parse lockfile integrity evidence, sanitize package-manager diagnostics, model offline/cache keys, or enforce update-as-new-identity checks",
    ] {
        assert!(
            reference.contains(phrase),
            "package loading reference must document registry/integrity phrase `{phrase}`"
        );
    }

    for phrase in [
        "Clay delegates registry access, resolution, lockfile writing, integrity verification, caching, and offline store behavior to pnpm/npm-compatible tooling instead of implementing a registry client",
        "requested spec, resolved version, registry/source URL, lockfile path, integrity digest, tarball or source path, package root, and offline/cache key",
        "Package-manager stdout, stderr, exit code, `package.json`, lockfile text, and registry metadata are diagnostic/provenance inputs only",
        "Diagnostics copied from package-manager output must be sanitized",
        "Offline/cache hits and updates do not widen authority",
        "generic provenance storage, lockfile integrity parsing, diagnostic sanitization, offline/cache key modeling, and update-as-new-identity enforcement",
    ] {
        assert!(
            wiki.contains(phrase),
            "package loading wiki must document registry/integrity phrase `{phrase}`"
        );
    }

    for phrase in [
        "--ignore-scripts",
        "captured stdout/stderr",
        "exit code",
        "lockfile management",
        "integrity verification",
    ] {
        assert!(
            manager.contains(phrase),
            "package manager boundary must keep documented primitive `{phrase}`"
        );
    }
}

#[test]
fn third_party_permission_model_and_denied_authorities_are_documented() {
    let security = read("docs/reference/primitives/package-security.md");
    let authority = read("docs/wiki/modules/third-party-runtime-authority.md");
    let permissions = read("src/packages/permissions.rs");

    for phrase in [
        "Third-Party Permission Model",
        "\"permissions\": [\"mode-registration\", \"parse-document\"]",
        "mode-registration",
        "mode-activation",
        "command-registration",
        "package-configuration",
        "parse-document",
        "render-decorations",
        "render-folding",
        "completion-provider",
        "Grant source is an explicit user/admin/decision-approved trust+permission record matched to package name, version, source, integrity, and `apiPrefix`",
        "Runtime enforcement happens in the parent at load, registration, configuration, parse/completion/decorations request, and output-publication boundaries",
        "Broad or catch-all permission names are prohibited",
        "trusted-third-party",
        "all",
        "admin",
        "Denied authorities stay denied for third-party packages unless a later approved decision grants one narrow capability with docs and tests: filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, workspace mutation",
        "Permission checks are install/enable/load/reload/registration/request/publication work only",
        "generic persisted grant source, trust-record match, parent-side sandbox request enforcement",
    ] {
        assert!(
            security.contains(phrase),
            "package security docs must document permission phrase `{phrase}`"
        );
    }

    for phrase in [
        "Third-Party Permission Model and Denied Authorities",
        "Allowed initial permission strings reuse the existing package permission primitive",
        "enable/load validates requested permissions and rejects unknown/prohibited strings",
        "parent revalidates bounded inert outputs before publishing behavior manifests, SDUI, decorations, folding, completion, or parse updates",
        "Broad/catch-all permissions are rejected",
        "trusted-third-party",
        "raw-deno-ops",
        "Denied authorities remain denied unless a later approved decision grants one narrow capability with tests: filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget, client-JS, raw-op, remote listener, workspace mutation",
        "`src/packages/permissions.rs::parse_permission` accepts only known permission strings and returns `ProhibitedAuthority` for blocked host capabilities",
        "approved decision log grants exact authority",
        "never keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot-path work",
    ] {
        assert!(
            authority.contains(phrase),
            "authority wiki must document permission phrase `{phrase}`"
        );
    }

    for phrase in [
        "trusted-third-party",
        "all",
        "admin",
        "system",
        "host",
        "runtime",
    ] {
        assert!(
            !permissions.contains(&format!("\"{phrase}\" => Ok")),
            "broad permission `{phrase}` must not become an accepted package permission"
        );
    }

    for phrase in [
        "filesystem",
        "network",
        "shell",
        "ai-mutation",
        "remote-listener",
        "wasm-execution",
        "raw-deno-ops",
        "native-widget",
        "client-javascript",
        "package-installation",
        "package-enable-disable",
        "workspace-mutation",
    ] {
        assert!(
            permissions.contains(phrase),
            "prohibited authority `{phrase}` must stay rejected in permissions parser"
        );
    }
}

#[test]
fn third_party_sandbox_enforcement_policy_is_documented() {
    let design = read("docs/design/persistent-runtime-sandbox.md");
    let authority = read("docs/wiki/modules/third-party-runtime-authority.md");
    let harness = read("src/server/runtime_sandbox.rs");
    let codec = read("src/protocol/codec.rs");

    for phrase in [
        "Third-Party Production Enforcement Contract",
        "newline-delimited JSON harness is evidence only, not production API",
        "length-prefixed frames, maximum frame size, typed request/response variants, decode validation, generation IDs, stable error codes",
        "parent validates trust + registry integrity + package metadata + permissions + budgets",
        "child evaluates/load/parse request for one runtime generation",
        "parent validates bounded inert outputs",
        "LoadThirdPartyPackage",
        "EvaluateThirdPartyModule",
        "ParseWithThirdPartyHandler",
        "trust record match, registry/source integrity match, manifest validation, permission grant match, entry path confinement, payload budget, timeout/heap budget, runtime generation, handler token, document version, and stale-generation rejection",
        "behavior/SDUI/decorations/folding/completion/parse validators",
        "Timeout, heap-limit, malformed response, oversized output, protocol mismatch, unknown variant, stale generation, stale handler token, or invalid output kills the child process",
        "last validated client state",
        "workspace roots, absolute source paths, file descriptors, package-manager handles, raw op names, V8 handles, Rust internals, capability tokens, client connection handles",
        "startup plus handshake target under 250 ms",
        "small parse request round trip target under 10 ms added overhead",
        "timeout kill plus fresh handshake target under 500 ms",
        "no keypress, paint, layout, scroll, text-event, or edit-ack dependency",
    ] {
        assert!(
            design.contains(phrase),
            "sandbox design must document enforcement phrase `{phrase}`"
        );
    }

    for phrase in [
        "Sandbox Enforcement and Parent Validation",
        "It is not production API and does not grant third-party authority",
        "bounded typed protocol like the main IPC `Codec`",
        "parent validates package metadata + permissions -> child evaluates -> parent validates inert outputs -> publish",
        "Parent pre-validates every load/evaluate/parse request",
        "Parent post-validates every response",
        "Timeout, heap-limit, malformed response, oversized output, protocol mismatch, unknown variant, stale generation, stale handler token, or invalid output kills the child",
        "The child receives no workspace roots, absolute source paths, file descriptors, package-manager handles, raw op names, V8 handles, Rust internals, capability tokens",
        "Production routing needs measured evidence first",
    ] {
        assert!(
            authority.contains(phrase),
            "authority wiki must document sandbox enforcement phrase `{phrase}`"
        );
    }

    for phrase in [
        "RuntimeSandboxSupervisor",
        "max_payload_bytes",
        "Timeout",
        "kill",
        "handshake",
    ] {
        assert!(
            harness.contains(phrase),
            "sandbox harness must keep evidence primitive `{phrase}`"
        );
    }

    for phrase in [
        "LENGTH_PREFIX_BYTES",
        "DEFAULT_MAX_FRAME_SIZE",
        "FrameTooLarge",
        "decode_frame",
    ] {
        assert!(
            codec.contains(phrase),
            "main codec must keep bounded typed protocol primitive `{phrase}`"
        );
    }
}

#[test]
fn third_party_rollback_disable_update_incident_policy_is_documented() {
    let reference = read("docs/reference/primitives/package-loading.md");
    let authority = read("docs/wiki/modules/third-party-runtime-authority.md");
    let hot_reload = read("docs/wiki/modules/persistent-runtime-hot-reload.md");
    let parse_lifecycle = read("docs/wiki/modules/parse-task-lifecycle.md");
    let parse_coordinator = read("src/server/parse_coordinator.rs");

    for phrase in [
        "Third-Party Disable, Update, Rollback, and Incident Policy",
        "Third-party disable is an active withdrawal, not only a future-load block",
        "marks the package generation revoked",
        "removes PackageService-enabled state for that package identity",
        "cancels parse work for the revoked generation",
        "withdraws commands, behavior manifests, SDUI/status trees, package UI/input/state/layout/theme declarations, decorations, folding, completion providers, and diagnostics owned by that package",
        "Updates are new package identities",
        "changed version, registry/source, tarball/path, integrity digest, `apiPrefix`, publisher, permission set, or Clay compatibility range requires a new trust+permission grant",
        "keeps the prior validated generation active and reports sanitized diagnostics; it does not partially merge new contributions",
        "build and validate the candidate generation off to the side, swap only after success",
        "stale generation outputs are rejected by runtime generation ID, document version, behavior version, handler token, and package provenance",
        "revoke the package identity, stop scheduling new package work, kill or replace the sandbox child for that generation",
        "Package-manager side effects are not runtime rollback authority",
        "never blocks keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths",
        "generic package-generation revocation, contribution ownership indexes, package-scoped withdrawal, sandbox-child replacement wiring, update-as-new-identity enforcement, and sanitized incident diagnostics",
    ] {
        assert!(
            reference.contains(phrase),
            "package loading reference must document rollback phrase `{phrase}`"
        );
    }

    for phrase in [
        "Rollback, Disable, Update, and Incident Response",
        "Disable is active withdrawal",
        "Updates are new identities",
        "Failed third-party generation -> keep prior validated manifest/UI -> cancel generation parse -> require explicit reload/update",
        "Stale output rejection is mandatory",
        "runtime generation ID, document version, behavior version, handler token, package identity, or provenance no longer matches active state",
        "Incident response is fail-closed",
        "Package-manager side effects do not imply active runtime state",
        "Current reusable primitives: `PackageService::enable` removes conflict candidates, Phase 19 reload keeps the previous runtime on failed evaluation, `RuntimeGenerationStore` makes swaps generation-based, `ParseCoordinator::cancel_generation` cancels old work",
        "never block keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths",
    ] {
        assert!(
            authority.contains(phrase),
            "authority wiki must document rollback phrase `{phrase}`"
        );
    }

    for phrase in [
        "Failed reloads keep the previous generation active",
        "Parse results publish only if document version and runtime generation still match active state",
    ] {
        assert!(
            hot_reload.contains(phrase),
            "hot reload wiki must keep rollback primitive phrase `{phrase}`"
        );
    }

    for phrase in [
        "Newer runtime generations replace handler tokens and cancel old-generation in-flight tasks",
        "Stale parse results are discarded before client publication",
    ] {
        assert!(
            parse_lifecycle.contains(phrase),
            "parse lifecycle wiki must keep stale-generation phrase `{phrase}`"
        );
    }

    for phrase in [
        "cancel_generation",
        "StaleRuntimeGeneration",
        "stale_results_rejected",
    ] {
        assert!(
            parse_coordinator.contains(phrase),
            "parse coordinator must keep rollback primitive `{phrase}`"
        );
    }
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
        "deny-by-default module resolution",
        "resolver-validated first-party `@clay/*` loading only",
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
        "atomic reload swap",
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
            "first-party `loadEntry` allowlist",
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
        "deny-by-default",
        "resolver-validated first-party `@clay/*`",
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
    // implemented resolver, the deny-by-default boundary, and the carried-
    // forward deferrals (non-@clay/*, hot reload, persistent enable state).
    // The old gap phrases are no longer present.
    if one_line_loader_is_implemented {
        // Authoritative reference docs must describe the resolver mechanics.
        for source in [&package_guide, &package_loading] {
            for phrase in [
                "Phase 18.6 shipped",
                "deny-by-default",
                "FirstPartyLoadEntryAllowlist",
                "PackageService",
                "loadEntry",
                "runtime-backed",
            ] {
                assert!(
                    source.contains(phrase),
                    "package docs must describe the implemented resolver with phrase `{phrase}`"
                );
            }
        }
        // The implementation wiki summarizes the resolver at a higher level.
        assert!(
            wiki.contains("Phase 18.6 implemented")
                && wiki.contains("loadPackage")
                && wiki.contains("deny-by-default"),
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
            package_loading.contains("constrained first-party")
                && (package_loading.contains("Non-`@clay/*`")
                    || package_loading.contains("non-`@clay/*`")
                    || package_loading.contains("non-@clay/*"))
                && (package_loading.contains("Hot-reload")
                    || package_loading.contains("hot reload"))
                && (package_loading.contains("Persistent shared enable state")
                    || package_loading.contains("persistent shared enable state")),
            "package loading reference must document the carried-forward deferrals"
        );
        assert!(
            package_loading.contains("serverLoadPackage")
                && package_loading.contains("remains a lower-level validation helper"),
            "package loading reference must reframe serverLoadPackage as a helper, not a gap"
        );
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

    // The deferral is decision-log-backed.
    for source in [&package_loading, &wiki, &package_guide, &markdown_docs] {
        assert!(
            source.contains("2026-06-15-1015-defer-generic-loadpackage-first-party-resolver"),
            "docs must reference the loadPackage deferral decision log"
        );
    }
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
        "FirstPartyLoadEntryAllowlist",
        "deny-by-default",
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
/// `FirstPartyLoadEntryAllowlist` and never invokes the pnpm backend or its
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
        package_loading.contains("@clay/*")
            && package_loading.contains("FirstPartyLoadEntryAllowlist"),
        "package loading reference must document the first-party resolver path as distinct from install"
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
            source.contains("deny-by-default"),
            "docs must document the deny-by-default boundary"
        );
        assert!(
            source.contains("loadPackage"),
            "docs must mention the loadPackage facade"
        );
    }
    // Carried-forward deferrals are explicitly stated as future work, not
    // current gaps.
    let has_non_clay_deferral = package_loading.contains("Non-`@clay/*`")
        || package_loading.contains("non-`@clay/*`")
        || package_loading.contains("non-@clay/*");
    assert!(
        has_non_clay_deferral
            && (package_loading.contains("Hot-reload") || package_loading.contains("hot reload"))
            && (package_loading.contains("Persistent shared enable state")
                || package_loading.contains("persistent shared enable state")),
        "package loading reference must document the carried-forward deferrals"
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
fn package_loading_docs_keep_third_party_execution_blocked() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_wiki = read("docs/wiki/modules/package-loading.md");
    let hardening = read("docs/wiki/modules/persistent-runtime-hardening.md");
    let resolver_op = read("src/server/ops/packages.rs");

    for phrase in [
        "left-pad",
        "@scope/pkg",
        "URL",
        "local path",
        "traversal",
        "package-manager installation/metadata records do not grant runtime execution authority",
        "approved third-party authority decision",
    ] {
        assert!(
            package_loading.contains(phrase),
            "package loading reference must document third-party block phrase `{phrase}`"
        );
    }
    assert!(
        package_wiki.contains("runtime-execution authority")
            && hardening.contains("before module loading"),
        "wiki pages must preserve the third-party execution gate"
    );
    assert!(
        resolver_op.contains("only resolves first-party `@clay/*` packages")
            && resolver_op.contains("is_valid_first_party_package_segment"),
        "resolver must keep the centralized first-party-only execution gate"
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
