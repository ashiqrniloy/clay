//! Plan 046 task 6: the two first-party Gruvbox Material theme packages must
//! parse as inert style-data, provide a FULL mapping (every base UI color key +
//! every `TokenType` variant), and resolve into the `StyleRegistry`.

use std::collections::HashSet;

use clay::editor::theme::{
    STATUS_CHROME_MIN_CONTRAST, StyleRegistry, TextStyleOverride, parse_override_token,
    status_chrome_contrast_ratio, status_chrome_meets_contrast, validate_active_theme_contrast,
};
use clay::packages::record::{PackageRecord, assemble_package_record};
use clay::protocol::{ActiveTheme, TokenType, UiDesignTokenOverride, WireDesignTokenValue};

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

/// Plan 088 task 3, extended by plan 118 task 13: every bundled theme package's
/// full editor/base palette meets WCAG AA on every required foreground/background
/// pair. The snapshot carries the theme's own `designTokens` as well as its
/// legacy `textStyles`, so the compatibility projection *and* the typed overrides
/// the client paints with are validated — not a core-fallback stub.
#[test]
fn bundled_themes_sdui_pairs_meet_aa_contrast() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let value = read_theme_package(specifier, dir);
        let record = assemble_package_record(&value).expect("theme validates");
        let snapshot = super::package_ui_conformance::theme_active_theme(specifier, &record);
        assert!(
            !snapshot.design_tokens.is_empty(),
            "{specifier} must ship the language's typed UI roles"
        );
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
    assert!((failure.threshold - 4.5).abs() < f64::EPSILON);
    assert!(
        failure.ratio < 4.5,
        "ratio {:.2} must be below 4.5",
        failure.ratio
    );
}

/// The frozen approved theme board (plan 118 task 5/7), copied into
/// `design-artifacts/approved/` on approval. It is the specification for the
/// four shipped themes' typed UI roles and depth pair.
fn approved_theme_board() -> serde_json::Value {
    let path = format!(
        "{}/design-artifacts/approved/quiet-instrument-migration/theme-values.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "read theme board ({path}): {err} — the approved board is a required contract \
             input for the shipped themes (`design-artifacts/README.md`)"
        )
    });
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse theme board ({path}): {err}"))
}

/// `#rgb`/`#rrggbb`/`#rrggbbaa` → RGBA bytes, the same shape the typed
/// `designTokens` descriptor stores, so a board value and a declared value are
/// comparable byte for byte.
fn parse_board_hex(hex: &str) -> [u8; 4] {
    let digits = hex.strip_prefix('#').expect("board hex starts with `#`");
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).expect("board hex is hex");
    match digits.len() {
        6 => [byte(0), byte(2), byte(4), 0xff],
        8 => [byte(0), byte(2), byte(4), byte(6)],
        other => panic!("board hex `{hex}` has {other} digits"),
    }
}

/// Theme-side roles keyed by token name, as declared by a theme package.
fn declared_roles(specifier: &str, dir: &str) -> std::collections::BTreeMap<String, [u8; 4]> {
    let record =
        assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
    record
        .contributions
        .design_tokens
        .iter()
        .map(|descriptor| {
            let clay::packages::record::DesignTokenValueDescriptor::Color(rgba) = descriptor.value
            else {
                panic!(
                    "{specifier} role `{}` must be a color (the language's theme-side roles are colors)",
                    descriptor.token
                );
            };
            (descriptor.token.clone(), rgba)
        })
        .collect()
}

/// Plan 118 task 13: the shipped themes implement the approved board exactly.
/// Each theme's typed `designTokens` are the board's 13 role values, and its
/// `textStyles` depth pair is the board's canvas/panel pair — which is the
/// Gruvbox Material Dark `shellBg`/`panelBg` swap, the one correction the board
/// names outside the token list. A silent value change fails here rather than
/// drifting from the user-approved artifact.
#[test]
fn shipped_theme_roles_match_the_approved_board() {
    let board = approved_theme_board();
    let themes = board["themes"]
        .as_object()
        .expect("board carries a `themes` object");
    let board_specifiers: std::collections::BTreeSet<&str> =
        themes.keys().map(String::as_str).collect();
    let bundled: std::collections::BTreeSet<&str> = super::package_ui_conformance::BUNDLED_THEMES
        .iter()
        .map(|(specifier, _)| *specifier)
        .collect();
    assert_eq!(
        board_specifiers, bundled,
        "the approved board and the bundled theme set must name the same packages"
    );

    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let approved = &themes[*specifier];

        let board_roles: std::collections::BTreeMap<String, [u8; 4]> = approved["tokens"]
            .as_object()
            .expect("theme entry carries `tokens`")
            .iter()
            .map(|(token, value)| {
                (
                    token.clone(),
                    parse_board_hex(value.as_str().expect("token value is a string")),
                )
            })
            .collect();
        assert_eq!(
            declared_roles(specifier, dir),
            board_roles,
            "{specifier} must declare the approved board's role values verbatim"
        );

        // Depth direction (DESIGN.md §10.2): the canvas is the editor `shellBg`
        // and the chrome the `panelBg`, on every theme. Gruvbox Material Dark
        // shipped the inverted pair before this task.
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let style = |key: &str| {
            record
                .contributions
                .text_styles
                .iter()
                .find(|entry| entry.token == key)
                .and_then(|entry| entry.color)
                .unwrap_or_else(|| panic!("{specifier} must declare `{key}`"))
        };
        assert_eq!(
            style("shellBg"),
            parse_board_hex(approved["canvas"].as_str().expect("canvas is a string")),
            "{specifier} `shellBg` must be the approved canvas"
        );
        assert_eq!(
            style("panelBg"),
            parse_board_hex(approved["panel"].as_str().expect("panel is a string")),
            "{specifier} `panelBg` must be the approved chrome"
        );

        // Paint path: the resolved snapshot the client projects into `--clay-*`
        // custom properties carries the theme's own values for these roles, not a
        // core-fallback substitute, and it resolves under the contrast gate.
        let resolved = clay::shell::theme::resolve_theme_token_snapshot(
            &super::package_ui_conformance::theme_active_theme(specifier, &record),
        )
        .unwrap_or_else(|failure| {
            panic!(
                "{specifier} resolved palette fails {}/{} ({:.2} < {:.1})",
                failure.foreground, failure.background, failure.ratio, failure.threshold
            )
        });
        for (token, value) in approved["tokens"].as_object().expect("theme tokens") {
            let expected = value.as_str().expect("token value is a string");
            match resolved.get(token) {
                Some(clay::shell::theme::ThemeTokenValueDto::Color(css)) => assert!(
                    css.eq_ignore_ascii_case(expected),
                    "{specifier} resolves `{token}` to {css}, board says {expected}"
                ),
                other => panic!("{specifier} `{token}` must resolve to a colour, got {other:?}"),
            }
        }
    }
}

/// Plan 118 task 13: the four themes' role *language*, independent of the exact
/// values — the border ladder is one grey at two strengths, one accent drives
/// every accent-driven role, the state fills are opaque, and the scrim never
/// lightens the canvas. These are the invariants that make the values a system
/// rather than 52 independent colours.
#[test]
fn shipped_theme_roles_obey_the_language() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let roles = declared_roles(specifier, dir);
        let role = |token: &str| -> [u8; 4] {
            *roles
                .get(token)
                .unwrap_or_else(|| panic!("{specifier} must declare `{token}`"))
        };

        // Ladder: one grey, quiet step at 34 % and structural step at full
        // strength; `border.strong` is the theme's ink (DESIGN.md §10.1).
        let hairline = role("border.hairline");
        let subtle = role("border.subtle");
        assert_eq!(
            &hairline[..3],
            &subtle[..3],
            "{specifier} `border.hairline` must be the structural grey, not a second colour"
        );
        assert_eq!(
            hairline[3], 0x57,
            "{specifier} hairline must sit at 34 % alpha"
        );
        assert_eq!(
            subtle[3], 0xff,
            "{specifier} `border.subtle` must be opaque"
        );
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let ink = record
            .contributions
            .text_styles
            .iter()
            .find(|entry| entry.token == "text")
            .and_then(|entry| entry.color)
            .unwrap_or_else(|| panic!("{specifier} must declare `text`"));
        assert_eq!(
            role("border.strong"),
            ink,
            "{specifier} `border.strong` must be the theme's ink"
        );

        // One accent drives every accent-driven role; the muted step is the same
        // hue one alpha step down, so an accent change cannot leave a stale ring.
        let accent = role("accent.primary");
        assert_eq!(
            role("focus.ring"),
            accent,
            "{specifier} focus ring is the accent"
        );
        assert_eq!(
            role("border.focus"),
            accent,
            "{specifier} focus border is the accent"
        );
        let muted = role("accent.muted");
        assert_eq!(
            &muted[..3],
            &accent[..3],
            "{specifier} muted accent is the same hue"
        );
        assert_eq!(
            muted[3], 0xbf,
            "{specifier} muted accent must sit at 75 % alpha"
        );
        assert_eq!(
            accent[3], 0xff,
            "{specifier} `accent.primary` must be opaque"
        );

        // State fills are opaque and mutually distinct (DESIGN.md §10.3): a
        // translucent fill lets the surface behind it decide the contrast, and
        // one shared value makes hover, press and selection indistinguishable.
        let fills = [
            role("surface.hover"),
            role("surface.active"),
            role("surface.selected"),
        ];
        for (index, fill) in fills.iter().enumerate() {
            assert_eq!(
                fill[3], 0xff,
                "{specifier} state fill {index} must be opaque"
            );
        }
        assert!(
            fills[0] != fills[1] && fills[1] != fills[2] && fills[0] != fills[2],
            "{specifier} hover/active/selected must be distinct fills"
        );

        // Two text steps, and a scrim that dims rather than lightens the canvas
        // (the dimming colour is the theme's own call — pure black on the light
        // themes, the chrome on Gruvbox Material Dark).
        assert_ne!(
            role("text.muted"),
            role("text.disabled"),
            "{specifier} muted and disabled text must be different steps"
        );
        let canvas = *record
            .contributions
            .text_styles
            .iter()
            .find(|entry| entry.token == "shellBg")
            .and_then(|entry| entry.color)
            .unwrap_or_else(|| panic!("{specifier} must declare `shellBg`"))
            .first_chunk::<4>()
            .expect("RGBA");
        let scrim = role("surface.scrim");
        assert!(
            clay::editor::theme::relative_luminance(clay::color::Color::from_rgba8(
                scrim[0], scrim[1], scrim[2], scrim[3]
            )) <= clay::editor::theme::relative_luminance(clay::color::Color::from_rgba8(
                canvas[0], canvas[1], canvas[2], canvas[3]
            )),
            "{specifier} `surface.scrim` must not be lighter than the canvas"
        );
    }
}

/// Plan 118 task 13: the shipped values are not merely decorative — the AA gate
/// rejects a theme whose own role values fall below the floor. Mutating one
/// shipped `accent.primary` to one shipped `surface.main` collapses the pair to
/// 1:1 and must be rejected by name before it can be installed.
#[test]
fn shipped_theme_accent_below_the_floor_is_rejected() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let canvas = record
            .contributions
            .text_styles
            .iter()
            .find(|entry| entry.token == "shellBg")
            .and_then(|entry| entry.color)
            .unwrap_or_else(|| panic!("{specifier} must declare `shellBg`"));
        let mut snapshot = super::package_ui_conformance::theme_active_theme(specifier, &record);
        let accent = snapshot
            .design_tokens
            .iter_mut()
            .find(|override_entry| override_entry.token == "accent.primary")
            .expect("theme declares accent.primary");
        accent.value = WireDesignTokenValue::Color(canvas);

        let failure = validate_active_theme_contrast(&snapshot).expect_err(&format!(
            "{specifier}: accent.primary painted onto the canvas must be rejected"
        ));
        assert_eq!(failure.foreground, "accent.primary", "{specifier}");
        assert_eq!(failure.background, "surface.main", "{specifier}");
        assert!(
            (failure.threshold - 3.0).abs() < f64::EPSILON,
            "{specifier}"
        );
        assert!(
            failure.ratio < 3.0,
            "{specifier} mutated ratio {:.2} must be below the UI floor",
            failure.ratio
        );
    }
}

/// Plan 118 task 14: the border ladder is monotonic on every surface a boundary
/// is drawn against. A hairline that is louder than the structural boundary, or
/// a `border.strong` that is quieter than `border.subtle`, is a language
/// violation even when each value clears its own floor — the levels stop
/// meaning anything.
#[test]
fn shipped_theme_border_ladder_is_monotonic() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let resolved = clay::shell::theme::resolve_theme_token_snapshot(
            &super::package_ui_conformance::theme_active_theme(specifier, &record),
        )
        .unwrap_or_else(|failure| {
            panic!(
                "{specifier} resolved palette fails {}/{} ({:.2} < {:.1})",
                failure.foreground, failure.background, failure.ratio, failure.threshold
            )
        });
        let color = |role: &str| {
            let value = resolved
                .get(role)
                .unwrap_or_else(|| panic!("{specifier} must resolve `{role}`"));
            let clay::shell::theme::ThemeTokenValueDto::Color(css) = value else {
                panic!("{specifier} `{role}` must resolve to a colour, got {value:?}");
            };
            let [r, g, b, a] = parse_board_hex(css);
            clay::color::Color::from_rgba8(r, g, b, a)
        };
        let ratio = |role: &str, surface: &str| {
            clay::editor::theme::composited_contrast_ratio(color(role), color(surface))
        };

        for surface in ["surface.main", "surface.panel"] {
            let hairline = ratio("border.hairline", surface);
            let subtle = ratio("border.subtle", surface);
            let strong = ratio("border.strong", surface);
            assert!(
                hairline < subtle && subtle < strong,
                "{specifier} on {surface}: hairline {hairline:.2} < subtle {subtle:.2} < strong {strong:.2}"
            );
        }
    }
}

/// Plan 118 task 14: a structural boundary that cannot be seen is rejected by
/// name, with the measured ratio. Mutating a shipped `border.subtle` to the
/// theme's own canvas collapses it to exactly 1:1 — the pre-migration failure
/// the board measured at 1.84–2.74:1.
#[test]
fn shipped_theme_invisible_boundary_is_rejected() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let canvas = record
            .contributions
            .text_styles
            .iter()
            .find(|entry| entry.token == "shellBg")
            .and_then(|entry| entry.color)
            .unwrap_or_else(|| panic!("{specifier} must declare `shellBg`"));

        // Both structural roles, on both surfaces. The gate reports the first
        // failing pair it walks into, so the surface named is whichever comes
        // first in the policy table — the ratio is the claim that matters.
        for role in ["border.subtle", "border.strong"] {
            let mut snapshot =
                super::package_ui_conformance::theme_active_theme(specifier, &record);
            let boundary = snapshot
                .design_tokens
                .iter_mut()
                .find(|override_entry| override_entry.token == role)
                .unwrap_or_else(|| panic!("{specifier} declares {role}"));
            boundary.value = WireDesignTokenValue::Color(canvas);

            let failure = validate_active_theme_contrast(&snapshot).expect_err(&format!(
                "{specifier}: {role} painted onto the canvas must be rejected"
            ));
            assert_eq!(failure.foreground, role, "{specifier}");
            assert!(
                ["surface.main", "surface.panel"].contains(&failure.background),
                "{specifier} {role} failed against {}, not a boundary surface",
                failure.background
            );
            assert!(
                (failure.threshold - 3.0).abs() < f64::EPSILON,
                "{specifier}"
            );
            assert!(
                failure.ratio < 1.2,
                "{specifier} mutated {role}/{} ratio {:.2} must collapse",
                failure.background,
                failure.ratio
            );
        }
    }
}

/// Plan 118 task 14: the hairline is exempt from the 3:1 boundary floor but not
/// from being visible. Its shipped value clears the 1.2:1 visibility floor on
/// both surfaces; an opaque hairline painted in the canvas colour does not, and
/// is rejected by name.
#[test]
fn shipped_theme_invisible_hairline_is_rejected() {
    for (specifier, dir) in super::package_ui_conformance::BUNDLED_THEMES {
        let record =
            assemble_package_record(&read_theme_package(specifier, dir)).expect("theme validates");
        let canvas = record
            .contributions
            .text_styles
            .iter()
            .find(|entry| entry.token == "shellBg")
            .and_then(|entry| entry.color)
            .unwrap_or_else(|| panic!("{specifier} must declare `shellBg`"));
        let resolved = clay::shell::theme::resolve_theme_token_snapshot(
            &super::package_ui_conformance::theme_active_theme(specifier, &record),
        )
        .expect("shipped palette resolves");
        let hairline = resolved
            .get("border.hairline")
            .expect("border.hairline resolves");
        let clay::shell::theme::ThemeTokenValueDto::Color(css) = hairline else {
            panic!("{specifier} `border.hairline` must be a colour, got {hairline:?}");
        };
        let [r, g, b, a] = parse_board_hex(css);
        assert!(
            a < 0xff,
            "{specifier} shipped hairline must keep its 34 % alpha, got {css}"
        );
        let shipped = clay::editor::theme::composited_contrast_ratio(
            clay::color::Color::from_rgba8(r, g, b, a),
            clay::color::Color::from_rgba8(canvas[0], canvas[1], canvas[2], canvas[3]),
        );
        assert!(
            shipped >= 1.2,
            "{specifier} shipped hairline is {shipped:.2}:1 on the canvas, below the 1.2 visibility floor"
        );

        // Opaque canvas colour: invisible by construction.
        let mut snapshot = super::package_ui_conformance::theme_active_theme(specifier, &record);
        let invisible = snapshot
            .design_tokens
            .iter_mut()
            .find(|override_entry| override_entry.token == "border.hairline")
            .expect("theme declares border.hairline");
        invisible.value = WireDesignTokenValue::Color(canvas);

        let failure = validate_active_theme_contrast(&snapshot).expect_err(&format!(
            "{specifier}: an invisible hairline must be rejected"
        ));
        assert_eq!(failure.foreground, "border.hairline", "{specifier}");
        assert!(
            (failure.threshold - 1.2).abs() < f64::EPSILON,
            "{specifier}"
        );
        assert!(
            failure.ratio < 1.2,
            "{specifier} invisible hairline measured {:.2}",
            failure.ratio
        );
    }
}
