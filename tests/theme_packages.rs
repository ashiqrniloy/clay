//! Plan 046 task 6: the two first-party Gruvbox Material theme packages must
//! parse as inert style-data, provide a FULL mapping (every base UI color key +
//! every `TokenType` variant), and resolve into the `StyleRegistry`.

use std::collections::HashSet;

use clay::editor::theme::{
    STATUS_CHROME_MIN_CONTRAST, StyleRegistry, TextStyleOverride, parse_override_token,
    status_chrome_contrast_ratio, status_chrome_meets_contrast, validate_active_theme_contrast,
};
use clay::packages::record::{PackageRecord, assemble_package_record};
use clay::protocol::{
    ActiveTheme, TextThemeOverride, TokenType, UiDesignTokenOverride, WireDesignTokenValue,
};

const EXPECTED_BASE_UI_KEYS: &[&str] = &[
    "shellBg",
    "panelBg",
    "text",
    "placeholder",
    "selection",
    "caret",
    "scrollbar",
    "scrollbarTrack",
    "statusBg",
    "statusText",
    "diagnosticError",
    "diagnosticWarning",
    "diagnosticInfo",
    "searchMatch",
];

const EXPECTED_TOKEN_TYPE_NAMES: &[&str] = &[
    "Namespace",
    "Type",
    "Class",
    "Enum",
    "Interface",
    "Struct",
    "TypeParameter",
    "Parameter",
    "Variable",
    "Property",
    "EnumMember",
    "Event",
    "Function",
    "Method",
    "Macro",
    "Keyword",
    "Modifier",
    "Comment",
    "String",
    "Number",
    "Regexp",
    "Operator",
    "Decorator",
    "Heading1",
    "Heading2",
    "Heading3",
    "Heading4",
    "Heading5",
    "Heading6",
    "ListItem",
    "Quote",
    "CodeBlock",
    "CodeSpan",
    "Link",
    "Paragraph",
];

fn read_theme_package(specifier: &str, dir: &str) -> serde_json::Value {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest_dir}/packages/{dir}/package.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {specifier} package.json ({path}): {err}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse {specifier} package.json as JSON: {err}"))
}

fn assert_full_theme_mapping(specifier: &str, dir: &str, keyword_bold: bool) {
    let value = read_theme_package(specifier, dir);
    let record = assemble_package_record(&value).unwrap_or_else(|err| {
        panic!(
            "{specifier} must validate as inert style-data: rule={:?} msg={}",
            err.rule, err.message
        )
    });
    assert_eq!(record.manifest.name, specifier);
    // Themes are inert data: no executable permission, no mode registration.
    assert!(
        record.manifest.clay.permissions.is_empty(),
        "{specifier} must request no permissions"
    );
    assert!(
        record.manifest.clay.modes.is_empty(),
        "{specifier} must register no modes"
    );

    let overrides = &record.contributions.text_styles;
    // Full mapping: 14 base UI keys + 35 TokenType variants = 49 entries.
    assert_eq!(
        overrides.len(),
        EXPECTED_BASE_UI_KEYS.len() + EXPECTED_TOKEN_TYPE_NAMES.len(),
        "{specifier} must provide a full textStyles mapping"
    );

    let mut declared: HashSet<String> = overrides.iter().map(|o| o.token.clone()).collect();
    for key in EXPECTED_BASE_UI_KEYS {
        assert!(
            declared.remove(*key),
            "{specifier} must override base UI key `{key}`"
        );
    }
    for name in EXPECTED_TOKEN_TYPE_NAMES {
        assert!(
            declared.remove(*name),
            "{specifier} must map TokenType variant `{name}`"
        );
        // Each declared override target must resolve to a known target.
        assert!(
            parse_override_token(name).is_some(),
            "{specifier} token `{name}` must resolve"
        );
    }
    assert!(
        declared.is_empty(),
        "{specifier} declared unexpected tokens: {declared:?}"
    );

    // Every entry must declare at least one override field (no no-op entries);
    // the package.json contract requires it.
    for o in overrides {
        assert!(
            o.color.is_some()
                || o.bold.is_some()
                || o.italic.is_some()
                || o.underline.is_some()
                || o.strike.is_some(),
            "{specifier} token `{}` must override at least one field",
            o.token
        );
    }

    // The overrides must layer over the Clay default and actually change it:
    // panel background + keyword syntax color must depart from the default.
    // Keyword boldness is theme-specific (Gruvbox Material makes keywords bold;
    // Modus keeps them unbolded per upstream font-lock faces). Construct
    // the editor-side overrides from the pub descriptor fields (the
    // `to_override` helper is pub(crate) and reserved for task 7's setTheme).
    let overrides_view: Vec<TextStyleOverride> = overrides
        .iter()
        .map(|o| TextStyleOverride {
            token: o.token.clone(),
            color: o
                .color
                .map(|[r, g, b, a]| clay::color::Color::from_rgba8(r, g, b, a)),
            background: o
                .background
                .map(|[r, g, b, a]| clay::color::Color::from_rgba8(r, g, b, a)),
            bold: o.bold,
            italic: o.italic,
            underline: o.underline,
            strike: o.strike,
            scale: o.scale.map(|milli| f32::from(milli) / 1000.0),
            provenance: o.provenance.clone(),
        })
        .collect();
    let registry = StyleRegistry::with_text_overrides(&overrides_view);
    let default = StyleRegistry::default();
    assert_ne!(
        registry.base.panel_bg, default.base.panel_bg,
        "{specifier} panelBg override must change the registry"
    );
    assert_ne!(
        registry.style_for(
            clay::protocol::DecorationKind::Syntax,
            TokenType::Keyword,
            clay::protocol::Modifiers::NONE,
        ),
        default.style_for(
            clay::protocol::DecorationKind::Syntax,
            TokenType::Keyword,
            clay::protocol::Modifiers::NONE,
        ),
        "{specifier} Keyword override must change the rendered StyleSpec"
    );
    assert_eq!(
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::Keyword,
                clay::protocol::Modifiers::NONE,
            )
            .bold,
        keyword_bold,
        "{specifier} keyword boldness must match the theme's upstream intent"
    );
    assert_ne!(
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::Heading1,
                clay::protocol::Modifiers::NONE,
            )
            .color,
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::Heading2,
                clay::protocol::Modifiers::NONE,
            )
            .color,
        "{specifier} must preserve per-TokenType color overrides instead of collapsing prose tokens"
    );
    assert!(
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::Quote,
                clay::protocol::Modifiers::NONE,
            )
            .background
            .is_some(),
        "{specifier} Quote must resolve a background fill"
    );
    assert!(
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::CodeBlock,
                clay::protocol::Modifiers::NONE,
            )
            .background
            .is_some(),
        "{specifier} CodeBlock must resolve a background fill"
    );
    assert!(
        registry
            .style_for(
                clay::protocol::DecorationKind::SearchMatch,
                TokenType::Variable,
                clay::protocol::Modifiers::NONE,
            )
            .background
            .is_some(),
        "{specifier} searchMatch must resolve a background fill"
    );
    assert!(
        registry.size_scale(TokenType::Heading1) > registry.size_scale(TokenType::Paragraph),
        "{specifier} Heading1 must resolve larger than body"
    );
    assert_ne!(
        registry
            .diagnostic_style(clay::protocol::DiagnosticSeverity::Error)
            .color,
        registry
            .diagnostic_style(clay::protocol::DiagnosticSeverity::Warning)
            .color,
        "{specifier} must provide distinct diagnosticError/diagnosticWarning colors"
    );
    assert_ne!(
        registry
            .diagnostic_style(clay::protocol::DiagnosticSeverity::Warning)
            .color,
        registry
            .diagnostic_style(clay::protocol::DiagnosticSeverity::Info)
            .color,
        "{specifier} must provide distinct diagnosticWarning/diagnosticInfo colors"
    );

    // Phase 26.1: dormant syntax vocabulary entries (the ones first-party
    // queries will start emitting in task 26.2) must not collapse to the same
    // resolved StyleSpec as any other syntax token in the same theme.
    let dormant = [
        clay::protocol::TokenType::Macro,
        clay::protocol::TokenType::Property,
        clay::protocol::TokenType::Method,
        clay::protocol::TokenType::Parameter,
        clay::protocol::TokenType::EnumMember,
        clay::protocol::TokenType::Operator,
        clay::protocol::TokenType::TypeParameter,
        clay::protocol::TokenType::Regexp,
        clay::protocol::TokenType::Decorator,
    ];
    let syntax_spec = |tt: clay::protocol::TokenType| {
        registry.style_for(
            clay::protocol::DecorationKind::Syntax,
            tt,
            clay::protocol::Modifiers::NONE,
        )
    };
    for (i, tt) in dormant.iter().enumerate() {
        let spec = syntax_spec(*tt);
        for other in EXPECTED_TOKEN_TYPE_NAMES {
            let other_tt = clay::protocol::TokenType::from_name(other).unwrap();
            if *tt == other_tt {
                continue;
            }
            assert_ne!(
                spec,
                syntax_spec(other_tt),
                "{specifier} dormant token {tt:?} must not share a StyleSpec with {other}"
            );
        }
        // Also distinct from the other dormant tokens (redundant with the loop
        // above, but makes the intent explicit).
        for other in dormant.iter().skip(i + 1) {
            assert_ne!(
                syntax_spec(*tt),
                syntax_spec(*other),
                "{specifier} dormant tokens must be distinct: {tt:?} vs {other:?}"
            );
        }
    }
}

#[test]
fn gruvbox_material_dark_theme_is_inert_full_mapping() {
    assert_full_theme_mapping(
        "@clay/theme-gruvbox-material-dark",
        "theme-gruvbox-material-dark",
        true,
    );
}

#[test]
fn gruvbox_material_light_theme_is_inert_full_mapping() {
    assert_full_theme_mapping(
        "@clay/theme-gruvbox-material-light",
        "theme-gruvbox-material-light",
        true,
    );
}

#[test]
fn modus_operandi_theme_is_inert_full_mapping() {
    assert_full_theme_mapping("@clay/theme-modus-operandi", "theme-modus-operandi", false);
}

#[test]
fn modus_vivendi_theme_is_inert_full_mapping() {
    assert_full_theme_mapping("@clay/theme-modus-vivendi", "theme-modus-vivendi", false);
}

#[test]
fn gruvbox_themes_distinct_palettes() {
    assert_distinct_theme_palettes(
        (
            "@clay/theme-gruvbox-material-dark",
            "theme-gruvbox-material-dark",
        ),
        (
            "@clay/theme-gruvbox-material-light",
            "theme-gruvbox-material-light",
        ),
    );
}

#[test]
fn modus_themes_distinct_palettes() {
    assert_distinct_theme_palettes(
        ("@clay/theme-modus-operandi", "theme-modus-operandi"),
        ("@clay/theme-modus-vivendi", "theme-modus-vivendi"),
    );
}

fn assert_distinct_theme_palettes(a: (&str, &str), b: (&str, &str)) {
    // The pair is genuinely different (e.g. panel backgrounds differ, the
    // text colors are inverse), not duplicate declarations.
    let dark = assemble_package_record(&read_theme_package(a.0, a.1)).expect("first validates");
    let light = assemble_package_record(&read_theme_package(b.0, b.1)).expect("second validates");

    let panel = |r: &PackageRecord| {
        r.contributions
            .text_styles
            .iter()
            .find(|o| o.token == "panelBg")
            .and_then(|o| o.color)
            .expect("panelBg override present")
    };
    assert_ne!(panel(&dark), panel(&light));
    let text_color = |r: &PackageRecord| {
        r.contributions
            .text_styles
            .iter()
            .find(|o| o.token == "text")
            .and_then(|o| o.color)
            .expect("text override present")
    };
    assert_ne!(text_color(&dark), text_color(&light));
}

#[test]
fn gruvbox_themes_status_chrome_meets_aa_contrast() {
    for (specifier, dir) in [
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
    ] {
        let value = read_theme_package(specifier, dir);
        let record = assemble_package_record(&value).expect("theme validates");
        let overrides: Vec<TextStyleOverride> = record
            .contributions
            .text_styles
            .iter()
            .map(|o| TextStyleOverride {
                token: o.token.clone(),
                color: o
                    .color
                    .map(|[r, g, b, a]| clay::color::Color::from_rgba8(r, g, b, a)),
                background: o
                    .background
                    .map(|[r, g, b, a]| clay::color::Color::from_rgba8(r, g, b, a)),
                bold: o.bold,
                italic: o.italic,
                underline: o.underline,
                strike: o.strike,
                scale: o.scale.map(|milli| f32::from(milli) / 1000.0),
                provenance: o.provenance.clone(),
            })
            .collect();
        let registry = StyleRegistry::with_text_overrides(&overrides);
        let ratio = status_chrome_contrast_ratio(&registry);
        assert!(
            status_chrome_meets_contrast(&registry),
            "{specifier} status chrome contrast {ratio:.2} must be >= {STATUS_CHROME_MIN_CONTRAST}"
        );
    }
}

/// Plan 088 task 3: every bundled theme package's full editor/base palette
/// meets WCAG AA on every required foreground/background pair. Legacy
/// `textStyles` are included in the snapshot so the compatibility projection
/// used by the client is validated, not only the core fallback palette.
#[test]
fn bundled_themes_sdui_pairs_meet_aa_contrast() {
    let bundled = [
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
    for (specifier, dir) in bundled {
        let value = read_theme_package(specifier, dir);
        let record = assemble_package_record(&value).expect("theme validates");
        let overrides = record
            .contributions
            .text_styles
            .iter()
            .map(|entry| TextThemeOverride {
                token: entry.token.clone(),
                color: entry.color,
                background: entry.background,
                bold: entry.bold,
                italic: entry.italic,
                underline: entry.underline,
                strike: entry.strike,
                scale: entry.scale,
                provenance: entry.provenance.clone(),
            })
            .collect();
        let snapshot = ActiveTheme {
            specifier: specifier.to_string(),
            overrides,
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

/// Phase 20.7 task 3: a theme snapshot whose `text.primary` overrides to match
/// `surface.main` is rejected with a `ContrastFailure` naming the pair, ratio,
/// and threshold (4.5 text). The AA floor is enforced before install so a
/// low-contrast palette never reaches the client.
#[test]
fn theme_package_below_aa_contrast_is_rejected() {
    // surface.main core fallback is #100f17. Override text.primary to the same
    // color so the pair collapses to a 1.0 contrast ratio.
    let surface_main = [0x10, 0x0f, 0x17, 0xff];
    let snapshot = ActiveTheme {
        specifier: "@clay/theme-low-contrast".to_string(),
        overrides: Vec::new(),
        design_tokens: vec![UiDesignTokenOverride {
            token: "text.primary".to_string(),
            value: WireDesignTokenValue::Color(surface_main),
            provenance: "theme-low-contrast".to_string(),
        }],
    };
    let failure = validate_active_theme_contrast(&snapshot)
        .expect_err("low-contrast text.primary/surface.main pair must be rejected");
    assert_eq!(failure.foreground, "text.primary");
    assert_eq!(failure.background, "surface.main");
    assert_eq!(failure.threshold, 4.5);
    assert!(
        failure.ratio < 4.5,
        "ratio {:.2} must be below 4.5",
        failure.ratio
    );
}

#[test]
fn design_neobrutal_bundled_package_validates_as_inert_data() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest_dir}/packages/design-neobrutal/package.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read @clay/design-neobrutal package.json ({path}): {err}"));
    let value: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse @clay/design-neobrutal as JSON: {err}"));

    let record = assemble_package_record(&value).unwrap_or_else(|err| {
        panic!(
            "@clay/design-neobrutal must validate as inert design-system data: rule={:?} msg={}",
            err.rule, err.message
        )
    });

    assert_eq!(record.manifest.name, "@clay/design-neobrutal");
    assert_eq!(record.manifest.version, "0.1.0");
    assert!(record.manifest.clay.permissions.is_empty());
    assert!(record.manifest.clay.modes.is_empty());
    assert!(record.manifest.clay.entry.is_none());
    assert!(record.manifest.clay.load_entry.is_none());

    let ds = record
        .contributions
        .ui_design_system
        .as_ref()
        .expect("@clay/design-neobrutal must contribute uiDesignSystem");

    assert_eq!(ds.id, "@clay/design-neobrutal");
    assert_eq!(ds.schema_version, 1);
    assert_eq!(ds.display_name, "Neobrutal (Default)");
    assert!(
        ds.recipe_count >= 25,
        "must contain at least 25 component recipes, got {}",
        ds.recipe_count
    );

    // Verify raw JSON contains zero literal color strings (#..., rgb, hsl)
    let decl_str = &ds.declaration_json;
    assert!(
        !decl_str.contains("\"#"),
        "design system must not contain literal hex colors"
    );
    assert!(
        !decl_str.contains("rgb("),
        "design system must not contain rgb() colors"
    );
    assert!(
        !decl_str.contains("hsl("),
        "design system must not contain hsl() colors"
    );

    // Parse declaration and verify Neobrutal geometry, borders, shadows, and legibility invariants
    let decl: clay::shell::design_system::UiDesignSystemDeclaration =
        serde_json::from_str(decl_str).expect("declaration_json must deserialize cleanly");
    assert_eq!(decl.schema_version, 1);

    // Plan 110 Task 7 Conformance:
    // 1. Neobrutal 90-degree corner geometry: border_radius must be 0.0 everywhere.
    // 2. Zero backdrop blur anywhere.
    for (key, recipe) in &decl.recipes {
        if let Some(radius) = recipe.border_radius {
            assert_eq!(
                radius, 0.0,
                "Neobrutal recipe {key} must have border_radius 0.0, got {radius}"
            );
        }
        assert_eq!(
            recipe.backdrop_blur.unwrap_or(0.0),
            0.0,
            "Neobrutal recipe {key} must not apply backdrop blur"
        );
    }

    // 3. 2px structural borders at rest on interactive kinds using the ink role (text.primary)
    let interactive_rest_keys = [
        "button.default.root.rest",
        "button.primary.root.rest",
        "button.muted.root.rest",
        "button.danger.root.rest",
        "textInput.default.input.rest",
        "dropdown.default.root.rest",
        "dropdown.default.trigger.rest",
        "card.default.root.rest",
        "tab.default.item.rest",
        "kbd.default.root.rest",
        "badge.default.root.rest",
        "popover.default.root.rest",
        "menu.default.root.rest",
        "modal.default.dialog.rest",
    ];
    for key_str in interactive_rest_keys {
        let key = clay::shell::design_system::RecipeKey::parse(key_str)
            .unwrap_or_else(|e| panic!("failed to parse key {key_str}: {e}"));
        let recipe = decl
            .recipes
            .get(&key)
            .unwrap_or_else(|| panic!("missing interactive recipe {key_str}"));
        let width = recipe
            .border_width
            .unwrap_or_else(|| panic!("interactive recipe {key_str} must declare border_width"));
        assert!(
            width >= 2.0,
            "interactive recipe {key_str} must have rest border_width >= 2.0, got {width}"
        );
        let border_color = recipe
            .border_color
            .as_ref()
            .unwrap_or_else(|| panic!("interactive recipe {key_str} must declare border_color"));
        assert_eq!(
            border_color.as_str(),
            "text.primary",
            "interactive recipe {key_str} must use ink role text.primary for high-contrast border"
        );
    }

    // 4. Hard offset shadows at rest: ink color (text.primary), blur == 0, rest offset >= 3
    let shadowed_rest_keys = [
        "button.default.root.rest",
        "button.primary.root.rest",
        "button.muted.root.rest",
        "button.danger.root.rest",
        "textInput.default.input.rest",
        "dropdown.default.root.rest",
        "dropdown.default.trigger.rest",
        "card.default.root.rest",
        "kbd.default.root.rest",
        "tooltip.default.root.rest",
        "popover.default.root.rest",
        "menu.default.root.rest",
        "modal.default.dialog.rest",
        "commandCentre.default.root.rest",
    ];
    for key_str in shadowed_rest_keys {
        let key = clay::shell::design_system::RecipeKey::parse(key_str)
            .unwrap_or_else(|e| panic!("failed to parse key {key_str}: {e}"));
        let recipe = decl
            .recipes
            .get(&key)
            .unwrap_or_else(|| panic!("missing shadowed recipe {key_str}"));
        let shadows = recipe
            .shadow
            .as_ref()
            .unwrap_or_else(|| panic!("recipe {key_str} must declare shadow"));
        assert!(
            !shadows.is_empty(),
            "recipe {key_str} shadow list must not be empty"
        );
        for (idx, shadow) in shadows.iter().enumerate() {
            assert_eq!(
                shadow.blur, 0.0,
                "recipe {key_str} shadow[{idx}] blur must be 0.0"
            );
            assert_eq!(
                shadow.color_role.as_str(),
                "text.primary",
                "recipe {key_str} shadow[{idx}] color_role must be text.primary (ink)"
            );
            assert!(
                shadow.x >= 3.0 && shadow.y >= 3.0,
                "recipe {key_str} shadow[{idx}] offsets must be >= 3.0, got ({}, {})",
                shadow.x,
                shadow.y
            );
        }
    }

    // 5. List row styling: solid surface.control fill at rest (never transparent), 2px ink border
    let list_row_rest_key = clay::shell::design_system::RecipeKey::parse("list.default.row.rest")
        .expect("list.default.row.rest parses");
    let list_row_rest = decl
        .recipes
        .get(&list_row_rest_key)
        .expect("list.default.row.rest must exist");
    assert_ne!(
        list_row_rest.background_color.as_ref().map(|c| c.as_str()),
        Some("transparent"),
        "list.default.row.rest background must not be transparent"
    );
    assert_eq!(
        list_row_rest.background_color.as_ref().map(|c| c.as_str()),
        Some("surface.control"),
        "list.default.row.rest background must be solid surface.control"
    );
    assert_eq!(
        list_row_rest.border_width,
        Some(2.0),
        "list.default.row.rest border_width must be 2.0"
    );

    // 6. Selected list row: solid surface.selected fill, 2px ink border
    let list_row_selected_key =
        clay::shell::design_system::RecipeKey::parse("list.default.row.selected")
            .expect("list.default.row.selected parses");
    let list_row_selected = decl
        .recipes
        .get(&list_row_selected_key)
        .expect("list.default.row.selected must exist");
    assert_eq!(
        list_row_selected
            .background_color
            .as_ref()
            .map(|c| c.as_str()),
        Some("surface.selected"),
        "list.default.row.selected background must be solid surface.selected"
    );
    assert_eq!(
        list_row_selected.border_width,
        Some(2.0),
        "list.default.row.selected border_width must be 2.0"
    );
    assert_eq!(
        list_row_selected.border_color.as_ref().map(|c| c.as_str()),
        Some("text.primary"),
        "list.default.row.selected border_color must be text.primary"
    );

    // 7. Interactive button tactile feedback: hover lift + active press shift
    let btn_hover_key = clay::shell::design_system::RecipeKey::parse("button.default.root.hover")
        .expect("button.default.root.hover parses");
    let btn_hover = decl
        .recipes
        .get(&btn_hover_key)
        .expect("button.default.root.hover must exist");
    assert_eq!(
        btn_hover.transform_preset,
        Some(clay::shell::design_system::TransformPreset::HoverLift)
    );
    let hover_shadow = &btn_hover.shadow.as_ref().expect("hover shadow")[0];
    assert_eq!((hover_shadow.x, hover_shadow.y), (4.0, 4.0));

    let btn_active_key = clay::shell::design_system::RecipeKey::parse("button.default.root.active")
        .expect("button.default.root.active parses");
    let btn_active = decl
        .recipes
        .get(&btn_active_key)
        .expect("button.default.root.active must exist");
    assert_eq!(
        btn_active.transform_preset,
        Some(clay::shell::design_system::TransformPreset::PressShiftDown)
    );
    let active_shadow = &btn_active.shadow.as_ref().expect("active shadow")[0];
    assert_eq!((active_shadow.x, active_shadow.y), (1.0, 1.0));
}

#[test]
fn design_glass_bundled_package_validates_as_inert_data() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest_dir}/packages/design-glass/package.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read @clay/design-glass package.json ({path}): {err}"));
    let value: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("parse @clay/design-glass as JSON: {err}"));

    let record = assemble_package_record(&value).unwrap_or_else(|err| {
        panic!(
            "@clay/design-glass must validate as inert design-system data: rule={:?} msg={}",
            err.rule, err.message
        )
    });

    assert_eq!(record.manifest.name, "@clay/design-glass");
    assert_eq!(record.manifest.version, "0.1.0");
    assert!(record.manifest.clay.permissions.is_empty());
    assert!(record.manifest.clay.modes.is_empty());
    assert!(record.manifest.clay.entry.is_none());
    assert!(record.manifest.clay.load_entry.is_none());

    let ds = record
        .contributions
        .ui_design_system
        .as_ref()
        .expect("@clay/design-glass must contribute uiDesignSystem");

    assert_eq!(ds.id, "@clay/design-glass");
    assert_eq!(ds.schema_version, 1);
    assert_eq!(ds.display_name, "Glass (Reference)");
    assert!(
        ds.recipe_count >= 25,
        "must contain at least 25 component recipes, got {}",
        ds.recipe_count
    );

    // Verify raw JSON contains zero literal color strings (#..., rgb, hsl)
    let decl_str = &ds.declaration_json;
    assert!(
        !decl_str.contains("\"#"),
        "design system must not contain literal hex colors"
    );
    assert!(
        !decl_str.contains("rgb("),
        "design system must not contain rgb() colors"
    );
    assert!(
        !decl_str.contains("hsl("),
        "design system must not contain hsl() colors"
    );

    // Parse declaration and verify Glass geometry, blur boundaries, and highlights
    let decl: clay::shell::design_system::UiDesignSystemDeclaration =
        serde_json::from_str(decl_str).expect("declaration_json must deserialize cleanly");
    assert_eq!(decl.schema_version, 1);

    // Performance Invariant: Editor text and scroll container must NOT apply backdrop blur
    for (key, recipe) in &decl.recipes {
        let key_str = key.to_string();
        if key_str.starts_with("editor.") || key_str.starts_with("scroll.") {
            assert_eq!(
                recipe.backdrop_blur.unwrap_or(0.0),
                0.0,
                "Recipe {key_str} in scrolling/editor path must have backdrop_blur 0.0 for 60fps performance"
            );
        }
    }

    // Modal dialog must have frosted glass properties (blur > 0, inner highlight)
    let modal_key = clay::shell::design_system::RecipeKey::new(
        "modal",
        "default",
        "dialog",
        clay::shell::design_system::RecipeState::Rest,
    );
    let modal_dialog = decl
        .recipes
        .get(&modal_key)
        .expect("modal dialog recipe must exist");
    assert!(
        modal_dialog.backdrop_blur.unwrap_or(0.0) >= 16.0,
        "modal dialog must have blur >= 16.0"
    );
    assert!(
        modal_dialog.inner_highlight.is_some(),
        "modal dialog must have inner highlight for optical refraction"
    );
}
