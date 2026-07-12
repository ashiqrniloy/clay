//! Plan 046 task 6: the two first-party Gruvbox Material theme packages must
//! parse as inert style-data, provide a FULL mapping (every base UI color key +
//! every `TokenType` variant), and resolve into the `StyleRegistry`.

use std::collections::HashSet;

use clay::editor::theme::{StyleRegistry, TextStyleOverride, parse_override_token};
use clay::packages::record::{PackageRecord, assemble_package_record};
use clay::protocol::TokenType;

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

fn assert_full_gruvbox_mapping(specifier: &str, dir: &str) {
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
    // Full mapping: 13 base UI keys + 35 TokenType variants = 48 entries.
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
    // panel background + keyword syntax color must depart from the default, and
    // Keyword must render bold (Gruvbox Material makes keywords bold). Construct
    // the editor-side overrides from the pub descriptor fields (the
    // `to_override` helper is pub(crate) and reserved for task 7's setTheme).
    let overrides_view: Vec<TextStyleOverride> = overrides
        .iter()
        .map(|o| TextStyleOverride {
            token: o.token.clone(),
            color: o
                .color
                .map(|[r, g, b, a]| masonry::peniko::Color::from_rgba8(r, g, b, a)),
            bold: o.bold,
            italic: o.italic,
            underline: o.underline,
            strike: o.strike,
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
    assert!(
        registry
            .style_for(
                clay::protocol::DecorationKind::Syntax,
                TokenType::Keyword,
                clay::protocol::Modifiers::NONE,
            )
            .bold,
        "{specifier} must make Keywords bold by default"
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
}

#[test]
fn gruvbox_material_dark_theme_is_inert_full_mapping() {
    assert_full_gruvbox_mapping(
        "@clay/theme-gruvbox-material-dark",
        "theme-gruvbox-material-dark",
    );
}

#[test]
fn gruvbox_material_light_theme_is_inert_full_mapping() {
    assert_full_gruvbox_mapping(
        "@clay/theme-gruvbox-material-light",
        "theme-gruvbox-material-light",
    );
}

#[test]
fn gruvbox_themes_distinct_palettes() {
    // Dark and light are genuinely different (e.g. panel backgrounds differ, the
    // text colors are inverse), not duplicate declarations.
    let dark = assemble_package_record(&read_theme_package(
        "@clay/theme-gruvbox-material-dark",
        "theme-gruvbox-material-dark",
    ))
    .expect("dark validates");
    let light = assemble_package_record(&read_theme_package(
        "@clay/theme-gruvbox-material-light",
        "theme-gruvbox-material-light",
    ))
    .expect("light validates");

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
