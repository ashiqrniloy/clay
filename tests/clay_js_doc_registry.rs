use std::collections::BTreeSet;

use clay::docs::registry::{
    ClayJsApiRegistry, UPDATE_COMMAND, check_generated_registry_current, registry_source_paths,
    repository_root,
};

#[test]
fn generated_registry_is_current() {
    let root = repository_root();
    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });
}

#[test]
fn generated_registry_contains_all_indexed_public_apis() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build generated registry from docs");
    let registry_paths: BTreeSet<_> = registry
        .entries
        .iter()
        .map(|entry| entry.documentation_path.clone())
        .collect();
    let registry_ids: BTreeSet<_> = registry
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect();

    assert_eq!(
        indexed_paths, registry_paths,
        "every docs/index.md registry source link must appear exactly once in generated registry data"
    );
    assert_eq!(
        registry_ids.len(),
        registry.entries.len(),
        "generated registry entries must have unique stable IDs"
    );
}

#[test]
fn generated_registry_preserves_configuration_metadata() {
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build generated registry from docs");

    let cursor_style = registry
        .entries
        .iter()
        .find(|entry| entry.id == "clay.editor.clientSetCursorStyle")
        .expect("cursor style configuration API is generated");
    assert_eq!(cursor_style.js_module, "clay:editor");
    assert_eq!(cursor_style.js_export, "clientSetCursorStyle");
    assert_eq!(
        cursor_style.js_facade,
        "runtime/js/editor.ts::clientSetCursorStyle"
    );
    assert_eq!(
        cursor_style.backing_rust,
        "src/editor/surface.rs::EditorSurface::paint_caret"
    );
    assert_eq!(cursor_style.deno_op, "op_clay_editor_set_cursor_style");
    assert!(cursor_style.permissions.is_empty());
    assert!(cursor_style.key_bindings.is_empty());
    assert!(cursor_style.lookup_tags.iter().any(|tag| tag == "editor"));
    assert!(cursor_style.security.contains("does not grant filesystem"));
    for property in ["color", "blinking", "type"] {
        assert!(
            cursor_style
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "cursor style registry entry must preserve custom property {property}"
        );
    }

    let bind_key = registry
        .entries
        .iter()
        .find(|entry| entry.id == "clay.keybindings.bindKey")
        .expect("bindKey configuration API is generated");
    assert_eq!(bind_key.js_module, "clay:keybindings");
    assert_eq!(bind_key.js_export, "bindKey");
    assert!(bind_key.key_bindings.is_empty());
    for property in ["key", "command", "scope", "when"] {
        assert!(
            bind_key
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "bindKey registry entry must preserve custom property {property}"
        );
    }

    let quit = registry
        .entries
        .iter()
        .find(|entry| entry.id == "clay.application.quit")
        .expect("quit API is generated");
    assert_eq!(quit.key_bindings, vec!["Escape".to_string()]);
}
