use std::collections::BTreeSet;

use clay::docs::registry::{
    ClayJsApiRegistry, UPDATE_COMMAND, check_generated_registry_current, registry_source_paths,
    repository_root,
};

fn denied_configuration_authorities() -> [&'static str; 9] {
    [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ]
}

fn frontmatter_security(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("security: "))
        .map(str::trim)
}

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

#[test]
fn lookup_finds_api_by_stable_id_and_export() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    let by_id = registry
        .by_id("clay.editor.clientSetCursorStyle")
        .expect("lookup by stable id");
    assert_eq!(by_id.js_module, "clay:editor");
    assert_eq!(by_id.js_export, "clientSetCursorStyle");

    let by_export = registry
        .by_js_export("clay:editor", "clientSetCursorStyle")
        .expect("lookup by JS module/export");
    assert_eq!(by_export.id, by_id.id);

    let by_name = registry.by_user_facing_name("set cursor style");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].id, "clay.editor.clientSetCursorStyle");

    let server_owned = registry.by_kind_owner(Some("clay-js-api"), Some("server"));
    assert!(
        server_owned
            .iter()
            .any(|entry| entry.id == "clay.keybindings.bindKey"),
        "kind/owner lookup should include server-owned key binding configuration APIs"
    );

    let editor_tagged = registry.by_lookup_tag("editor");
    assert!(
        editor_tagged
            .iter()
            .any(|entry| entry.id == "clay.editor.clientSetCursorStyle"),
        "lookup tag search should find editor configuration APIs"
    );
}

#[test]
fn lookup_finds_configuration_by_custom_property() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for property in ["color", "blinking", "type"] {
        let matches = registry.by_custom_property(property);
        assert!(
            matches
                .iter()
                .any(|entry| entry.id == "clay.editor.clientSetCursorStyle"),
            "custom property lookup should find cursor style by {property}"
        );
    }
}

#[test]
fn cursor_style_custom_properties_are_complete() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let cursor_style = registry
        .by_id("clay.editor.clientSetCursorStyle")
        .expect("cursor style customization API");

    let color = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "color")
        .expect("color custom property");
    assert_eq!(color.property_type, "string");
    assert_eq!(color.default, "inherited");
    assert!(color.description.contains("#ffcc00"));

    let blinking = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "blinking")
        .expect("blinking custom property");
    assert_eq!(blinking.property_type, "boolean");
    assert_eq!(blinking.default, "true");
    assert!(blinking.description.contains("client-local UI metadata"));

    let cursor_type = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "type")
        .expect("type custom property");
    assert_eq!(cursor_type.property_type, "enum");
    assert_eq!(cursor_type.default, "bar");
    for allowed in ["block", "bar", "underline"] {
        assert!(
            cursor_type.description.contains(allowed),
            "type custom property must document allowed value {allowed}"
        );
    }

    let root = repository_root();
    let text = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/editor/client-set-cursor-style.md"),
    )
    .expect("read cursor style API doc");
    assert!(text.contains("default `inherited`"));
    assert!(text.contains("default `true`"));
    assert!(text.contains("allowed values are `\"block\"`, `\"bar\"`, and `\"underline\"`"));
}

#[test]
fn editor_customization_has_no_external_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for id in [
        "clay.editor.clientSetCursorStyle",
        "clay.editor.clientSetViewport",
    ] {
        let entry = registry.by_id(id).expect("editor customization entry");
        assert!(
            entry.security.contains("document mutation"),
            "{id} must deny document mutation authority"
        );
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "workspace",
            "package",
            "WASM",
            "client-side JavaScript",
        ] {
            assert!(
                entry.security.contains(denied),
                "{id} must deny implicit {denied} authority"
            );
        }
    }
}

#[test]
fn configuration_lookup_finds_cursor_customization() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    assert!(
        registry
            .by_lookup_tag("cursorstylecustomization")
            .iter()
            .any(|entry| entry.id == "clay.editor.clientSetCursorStyle"),
        "cursor style customization should be discoverable by lookup tag"
    );
    for property in ["color", "blinking", "type"] {
        assert!(
            registry
                .by_custom_property(property)
                .iter()
                .any(|entry| entry.id == "clay.editor.clientSetCursorStyle"),
            "cursor style customization should be discoverable by {property} custom property"
        );
    }
}

#[test]
fn lookup_lists_empty_default_key_bindings() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for id in [
        "clay.keybindings.bindKey",
        "clay.keybindings.unbindKey",
        "clay.keybindings.listKeyBindings",
        "clay.editor.clientSetCursorStyle",
    ] {
        let entry = registry
            .by_id(id)
            .expect("lookup entry with empty defaults");
        assert!(
            entry.key_bindings.is_empty(),
            "{id} should expose an empty key_bindings list when it has no defaults"
        );
    }

    let escape = registry.by_key_binding("Escape");
    assert_eq!(escape.len(), 1);
    assert_eq!(escape[0].id, "clay.application.quit");
}

#[test]
fn keybinding_configuration_apis_have_empty_defaults() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, export) in [
        ("clay.keybindings.bindKey", "bindKey"),
        ("clay.keybindings.unbindKey", "unbindKey"),
        ("clay.keybindings.listKeyBindings", "listKeyBindings"),
    ] {
        let entry = registry.by_id(id).expect("key binding API is generated");
        assert_eq!(entry.js_module, "clay:keybindings");
        assert_eq!(entry.js_export, export);
        assert!(
            entry.key_bindings.is_empty(),
            "{id} has no default key binding"
        );
        assert!(
            entry.lookup_tags.iter().any(|tag| tag == "keybindings"),
            "{id} should be discoverable by keybindings lookup tag"
        );
        assert!(
            entry.security.contains("client-side JavaScript"),
            "{id} must deny client-side JavaScript authority"
        );
    }
}

#[test]
fn keybinding_configuration_custom_properties_are_queryable() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    let bind_key = registry
        .by_id("clay.keybindings.bindKey")
        .expect("bindKey registry entry");
    let bind_key_properties: BTreeSet<_> = bind_key
        .custom_properties
        .iter()
        .map(|property| property.name.as_str())
        .collect();
    assert_eq!(
        bind_key_properties,
        BTreeSet::from(["command", "key", "scope", "when"])
    );

    for property in ["key", "command", "scope", "when"] {
        let matches = registry.by_custom_property(property);
        assert!(
            matches
                .iter()
                .any(|entry| entry.id == "clay.keybindings.bindKey"),
            "custom property lookup should find bindKey by {property}"
        );
    }
    assert!(
        registry
            .by_custom_property("scope")
            .iter()
            .any(|entry| entry.id == "clay.keybindings.listKeyBindings"),
        "scope lookup should include listKeyBindings"
    );
}

#[test]
fn keybinding_docs_reject_undocumented_authority() {
    let root = repository_root();
    let denied_authorities = [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ];

    for path in [
        "docs/reference/clay-js-api/keybindings/bind-key.md",
        "docs/reference/clay-js-api/keybindings/unbind-key.md",
        "docs/reference/clay-js-api/keybindings/list-key-bindings.md",
    ] {
        let text = std::fs::read_to_string(root.join(path)).expect("read key binding API doc");
        assert!(
            text.contains("server-owned")
                || text.contains("future inert behavior manifests")
                || text.contains("manifest-routing metadata"),
            "{path} must describe behavior-manifest routing instead of client JavaScript hooks"
        );
        for denied in denied_authorities {
            assert!(
                text.contains(denied),
                "{path} must deny implicit {denied} authority"
            );
        }
    }

    let bind_key =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey API doc");
    assert!(
        bind_key.contains("documented Clay command/API ID")
            && bind_key.contains("registered and permissioned"),
        "bindKey docs must require documented/registered command IDs before binding"
    );
}

#[test]
fn configuration_entrypoint_is_documented_and_indexed() {
    let root = repository_root();
    let config_overview =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    assert!(
        config_overview.contains("~/.config/clay/init.js"),
        "configuration overview must document the init.js entry point"
    );

    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let load_module = registry
        .by_id("clay.configuration.loadConfigurationModule")
        .expect("loadConfigurationModule generated entry");
    let state = registry
        .by_id("clay.configuration.getConfigurationState")
        .expect("getConfigurationState generated entry");

    assert_eq!(load_module.js_module, "clay:configuration");
    assert_eq!(load_module.js_export, "loadConfigurationModule");
    assert!(
        load_module
            .security
            .contains("Phase 8 does not execute JavaScript")
    );
    assert!(
        load_module
            .custom_properties
            .iter()
            .any(|property| property.name == "path")
    );
    assert_eq!(state.js_module, "clay:configuration");
    assert_eq!(state.js_export, "getConfigurationState");
    assert!(
        state
            .security
            .contains("Returns configuration metadata only")
    );
    assert!(
        state
            .custom_properties
            .iter()
            .any(|property| property.name == "entryPoint")
    );
}

#[test]
fn configuration_module_loading_is_planned_no_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let load_module = registry
        .by_js_export("clay:configuration", "loadConfigurationModule")
        .expect("configuration module loading export");

    assert!(load_module.key_bindings.is_empty());
    for denied in [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ] {
        assert!(
            load_module.security.contains(denied),
            "loadConfigurationModule must deny implicit {denied} authority"
        );
    }
    assert!(
        registry
            .by_lookup_tag("configuration")
            .iter()
            .any(|entry| entry.id == "clay.configuration.loadConfigurationModule")
    );
    assert!(
        registry
            .by_custom_property("path")
            .iter()
            .any(|entry| entry.id == "clay.configuration.loadConfigurationModule")
    );
}

#[test]
fn lookup_is_read_only() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    assert!(registry.by_id("clay.keybindings.bindKey").is_some());
    assert!(
        registry
            .by_id("clay.configuration.loadConfigurationModule")
            .is_some()
    );
    assert!(
        registry.by_id("~/.config/clay/init.js").is_none(),
        "documentation lookup must not treat local configuration files as executable registry entries"
    );
    assert!(
        registry
            .entries
            .iter()
            .all(|entry| !entry.security.contains("executes configuration files")),
        "lookup exposes documentation metadata only and must not execute JavaScript or configuration files"
    );
}

#[test]
fn generated_registry_configuration_security_denies_implicit_external_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for entry in registry.entries.iter().filter(|entry| {
        entry.lookup_tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "configuration" | "keybindings" | "cursorstylecustomization"
            )
        }) || !entry.custom_properties.is_empty()
    }) {
        for denied in denied_configuration_authorities() {
            assert!(
                entry.security.contains(denied),
                "{} {} generated security metadata is missing no-authority language for {denied}",
                entry.id,
                entry.documentation_path
            );
        }
    }
}

#[test]
fn generated_registry_security_matches_source_docs() {
    let root = repository_root();
    let generated = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let from_docs = ClayJsApiRegistry::from_docs(&root).expect("build registry from source docs");

    for generated_entry in &generated.entries {
        let source_entry = from_docs
            .by_id(&generated_entry.id)
            .unwrap_or_else(|| panic!("source docs are missing {}", generated_entry.id));
        assert_eq!(
            generated_entry.security, source_entry.security,
            "{} {} generated security metadata must match source docs",
            generated_entry.id, generated_entry.documentation_path
        );

        let doc_text = std::fs::read_to_string(root.join(&generated_entry.documentation_path))
            .expect("read source API doc");
        assert_eq!(
            frontmatter_security(&doc_text),
            Some(generated_entry.security.as_str()),
            "{} {} generated registry security must preserve frontmatter exactly",
            generated_entry.id,
            generated_entry.documentation_path
        );
    }
}
