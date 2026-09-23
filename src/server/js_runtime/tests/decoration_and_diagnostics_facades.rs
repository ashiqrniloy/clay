use super::*;

#[tokio::test]
async fn phase18_parse_and_decoration_facades_are_runtime_backed() {
    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/markdown-phase18",
            "markdown",
            &["parse-document", "render-decorations"],
            serde_json::json!({}),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ParseDocument,
            crate::packages::permissions::PackagePermission::RenderDecorations,
        ],
        r#"
            import { serverPublishDecorations } from "clay:decorations";
            import { serverRegisterParseHandler } from "clay:parse";
            const handler = serverRegisterParseHandler({
              mode: "markdown",
              parseUnit: "line-group",
              viewportPriority: true,
            });
            const decorations = serverPublishDecorations({
              documentId: 1,
              documentVersion: 1,
              viewport: { byteStart: 0, byteEnd: 12 },
              spans: [{ byteStart: 0, byteEnd: 5, kind: "syntax", styleToken: "markup.inline-code", fontRole: "monospace", priority: 10 }],
            });
            Deno.core.ops.op_clay_runtime_record(`${handler.mode}:${decorations.publishedSpanCount}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(result.op_records, vec!["markdown:1"]);
    assert_eq!(result.parse_handlers.len(), 1);
    assert_eq!(
        result.published_decoration_set.unwrap().spans[0].font_role,
        Some(crate::protocol::DocumentFontRole::Monospace)
    );
}

#[tokio::test]
async fn semantic_two_axis_publication_accepts_token_type_and_modifiers() {
    use crate::protocol::{DecorationKind, Modifiers, TokenType};

    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@org/semantic",
            "semanticpkg",
            &["render-decorations"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::RenderDecorations],
        r#"
            import { serverPublishDecorations } from "clay:decorations";
            const decorations = serverPublishDecorations({
              documentId: 7,
              documentVersion: 3,
              viewport: { byteStart: 0, byteEnd: 16 },
              spans: [{
                byteStart: 2,
                byteEnd: 10,
                kind: "semantic",
                tokenType: "Function",
                modifiers: ["Declaration", "Readonly"],
                priority: 20,
              }],
            });
            Deno.core.ops.op_clay_runtime_record(`${decorations.publishedSpanCount}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(result.op_records, vec!["1"]);
    let set = result
        .published_decoration_set
        .expect("semantic set published");
    assert_eq!(set.spans.len(), 1);
    assert_eq!(set.spans[0].kind, DecorationKind::Semantic);
    assert_eq!(set.spans[0].token_type, TokenType::Function);
    assert!(set.spans[0].modifiers.contains(Modifiers::DECLARATION));
    assert!(set.spans[0].modifiers.contains(Modifiers::READONLY));
    assert!(set.spans[0].scope.is_none());
    assert_eq!(set.spans[0].provenance.package_prefix, "semanticpkg");
}

#[tokio::test]
async fn diagnostics_facade_publishes_validated_range_diagnostics() {
    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/rust-diag",
            "rust",
            &["render-decorations"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::RenderDecorations],
        r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            const published = serverPublishDiagnostics({
              documentId: 7,
              documentVersion: 3,
              viewport: { byteStart: 0, byteEnd: 64 },
              source: "my-parser",
              spans: [{
                byteStart: 4,
                byteEnd: 5,
                severity: "error",
                code: "parser.syntax-error",
                message: "Syntax error",
              }],
            });
            Deno.core.ops.op_clay_runtime_record(`${published.source}:${published.publishedSpanCount}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(result.op_records, vec!["my-parser:1"]);
    let set = result.published_diagnostic_set.expect("diagnostic set");
    assert_eq!(set.source, "my-parser");
    assert_eq!(set.spans.len(), 1);
    assert_eq!(
        set.spans[0].severity,
        crate::protocol::DiagnosticSeverity::Error
    );
    assert_eq!(set.provenance.package_prefix, "rust");
}

#[tokio::test]
async fn diagnostics_publication_rejects_missing_permission_or_bad_provenance() {
    // No executing-package context at all: raw/config code cannot publish.
    let missing = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            serverPublishDiagnostics({
              documentId: 1,
              documentVersion: 1,
              viewport: { byteStart: 0, byteEnd: 8 },
              source: "my-parser",
              spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
            });
            "#,
        )
        .await
        .unwrap_err();
    assert!(
        missing.to_string().contains("packages.no_active_package"),
        "publication without package context must fail, got {missing}"
    );

    // Enabled package whose approved capabilities were shrunk below
    // render-decorations.
    let service = ClayJsRuntimeService::default();
    let error = evaluate_as_package(
        &service,
        test_package_json("@org/unapproved", "unapproved", &[], serde_json::json!({})),
        vec![],
        r#"
        import { serverPublishDiagnostics } from "clay:diagnostics";
        serverPublishDiagnostics({
          documentId: 1,
          documentVersion: 1,
          viewport: { byteStart: 0, byteEnd: 8 },
          source: "my-parser",
          spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
        });
        "#,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("packages.missing_permission"),
        "missing render-decorations approval must fail, got {error}"
    );
}

#[tokio::test]
async fn diagnostics_publication_rejects_stale_oversized_or_executable_data() {
    let service = ClayJsRuntimeService::default();
    let package_json = test_package_json(
        "@clay/rust-diag",
        "rust",
        &["render-decorations"],
        serde_json::json!({}),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::RenderDecorations];
    let stale = evaluate_as_package(
        &service,
        package_json.clone(),
        approved.clone(),
        r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            serverPublishDiagnostics({
              documentId: 1,
              documentVersion: 1,
              currentDocumentVersion: 2,
              viewport: { byteStart: 0, byteEnd: 8 },
              source: "my-parser",
              spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
            });
            "#,
    )
    .await
    .unwrap_err();
    assert!(
        stale.to_string().contains("diagnostics.publish_failed"),
        "stale version must fail, got {stale}"
    );

    // Executable callback fields are rejected by the facade before any op.
    let executable = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            serverPublishDiagnostics({
              documentId: 1,
              documentVersion: 1,
              viewport: { byteStart: 0, byteEnd: 8 },
              source: "my-parser",
              spans: [],
              callback: () => {},
            });
            "#,
        )
        .await
        .unwrap_err();
    assert!(
        executable
            .to_string()
            .contains("diagnostics.invalid_publication"),
        "executable callback must fail, got {executable}"
    );

    let oversized_source = format!(
        r#"
            import {{ serverPublishDiagnostics }} from "clay:diagnostics";
            serverPublishDiagnostics({{
              documentId: 1,
              documentVersion: 1,
              viewport: {{ byteStart: 0, byteEnd: 8 }},
              source: "my-parser",
              spans: [{{
                byteStart: 1,
                byteEnd: 2,
                severity: "error",
                code: "x",
                message: "{}",
              }}],
            }});
            "#,
        "m".repeat(2048)
    );
    let oversized = evaluate_as_package(&service, package_json, approved, &oversized_source)
        .await
        .unwrap_err();
    assert!(
        oversized.to_string().contains("diagnostics.publish_failed"),
        "oversized message must fail, got {oversized}"
    );
}
