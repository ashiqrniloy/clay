use clay::packages::{permissions::PackagePermission, record::assemble_package_record};
use clay::protocol::{
    BehaviorManifest, CommandAuthority, DecorationActivatePlan, DecorationKind,
    DecorationProvenance, DecorationSet, DecorationSpan, DecorationTarget, InlayHintPayload,
    InlayPlacement, Modifiers, RoutingPolicy, TextByteRange, TokenType, plan_decoration_activate,
    resolve_workspace_href,
};
use clay::server::decorations::{DecorationValidationError, validate_decoration_publication};
use serde_json::json;

fn package(
    name: &str,
    prefix: &str,
    permissions: &[&str],
) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "1.0.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "permissions": permissions,
            "modes": [prefix],
            "docs": "./docs/index.md"
        }
    }))
    .expect("authority package fixture validates")
}

fn link_set(package_name: &str, prefix: &str, target: DecorationTarget) -> DecorationSet {
    let provenance = DecorationProvenance {
        package_name: package_name.to_string(),
        package_version: "1.0.0".to_string(),
        package_prefix: prefix.to_string(),
    };
    let mut span = DecorationSpan::from_vocabulary(
        0,
        5,
        DecorationKind::Link,
        TokenType::Link,
        Modifiers::NONE,
        80,
        provenance,
    );
    span.target = Some(target);
    DecorationSet {
        document_id: 7,
        document_version: 3,
        package_prefix: prefix.to_string(),
        kind: DecorationKind::Link,
        viewport_byte_start: 0,
        viewport_byte_end: 8,
        spans: vec![span],
    }
}

#[test]
fn link_publish_does_not_grant_filesystem_or_network() {
    let package = package("@vendor/links", "links", &["render-decorations"]);
    assert!(
        package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::RenderDecorations)
    );
    assert!(
        !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::Filesystem)
    );
    assert!(
        !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::Network)
    );

    let published = validate_decoration_publication(
        &package,
        3,
        link_set(
            "@vendor/links",
            "links",
            DecorationTarget::WorkspacePath {
                relative_path: "docs/note.md".to_string(),
                range: None,
            },
        ),
    )
    .expect("render-decorations is sufficient to publish inert links");
    assert!(published.spans[0].target.is_some());

    let inlay = DecorationSpan::from_inlay(
        0,
        1,
        InlayHintPayload {
            label: ": i32".to_string(),
            placement: InlayPlacement::After,
        },
        80,
        DecorationProvenance {
            package_name: "@vendor/links".to_string(),
            package_version: "1.0.0".to_string(),
            package_prefix: "links".to_string(),
        },
    );
    let inlay_set = DecorationSet {
        document_id: 7,
        document_version: 3,
        package_prefix: "links".to_string(),
        kind: DecorationKind::InlayHint,
        viewport_byte_start: 0,
        viewport_byte_end: 8,
        spans: vec![inlay],
    };
    validate_decoration_publication(&package, 3, inlay_set)
        .expect("render-decorations is sufficient to publish inert inlays");
}

#[test]
fn link_activation_without_workspace_grant_is_denied() {
    let target = DecorationTarget::WorkspacePath {
        relative_path: "notes.md".to_string(),
        range: None,
    };
    assert_eq!(
        plan_decoration_activate(&target, 7, 0, "readme.md", None),
        DecorationActivatePlan::Denied
    );
    assert_eq!(
        plan_decoration_activate(
            &DecorationTarget::DocumentRange {
                range: TextByteRange::new(2, 4),
            },
            7,
            0,
            "readme.md",
            None,
        ),
        DecorationActivatePlan::Jump { byte_start: 2 }
    );
}

#[test]
fn https_href_does_not_invoke_package_or_network_ops() {
    let target = DecorationTarget::WorkspacePath {
        relative_path: "https://example.com/docs".to_string(),
        range: None,
    };
    let package = package("@vendor/links", "links", &["render-decorations"]);
    let published = validate_decoration_publication(
        &package,
        3,
        link_set("@vendor/links", "links", target.clone()),
    )
    .expect("URL remains inert publication data");

    assert!(resolve_workspace_href("docs/readme.md", target.hover_text()).is_none());
    assert_eq!(
        plan_decoration_activate(&target, 7, 1, "docs/readme.md", None),
        DecorationActivatePlan::Denied
    );
    assert_eq!(
        published.spans[0].target.as_ref().unwrap().hover_text(),
        "https://example.com/docs"
    );
}

#[test]
fn hover_intent_does_not_imply_parse_document_or_language_server() {
    let package = package("@vendor/links", "links", &["render-decorations"]);
    assert!(
        !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::ParseDocument)
    );
    assert!(
        !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::LanguageServer)
    );

    let target = DecorationTarget::DisplayOnly {
        text: "inert hover label".to_string(),
    };
    let published = validate_decoration_publication(
        &package,
        3,
        link_set("@vendor/links", "links", target.clone()),
    )
    .expect("decoration hover data needs only render-decorations");
    assert_eq!(
        published.spans[0].target.as_ref().unwrap().hover_text(),
        target.hover_text()
    );

    let manifest = BehaviorManifest::minimal_text_editing(1);
    for command_id in ["language.hover", "language.goToDefinition"] {
        let command = manifest
            .commands
            .iter()
            .find(|command| command.command_id == command_id)
            .expect("language command is declared");
        assert_eq!(command.authority, CommandAuthority::ServerIntent);
        assert_eq!(command.routing_policy, RoutingPolicy::UiReactivePriority);
    }
}

#[test]
fn render_folding_cannot_publish_or_activate_links() {
    let package = package("@vendor/folds", "folds", &["render-folding"]);
    let error = validate_decoration_publication(
        &package,
        3,
        link_set(
            "@vendor/folds",
            "folds",
            DecorationTarget::WorkspacePath {
                relative_path: "notes.md".to_string(),
                range: None,
            },
        ),
    )
    .expect_err("render-folding must not publish links");
    assert!(matches!(
        error,
        DecorationValidationError::MissingPermission { .. }
    ));
    assert_eq!(
        plan_decoration_activate(
            &DecorationTarget::DisplayOnly {
                text: "https://example.com".to_string(),
            },
            7,
            1,
            "readme.md",
            None,
        ),
        DecorationActivatePlan::Denied
    );
}
