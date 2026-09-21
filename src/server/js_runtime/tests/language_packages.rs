use super::*;

#[tokio::test]
async fn load_package_registers_first_party_syntax_grammars() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/rust");
            await loadPackage("@clay/typescript");
            await loadPackage("@clay/javascript");
            "#,
        )
        .await
        .unwrap();

    let languages = evaluation
        .syntax_grammars
        .iter()
        .map(|grammar| (grammar.language_id.as_str(), grammar.engine_tier))
        .collect::<Vec<_>>();
    assert_eq!(
        languages,
        vec![
            (
                "javascript",
                crate::server::syntax::SyntaxEngineTier::Native
            ),
            ("markdown", crate::server::syntax::SyntaxEngineTier::Native),
            ("rust", crate::server::syntax::SyntaxEngineTier::Native),
            ("tsx", crate::server::syntax::SyntaxEngineTier::Native),
            (
                "typescript",
                crate::server::syntax::SyntaxEngineTier::Native
            ),
        ]
    );
}

#[tokio::test]
async fn lsp_rust_package_loads_after_exact_grant_without_starting_child() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    if crate::packages::authorization::resolve_language_server_executable("rustup").is_none() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configuration/lsp-rust");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/rust");
    let mut workspace = WorkspaceState::new();
    let registered = workspace.add_root(&workspace_root).unwrap();
    assert_eq!(registered, 1);

    let runtime = ClayJsRuntimeService::default();
    let evaluation = runtime
        .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(evaluation.document_analyzers.len(), 1);
    let analyzer = &evaluation.document_analyzers[0];
    assert_eq!(analyzer.package.manifest.name, "@clay/lsp-rust");
    assert_eq!(analyzer.id, "lsp-rust.bridge");
    assert_eq!(analyzer.contribution, "lsp-rust.server");
    assert_eq!(analyzer.modes, ["rust"]);
    assert_eq!(
        evaluation.op_records,
        ["@clay/lsp-rust:1"],
        "load must register metadata only; analyzer starts lazily on document open"
    );

    let invocation = runtime
        .evaluate_controlled_module(
            r#"
            import { createRustAnalyzerBridge } from "clay://packages/@clay/lsp-rust/dist/server.js";
            import { FrameDecoder, encodeFrame } from "lsp-shared/framing.js";
            const decoder = new FrameDecoder();
            const reads = [];
            const methods = [];
            const session = {
              async sendBytes(bytes) {
                for (const message of decoder.push(bytes)) {
                  methods.push(message.method);
                  if (message.method === "initialize") reads.push(encodeFrame({
                    jsonrpc: "2.0",
                    id: message.id,
                    result: { capabilities: { textDocumentSync: { openClose: true, change: 2 } } },
                  }));
                }
              },
              async readBytes() { return reads.shift() ?? new Uint8Array(); },
              async stop() {},
            };
            const bridge = createRustAnalyzerBridge({
              startSession: async () => session,
              publishDecorations() {},
              publishDiagnostics() {},
              packageManifest: {},
            });
            await bridge.handle({
              kind: "open",
              identity: { package: "@clay/lsp-rust", contribution: "lsp-rust.server" },
              documentId: 7,
              documentVersion: 1,
              workspaceRootId: 1,
              canonicalRootPath: "/tmp",
              relativePath: "main.rs",
              text: "fn main() {}\n",
            });
            Deno.core.ops.op_clay_runtime_record(methods.join(","));
            "#,
        )
        .await
        .unwrap();
    assert_eq!(
        invocation.op_records,
        ["initialize,initialized,textDocument/didOpen"]
    );
}

#[tokio::test]
async fn lsp_typescript_and_javascript_packages_load_after_exact_grants_without_starting_children()
{
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    if crate::packages::authorization::resolve_language_server_executable(
        "typescript-language-server",
    )
    .is_none()
    {
        return;
    }

    let mut typescript_runtime = None;
    for (config_dir, workspace_dir, package_name, analyzer_id, contribution, mode) in [
        (
            "tests/fixtures/configuration/lsp-typescript",
            "tests/fixtures/lsp/typescript",
            "@clay/lsp-typescript",
            "lsp-typescript.bridge",
            "lsp-typescript.server",
            "typescript",
        ),
        (
            "tests/fixtures/configuration/lsp-javascript",
            "tests/fixtures/lsp/javascript",
            "@clay/lsp-javascript",
            "lsp-javascript.bridge",
            "lsp-javascript.server",
            "javascript",
        ),
    ] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(config_dir);
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(workspace_dir);
        let mut workspace = WorkspaceState::new();
        let registered = workspace.add_root(&workspace_root).unwrap();
        assert_eq!(registered, 1);

        let runtime = ClayJsRuntimeService::default();
        let evaluation = runtime
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        assert_eq!(evaluation.document_analyzers.len(), 1);
        let analyzer = &evaluation.document_analyzers[0];
        assert_eq!(analyzer.package.manifest.name, package_name);
        assert_eq!(analyzer.id, analyzer_id);
        assert_eq!(analyzer.contribution, contribution);
        assert_eq!(analyzer.modes, [mode]);
        assert_eq!(
            evaluation.op_records,
            [format!("{package_name}:1")],
            "load must register metadata only; analyzer starts lazily on document open"
        );
        if package_name == "@clay/lsp-typescript" {
            typescript_runtime = Some(runtime);
        }
    }

    let runtime = typescript_runtime.expect("typescript bridge runtime loaded");
    let invocation = runtime
        .evaluate_controlled_module(
            r#"
            import { createTypescriptBridge } from "clay://packages/@clay/lsp-typescript/dist/server.js";
            import { FrameDecoder, encodeFrame } from "lsp-shared/framing.js";
            const decoder = new FrameDecoder();
            const reads = [];
            const methods = [];
            const session = {
              async sendBytes(bytes) {
                for (const message of decoder.push(bytes)) {
                  methods.push(message.method);
                  if (message.method === "initialize") reads.push(encodeFrame({
                    jsonrpc: "2.0",
                    id: message.id,
                    result: { capabilities: { textDocumentSync: { openClose: true, change: 2 } } },
                  }));
                }
              },
              async readBytes() { return reads.shift() ?? new Uint8Array(); },
              async stop() {},
            };
            const bridge = createTypescriptBridge({
              startSession: async () => session,
              publishDecorations() {},
              publishDiagnostics() {},
              packageManifest: {},
            });
            await bridge.handle({
              kind: "open",
              identity: { package: "@clay/lsp-typescript", contribution: "lsp-typescript.server" },
              documentId: 7,
              documentVersion: 1,
              workspaceRootId: 1,
              canonicalRootPath: "/tmp",
              relativePath: "main.ts",
              text: "export const value = 1;\n",
            });
            Deno.core.ops.op_clay_runtime_record(methods.join(","));
            "#,
        )
        .await
        .unwrap();
    assert_eq!(
        invocation.op_records,
        ["initialize,initialized,textDocument/didOpen"]
    );
}

#[tokio::test]
async fn lsp_markdown_package_loads_after_exact_grant_without_starting_child() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    if crate::packages::authorization::resolve_language_server_executable("marksman").is_none() {
        return;
    }
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configuration/lsp-markdown");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/markdown");
    let mut workspace = WorkspaceState::new();
    let registered = workspace.add_root(&workspace_root).unwrap();
    assert_eq!(registered, 1);

    let runtime = ClayJsRuntimeService::default();
    let evaluation = runtime
        .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(evaluation.document_analyzers.len(), 1);
    let analyzer = &evaluation.document_analyzers[0];
    assert_eq!(analyzer.package.manifest.name, "@clay/lsp-markdown");
    assert_eq!(analyzer.id, "lsp-markdown.bridge");
    assert_eq!(analyzer.contribution, "lsp-markdown.server");
    assert_eq!(analyzer.modes, ["markdown"]);
    assert_eq!(
        evaluation.op_records,
        ["@clay/lsp-markdown:1"],
        "load must register metadata only; analyzer starts lazily on document open"
    );

    let invocation = runtime
        .evaluate_controlled_module(
            r##"
            import { createMarksmanBridge } from "clay://packages/@clay/lsp-markdown/dist/server.js";
            import { FrameDecoder, encodeFrame } from "lsp-shared/framing.js";
            const decoder = new FrameDecoder();
            const reads = [];
            const methods = [];
            const session = {
              async sendBytes(bytes) {
                for (const message of decoder.push(bytes)) {
                  methods.push(message.method);
                  if (message.method === "initialize") reads.push(encodeFrame({
                    jsonrpc: "2.0",
                    id: message.id,
                    result: { capabilities: { textDocumentSync: { openClose: true, change: 1 } } },
                  }));
                }
              },
              async readBytes() { return reads.shift() ?? new Uint8Array(); },
              async stop() {},
            };
            const bridge = createMarksmanBridge({
              startSession: async () => session,
              publishDecorations() {},
              publishDiagnostics() {},
              packageManifest: {},
            });
            await bridge.handle({
              kind: "open",
              identity: { package: "@clay/lsp-markdown", contribution: "lsp-markdown.server" },
              documentId: 7,
              documentVersion: 1,
              workspaceRootId: 1,
              canonicalRootPath: "/tmp",
              relativePath: "README.md",
              text: "# Title\n",
            });
            Deno.core.ops.op_clay_runtime_record(methods.join(","));
            "##,
        )
        .await
        .unwrap();
    assert_eq!(
        invocation.op_records,
        ["initialize,initialized,textDocument/didOpen"]
    );
}

#[tokio::test]
async fn lsp_language_packages_fixture_grants_before_load_without_starting_children() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let required = ["rustup", "typescript-language-server", "marksman"];
    if required.iter().any(|executable| {
        crate::packages::authorization::resolve_language_server_executable(executable).is_none()
    }) {
        return;
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/configuration/lsp-language-packages");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/rust");
    let mut workspace = WorkspaceState::new();
    assert_eq!(workspace.add_root(&workspace_root).unwrap(), 1);

    let evaluation = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    let analyzer_names: Vec<_> = evaluation
        .document_analyzers
        .iter()
        .map(|analyzer| analyzer.package.manifest.name.as_str())
        .collect();
    assert_eq!(
        analyzer_names,
        [
            "@clay/lsp-rust",
            "@clay/lsp-typescript",
            "@clay/lsp-javascript",
            "@clay/lsp-markdown",
        ]
    );
    assert!(
        evaluation.op_records.is_empty(),
        "representative fixture must register only; it must not start language-server children: {:?}",
        evaluation.op_records
    );

    let completion_ids: Vec<_> = evaluation
        .completion_providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect();
    for required_id in [
        "rust.keywords",
        "typescript.keywords",
        "javascript.keywords",
        "markdown.keywords",
    ] {
        assert!(
            completion_ids.contains(&required_id),
            "fixture must register base provider {required_id}; got {completion_ids:?}"
        );
    }
    assert!(
        completion_ids.iter().all(|id| !id.starts_with("lsp-")),
        "LSP completion providers register lazily through analyzers; load must not eagerly start them: {completion_ids:?}"
    );

    for package in [
        "lsp-rust",
        "lsp-typescript",
        "lsp-javascript",
        "lsp-markdown",
    ] {
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{}/packages/{package}/package.json",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
        .unwrap();
        let provider = &manifest["clay"]["contributions"]["completionProviders"][0];
        assert_eq!(provider["priority"].as_i64(), Some(100));
        assert!(
            provider.get("exclusive").is_none()
                || provider["exclusive"] == serde_json::Value::Bool(false)
        );
    }
}
