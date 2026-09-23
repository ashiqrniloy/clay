//! Unit tests for the shell theme module (plan 133 task 7: moved verbatim
//! from `src/shell/theme.rs`; module path `shell::theme::tests` unchanged).

use crate::color::Color;
use crate::editor::typography::UiTextVariant;

use super::*;

#[test]
fn theme_token_registry_resolves_package_tokens_to_core_fallbacks() {
    let mut resolver = ThemeTokenResolver::new();
    resolver.insert_package_token(PackageThemeToken {
        token: "markdown.preview.background".to_string(),
        token_type: ThemeTokenType::ColorRole,
        fallback: "surface.panel".to_string(),
        description: "Markdown preview background".to_string(),
    });

    let resolved = resolver
        .resolve("markdown.preview.background", ThemeTokenType::ColorRole)
        .expect("package token should resolve through core fallback");

    assert_eq!(resolved.core_token, "surface.panel");
    assert_eq!(resolved.token_type, ThemeTokenType::ColorRole);
    assert_eq!(
        resolved.value,
        ResolvedThemeValue::Color(Color::from_rgb8(0x21, 0x20, 0x2b))
    );
}

#[test]
fn theme_token_registry_rejects_unknown_tokens_and_type_mismatches() {
    let mut resolver = ThemeTokenResolver::new();
    resolver.insert_package_token(PackageThemeToken {
        token: "markdown.preview.background".to_string(),
        token_type: ThemeTokenType::ColorRole,
        fallback: "surface.panel".to_string(),
        description: "Markdown preview background".to_string(),
    });

    assert!(
        resolver
            .resolve("markdown.preview.background", ThemeTokenType::Spacing)
            .is_none()
    );
    assert!(
        resolver
            .resolve("markdown.preview.missing", ThemeTokenType::ColorRole)
            .is_none()
    );
    assert!(!core_fallback_matches_type(
        "surface.panel",
        ThemeTokenType::Spacing
    ));
}

#[test]
fn sdui_theme_style_uses_core_tokens_for_compatibility_renderer() {
    let style = SduiThemeStyle::default();

    assert!((style.panel_padding - 14.0).abs() < f64::EPSILON);
    assert_eq!(style.title_text, UiTextVariant::Title);
    assert_eq!(style.body_text, UiTextVariant::Body);
    assert_eq!(style.status_text, UiTextVariant::Status);
    assert_eq!(style.panel_background, Color::from_rgb8(0x21, 0x20, 0x2b));
}

/// Phase 20.1: every core design token has a unique name, a typed fallback,
/// and round-trips through the resolver with a matching type. This also
/// locks the 4pt spacing scale and the minimalist elevation/motion/z/density
/// defaults.
#[test]
fn core_design_token_catalog_has_unique_names_and_typed_fallbacks() {
    let resolver = ThemeTokenResolver::new();

    // (token, expected_type, expected resolved value)
    let cases: &[(&str, ThemeTokenType, ResolvedThemeValue)] = &[
        // Legacy color roles resolve unchanged.
        (
            "surface.panel",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x21, 0x20, 0x2b)),
        ),
        (
            "text.primary",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0xee, 0xea, 0xff)),
        ),
        // Phase 20.1 state/border/focus/muted color roles.
        (
            "surface.hover",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x2d, 0x2b, 0x3d)),
        ),
        (
            "surface.active",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x34, 0x31, 0x47)),
        ),
        (
            "surface.disabled",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x1b, 0x1a, 0x24)),
        ),
        (
            "text.disabled",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x80, 0x7a, 0x9b)),
        ),
        (
            "focus.ring",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x96, 0x8a, 0xff)),
        ),
        (
            "border.hairline",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgba8(0x72, 0x6b, 0x98, 0x57)),
        ),
        (
            "border.strong",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0xee, 0xea, 0xff)),
        ),
        (
            "diagnostic.success",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x6b, 0xcf, 0x9b)),
        ),
        // 4pt spacing scale.
        (
            "spacing.xxs",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(4.0),
        ),
        (
            "spacing.xs",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(8.0),
        ),
        (
            "spacing.sm",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(12.0),
        ),
        (
            "spacing.md",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(16.0),
        ),
        (
            "spacing.lg",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(24.0),
        ),
        (
            "spacing.xl",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(32.0),
        ),
        (
            "spacing.xxl",
            ThemeTokenType::Spacing,
            ResolvedThemeValue::F64(48.0),
        ),
        // Restrained radii.
        (
            "radius.xs",
            ThemeTokenType::Radius,
            ResolvedThemeValue::F64(2.0),
        ),
        (
            "radius.sm",
            ThemeTokenType::Radius,
            ResolvedThemeValue::F64(4.0),
        ),
        (
            "radius.lg",
            ThemeTokenType::Radius,
            ResolvedThemeValue::F64(8.0),
        ),
        // Typed dimensions.
        (
            "dimension.sidebar.default",
            ThemeTokenType::Dimension,
            ResolvedThemeValue::Dimension(244.0),
        ),
        (
            "dimension.panel.side.max",
            ThemeTokenType::Dimension,
            ResolvedThemeValue::Dimension(480.0),
        ),
        (
            "dimension.panel.vertical.max",
            ThemeTokenType::Dimension,
            ResolvedThemeValue::Dimension(240.0),
        ),
        // Elevation.
        (
            "elevation.raised",
            ThemeTokenType::Elevation,
            ResolvedThemeValue::Elevation(ElevationLevel::Raised),
        ),
        // Motion durations.
        (
            "motion.instant",
            ThemeTokenType::MotionDuration,
            ResolvedThemeValue::MotionDuration(MotionDuration::const_millis(0.0)),
        ),
        (
            "motion.slow",
            ThemeTokenType::MotionDuration,
            ResolvedThemeValue::MotionDuration(MotionDuration::const_millis(400.0)),
        ),
        // Z-levels.
        (
            "z.tooltip",
            ThemeTokenType::ZLevel,
            ResolvedThemeValue::ZLevel(ZLevel::Tooltip),
        ),
        // Density.
        (
            "density.spacious",
            ThemeTokenType::Density,
            ResolvedThemeValue::Density(DensityLevel::Spacious),
        ),
        // Phase 24.4: centered Command Centre surface.
        (
            "surface.scrim",
            ThemeTokenType::ColorRole,
            ResolvedThemeValue::Color(Color::from_rgb8(0x00, 0x00, 0x00)),
        ),
        (
            "opacity.scrim",
            ThemeTokenType::Opacity,
            ResolvedThemeValue::F32(0.5),
        ),
        (
            "dimension.overlay.centered.width",
            ThemeTokenType::Dimension,
            ResolvedThemeValue::Dimension(640.0),
        ),
    ];

    let mut seen = std::collections::BTreeSet::new();
    for (token, expected_type, expected_value) in cases {
        assert!(
            seen.insert(*token),
            "duplicate token in test catalog: {token}"
        );
        let resolved = resolver
            .resolve(token, *expected_type)
            .unwrap_or_else(|| panic!("{token} should resolve"));
        assert_eq!(resolved.token_type, *expected_type, "{token} type");
        assert_eq!(resolved.value, *expected_value, "{token} value");
    }

    // Unique names across the whole core catalog.
    let all_core = [
        "surface.panel",
        "surface.overlay",
        "surface.main",
        "surface.control",
        "surface.list",
        "surface.selected",
        "surface.hover",
        "surface.active",
        "surface.disabled",
        "text.primary",
        "text.muted",
        "text.disabled",
        "accent.primary",
        "accent.muted",
        "focus.ring",
        "border.hairline",
        "border.subtle",
        "border.strong",
        "border.focus",
        "diagnostic.error",
        "diagnostic.warning",
        "diagnostic.info",
        "diagnostic.success",
        "spacing.none",
        "spacing.inline",
        "spacing.panel",
        "spacing.row",
        "spacing.xxs",
        "spacing.xs",
        "spacing.sm",
        "spacing.md",
        "spacing.lg",
        "spacing.xl",
        "spacing.xxl",
        "radius.none",
        "radius.panel",
        "radius.xs",
        "radius.sm",
        "radius.lg",
        "typography.body",
        "typography.title",
        "typography.status",
        "typography.display",
        "typography.section",
        "typography.detail",
        "typography.caption",
        "opacity.disabled",
        "opacity.full",
        "dimension.border.hairline",
        "dimension.border.thin",
        "dimension.border.thick",
        "dimension.panel.side.default",
        "dimension.panel.side.min",
        "dimension.panel.side.max",
        "dimension.panel.vertical.default",
        "dimension.panel.vertical.min",
        "dimension.panel.vertical.max",
        "dimension.sidebar.default",
        "elevation.none",
        "elevation.raised",
        "elevation.overlay",
        "motion.instant",
        "motion.fast",
        "motion.normal",
        "motion.slow",
        "z.base",
        "z.panel",
        "z.overlay",
        "z.modal",
        "z.tooltip",
        "density.compact",
        "density.default",
        "density.spacious",
        "surface.scrim",
        "opacity.scrim",
        "dimension.overlay.centered.width",
    ];
    let mut names = std::collections::BTreeSet::new();
    for token in all_core {
        assert!(names.insert(token), "duplicate core token name: {token}");
        assert_eq!(
            core_token_type(token),
            core_theme_value(token).map(|v| v.token_type)
        );
    }
}

#[test]
fn legacy_theme_tokens_resolve_unchanged() {
    let resolver = ThemeTokenResolver::new();
    // Legacy names, types, and values are preserved exactly.
    assert_eq!(
        resolver
            .resolve("spacing.panel", ThemeTokenType::Spacing)
            .unwrap()
            .value,
        ResolvedThemeValue::F64(14.0)
    );
    assert_eq!(
        resolver
            .resolve("radius.panel", ThemeTokenType::Radius)
            .unwrap()
            .value,
        ResolvedThemeValue::F64(6.0)
    );
    assert_eq!(
        resolver
            .resolve("opacity.disabled", ThemeTokenType::Opacity)
            .unwrap()
            .value,
        ResolvedThemeValue::F32(0.55)
    );
    assert_eq!(
        resolver
            .resolve("typography.body", ThemeTokenType::Typography)
            .unwrap()
            .value,
        ResolvedThemeValue::Typography(UiTextVariant::Body)
    );
    // Type mismatch still rejected for legacy tokens.
    assert!(
        resolver
            .resolve("spacing.panel", ThemeTokenType::Radius)
            .is_none()
    );
    // New types cannot satisfy legacy expected types.
    assert!(
        resolver
            .resolve("dimension.sidebar.default", ThemeTokenType::Spacing)
            .is_none()
    );
}

#[test]
fn package_tokens_accept_new_same_typed_fallbacks() {
    let mut resolver = ThemeTokenResolver::new();
    resolver.insert_package_token(PackageThemeToken {
        token: "my.panel.elevation".to_string(),
        token_type: ThemeTokenType::Elevation,
        fallback: "elevation.raised".to_string(),
        description: "panel elevation".to_string(),
    });
    resolver.insert_package_token(PackageThemeToken {
        token: "my.sidebar.width".to_string(),
        token_type: ThemeTokenType::Dimension,
        fallback: "dimension.sidebar.default".to_string(),
        description: "sidebar width".to_string(),
    });
    resolver.insert_package_token(PackageThemeToken {
        token: "my.tooltip.z".to_string(),
        token_type: ThemeTokenType::ZLevel,
        fallback: "z.tooltip".to_string(),
        description: "tooltip z".to_string(),
    });
    resolver.insert_package_token(PackageThemeToken {
        token: "my.density".to_string(),
        token_type: ThemeTokenType::Density,
        fallback: "density.default".to_string(),
        description: "density".to_string(),
    });
    resolver.insert_package_token(PackageThemeToken {
        token: "my.fade".to_string(),
        token_type: ThemeTokenType::MotionDuration,
        fallback: "motion.fast".to_string(),
        description: "fade duration".to_string(),
    });

    assert!(resolver.resolves_as("my.panel.elevation", ThemeTokenType::Elevation));
    assert!(resolver.resolves_as("my.sidebar.width", ThemeTokenType::Dimension));
    assert!(resolver.resolves_as("my.tooltip.z", ThemeTokenType::ZLevel));
    assert!(resolver.resolves_as("my.density", ThemeTokenType::Density));
    assert!(resolver.resolves_as("my.fade", ThemeTokenType::MotionDuration));

    let elev = resolver
        .resolve("my.panel.elevation", ThemeTokenType::Elevation)
        .unwrap();
    assert_eq!(elev.core_token, "elevation.raised");
    assert_eq!(
        elev.value,
        ResolvedThemeValue::Elevation(ElevationLevel::Raised)
    );
}

#[test]
fn package_tokens_reject_type_mismatch_raw_values_and_invalid_units() {
    let mut resolver = ThemeTokenResolver::new();
    // Same name but wrong fallback type is rejected.
    resolver.insert_package_token(PackageThemeToken {
        token: "my.bad.dimensions".to_string(),
        token_type: ThemeTokenType::Dimension,
        fallback: "elevation.raised".to_string(), // wrong type
        description: "bad".to_string(),
    });
    assert!(!resolver.resolves_as("my.bad.dimensions", ThemeTokenType::Dimension));
    assert!(!resolver.resolves_as("my.bad.dimensions", ThemeTokenType::Elevation));

    // Resolving a dimension token as spacing fails (no cross-type aliases).
    assert!(
        resolver
            .resolve("dimension.sidebar.default", ThemeTokenType::Spacing)
            .is_none()
    );
    // Unknown fallback cannot satisfy any new type.
    assert!(
        resolver
            .resolve("dimension.missing", ThemeTokenType::Dimension)
            .is_none()
    );

    // MotionDuration validation rejects non-finite and out-of-range values.
    assert!(MotionDuration::from_millis(f64::NAN).is_none());
    assert!(MotionDuration::from_millis(f64::INFINITY).is_none());
    assert!(MotionDuration::from_millis(-1.0).is_none());
    assert!(MotionDuration::from_millis(MotionDuration::MAX_MILLIS + 1.0).is_none());
    assert!(MotionDuration::from_millis(0.0).is_some());
    assert!(MotionDuration::from_millis(MotionDuration::MAX_MILLIS).is_some());

    // Dimension validation rejects non-finite and out-of-range values.
    assert!(!is_valid_dimension(f64::NAN));
    assert!(!is_valid_dimension(f64::INFINITY));
    assert!(!is_valid_dimension(-1.0));
    assert!(!is_valid_dimension(MAX_DIMENSION_PX + 1.0));
    assert!(is_valid_dimension(0.0));
    assert!(is_valid_dimension(MAX_DIMENSION_PX));

    // Level parses reject unknown strings.
    assert!(ElevationLevel::parse("huge").is_none());
    assert!(ZLevel::parse("sky").is_none());
    assert!(DensityLevel::parse("tight").is_none());
    assert_eq!(
        ElevationLevel::parse("overlay"),
        Some(ElevationLevel::Overlay)
    );
    assert_eq!(ZLevel::parse("modal"), Some(ZLevel::Modal));
    assert_eq!(DensityLevel::parse("compact"), Some(DensityLevel::Compact));
    // Level names round-trip through `as_str`/`parse`.
    for level in [
        ElevationLevel::None,
        ElevationLevel::Raised,
        ElevationLevel::Overlay,
    ] {
        assert_eq!(ElevationLevel::parse(level.as_str()), Some(level));
    }
    for level in [
        ZLevel::Base,
        ZLevel::Panel,
        ZLevel::Overlay,
        ZLevel::Modal,
        ZLevel::Tooltip,
    ] {
        assert_eq!(ZLevel::parse(level.as_str()), Some(level));
    }
    for level in [
        DensityLevel::Compact,
        DensityLevel::Default,
        DensityLevel::Spacious,
    ] {
        assert_eq!(DensityLevel::parse(level.as_str()), Some(level));
    }
}

#[test]
fn four_point_spacing_scale_and_minimalist_defaults_are_locked() {
    let resolver = ThemeTokenResolver::new();
    // 4pt base scale: 4/8/12/16/24/32/48.
    let scale = ["xxs", "xs", "sm", "md", "lg", "xl", "xxl"];
    let expected = [4.0_f64, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0];
    for (name, value) in scale.into_iter().zip(expected) {
        let token = format!("spacing.{name}");
        assert!(
            (resolve_f64(&resolver, &token, ThemeTokenType::Spacing) - value).abs() < f64::EPSILON,
            "{token}"
        );
    }

    // Typed accessors resolve the new domains.
    assert_eq!(
        resolve_dimension(&resolver, "dimension.sidebar.default"),
        Some(244.0)
    );
    assert_eq!(
        resolve_elevation(&resolver, "elevation.overlay"),
        Some(ElevationLevel::Overlay)
    );
    assert_eq!(
        resolve_motion_duration(&resolver, "motion.normal"),
        Some(200.0)
    );
    assert_eq!(resolve_z_level(&resolver, "z.panel"), Some(ZLevel::Panel));
    assert_eq!(
        resolve_density(&resolver, "density.compact"),
        Some(DensityLevel::Compact)
    );

    // Minimalist defaults: motion prefers instant; elevation is near-flat.
    assert_eq!(
        resolve_motion_duration(&resolver, "motion.instant"),
        Some(0.0)
    );
    assert_eq!(
        resolve_elevation(&resolver, "elevation.none"),
        Some(ElevationLevel::None)
    );
    // Z-level ordering is monotonic for overlay stacking.
    assert!(ZLevel::Base < ZLevel::Panel);
    assert!(ZLevel::Panel < ZLevel::Overlay);
    assert!(ZLevel::Overlay < ZLevel::Modal);
    assert!(ZLevel::Modal < ZLevel::Tooltip);
}

use crate::protocol::{UiDesignTokenOverride, WireDesignTokenValue};

fn override_entry(token: &str, value: WireDesignTokenValue) -> UiDesignTokenOverride {
    UiDesignTokenOverride {
        token: token.to_string(),
        value,
        provenance: "theme-gruvbox".to_string(),
    }
}

#[test]
fn active_theme_round_trips_typed_ui_token_overrides() {
    let overrides = vec![
        override_entry(
            "surface.hover",
            WireDesignTokenValue::Color([0x10, 0x20, 0x30, 0xff]),
        ),
        override_entry("spacing.md", WireDesignTokenValue::Scalar(20.0)),
        override_entry("opacity.full", WireDesignTokenValue::Opacity(0.9)),
        override_entry(
            "dimension.sidebar.default",
            WireDesignTokenValue::Scalar(200.0),
        ),
        override_entry(
            "elevation.raised",
            WireDesignTokenValue::Level("raised".to_string()),
        ),
        override_entry("motion.fast", WireDesignTokenValue::Scalar(80.0)),
        override_entry(
            "z.tooltip",
            WireDesignTokenValue::Level("tooltip".to_string()),
        ),
        override_entry(
            "density.spacious",
            WireDesignTokenValue::Level("spacious".to_string()),
        ),
    ];
    let theme = ResolvedUiTheme::from_active_theme(&overrides).expect("valid overrides");

    // Active overrides win.
    assert_eq!(
        theme.color("surface.hover"),
        Some(Color::from_rgba8(0x10, 0x20, 0x30, 0xff))
    );
    assert_eq!(theme.scalar_f64("spacing.md"), Some(20.0));
    assert_eq!(theme.opacity("opacity.full"), Some(0.9));
    assert_eq!(theme.dimension("dimension.sidebar.default"), Some(200.0));
    assert_eq!(
        theme.elevation("elevation.raised"),
        Some(ElevationLevel::Raised)
    );
    assert_eq!(theme.motion_duration("motion.fast"), Some(80.0));
    assert_eq!(theme.z_level("z.tooltip"), Some(ZLevel::Tooltip));
    assert_eq!(
        theme.density("density.spacious"),
        Some(DensityLevel::Spacious)
    );
    assert!(!theme.is_empty());
}

#[test]
fn theme_install_is_atomic_across_editor_and_ui_tokens() {
    // A coherent theme snapshot carries both editor text-style overrides
    // (consumed by StyleRegistry::from_active_theme) and typed UI tokens
    // (consumed by ResolvedUiTheme::from_active_theme) from the same
    // ActiveTheme; both builders run from one snapshot with no second
    // selection step.
    let active = crate::protocol::ActiveTheme {
        specifier: "@clay/theme-x".to_string(),
        overrides: Vec::new(),
        design_tokens: vec![
            override_entry(
                "surface.panel",
                WireDesignTokenValue::Color([0x11, 0x22, 0x33, 0xff]),
            ),
            override_entry("radius.sm", WireDesignTokenValue::Scalar(5.0)),
        ],
    };
    let editor = crate::editor::theme::StyleRegistry::from_active_theme(&active);
    let ui = ResolvedUiTheme::from_active_theme(&active.design_tokens).expect("valid");
    // Both registries are built from the same snapshot and reflect overrides.
    assert_eq!(
        ui.color("surface.panel"),
        Some(Color::from_rgba8(0x11, 0x22, 0x33, 0xff))
    );
    assert_eq!(ui.scalar_f64("radius.sm"), Some(5.0));
    // Editor registry has no design-token awareness but still builds cleanly.
    let _ = editor;
}

#[test]
fn gruvbox_themes_use_new_core_fallbacks_without_manifest_changes() {
    // Themes that omit designTokens ship an empty vector and resolve every
    // UI value from core fallbacks unchanged (no manifest edits required).
    let empty = crate::protocol::ActiveTheme {
        specifier: "@clay/theme-gruvbox-material-dark".to_string(),
        overrides: Vec::new(),
        design_tokens: Vec::new(),
    };
    let ui = ResolvedUiTheme::from_active_theme(&empty.design_tokens).expect("empty ok");
    assert!(ui.is_empty());
    // Core fallbacks are the resolved values.
    assert_eq!(
        ui.color("surface.panel"),
        Some(Color::from_rgb8(0x21, 0x20, 0x2b))
    );
    assert_eq!(ui.scalar_f64("spacing.panel"), Some(14.0));
    assert_eq!(ui.opacity("opacity.disabled"), Some(0.55));
    assert_eq!(ui.dimension("dimension.sidebar.default"), Some(244.0));
    assert_eq!(ui.elevation("elevation.none"), Some(ElevationLevel::None));
    assert_eq!(ui.motion_duration("motion.instant"), Some(0.0));
    assert_eq!(ui.z_level("z.base"), Some(ZLevel::Base));
    assert_eq!(ui.density("density.default"), Some(DensityLevel::Default));
}

#[test]
fn base_palette_layers_under_design_tokens_for_legacy_themes() {
    // A legacy theme ships no designTokens, so without the base layer every
    // shell color would resolve to the dark core catalog (the bug: a dark
    // sidebar / scrollbar on a light editor). with_base_ui layers the editor
    // palette under the overrides so chrome tracks the editor text theme.
    use crate::editor::theme::BaseUiColors;
    let base = BaseUiColors {
        shell_bg: Color::from_rgb8(0x11, 0x00, 0x01),
        panel_bg: Color::from_rgb8(0x11, 0x00, 0x02),
        text: Color::from_rgb8(0x11, 0x00, 0x03),
        placeholder: Color::from_rgb8(0x11, 0x00, 0x04),
        selection: Color::from_rgb8(0x11, 0x00, 0x05),
        caret: Color::from_rgb8(0x11, 0x00, 0x06),
        scrollbar: Color::from_rgb8(0x11, 0x00, 0x07),
        scrollbar_track: Color::from_rgb8(0x11, 0x00, 0x08),
        status_bg: Color::from_rgb8(0x11, 0x00, 0x09),
        status_text: Color::from_rgb8(0x11, 0x00, 0x0a),
        diagnostic_error: Color::from_rgb8(0x11, 0x00, 0x0b),
        diagnostic_warning: Color::from_rgb8(0x11, 0x00, 0x0c),
        diagnostic_info: Color::from_rgb8(0x11, 0x00, 0x0d),
        accent: None,
        border_hairline: None,
        border_subtle: None,
        border_strong: None,
    };
    let ui = ResolvedUiTheme::from_active_theme(&[])
        .expect("empty ok")
        .with_base_ui(&base);
    assert_eq!(ui.color("surface.panel"), Some(base.panel_bg));
    assert_eq!(ui.color("surface.list"), Some(base.panel_bg));
    // The completion-menu / tooltip popup background tracks the panel too;
    // without this it fell back to the dark core catalog (a dark completion
    // menu with dark text on a light editor).
    assert_eq!(ui.color("surface.tooltip"), Some(base.panel_bg));
    assert_eq!(ui.color("surface.main"), Some(base.shell_bg));
    assert_eq!(ui.color("surface.selected"), Some(base.selection));
    assert_eq!(ui.color("surface.control"), Some(base.status_bg));
    assert_eq!(ui.color("surface.scrollbar"), Some(base.scrollbar));
    assert_eq!(
        ui.color("surface.scrollbar.track"),
        Some(base.scrollbar_track)
    );
    assert_eq!(ui.color("text.primary"), Some(base.text));
    let expected_muted = if crate::editor::theme::contrast_ratio(base.placeholder, base.panel_bg)
        >= TEXT_CONTRAST_MIN
    {
        base.placeholder
    } else {
        base.text
    };
    assert_eq!(ui.color("text.muted"), Some(expected_muted));
    assert_eq!(ui.color("text.disabled"), Some(base.placeholder));
    assert_eq!(ui.color("surface.hover"), Some(base.selection));
    assert_eq!(ui.color("surface.active"), Some(base.selection));
    assert_eq!(ui.color("accent.primary"), Some(base.caret));
    assert_eq!(ui.color("border.focus"), Some(base.caret));
    assert_eq!(ui.color("focus.ring"), Some(base.caret));
    // The border ladder is flat until the theme expresses one (plan 118 E7).
    assert_eq!(ui.color("border.hairline"), Some(base.scrollbar));
    assert_eq!(ui.color("border.subtle"), Some(base.scrollbar));
    assert_eq!(ui.color("border.strong"), Some(base.scrollbar));
    assert_eq!(ui.color("diagnostic.error"), Some(base.diagnostic_error));
    // Non-color tokens are not in the base palette: core catalog still wins.
    assert_eq!(ui.scalar_f64("spacing.panel"), Some(14.0));

    // A design-token override beats the base layer (modern themes win).
    let ui = ResolvedUiTheme::from_active_theme(&[override_entry(
        "surface.panel",
        WireDesignTokenValue::Color([0x22, 0x33, 0x44, 0xff]),
    )])
    .expect("valid")
    .with_base_ui(&base);
    assert_eq!(
        ui.color("surface.panel"),
        Some(Color::from_rgba8(0x22, 0x33, 0x44, 0xff))
    );
    // Sibling token without an override still tracks the base palette.
    assert_eq!(ui.color("surface.scrollbar"), Some(base.scrollbar));
}

#[test]
fn legacy_text_style_contrast_is_checked_before_install() {
    let same = Some([0x10, 0x0f, 0x17, 0xff]);
    let snapshot = crate::protocol::ActiveTheme {
        specifier: "@clay/theme-low-contrast-legacy".to_string(),
        overrides: vec![
            crate::protocol::TextThemeOverride {
                token: "shellBg".to_string(),
                color: same,
                background: None,
                bold: None,
                italic: None,
                underline: None,
                strike: None,
                scale: None,
                provenance: "theme-low-contrast-legacy".to_string(),
            },
            crate::protocol::TextThemeOverride {
                token: "text".to_string(),
                color: same,
                background: None,
                bold: None,
                italic: None,
                underline: None,
                strike: None,
                scale: None,
                provenance: "theme-low-contrast-legacy".to_string(),
            },
        ],
        design_tokens: Vec::new(),
    };

    let failure = validate_active_theme_contrast(&snapshot)
        .expect_err("legacy textStyles must use the same contrast gate");
    assert_eq!(failure.foreground, "text.primary");
    assert_eq!(failure.background, "surface.main");
    assert!(failure.ratio < TEXT_CONTRAST_MIN);
}

/// Plan 118 task E7: a legacy theme (`textStyles` only, no `designTokens`)
/// can express an accent hue and a real border ladder, and the projection
/// follows the theme's own keys instead of the caret / scrollbar stand-ins.
#[test]
fn legacy_theme_can_express_an_accent_and_a_border_ladder() {
    use crate::editor::theme::BaseUiColors;

    let base = BaseUiColors {
        shell_bg: Color::from_rgb8(0xf7, 0xf7, 0xf7),
        panel_bg: Color::from_rgb8(0xff, 0xff, 0xff),
        text: Color::from_rgb8(0x11, 0x11, 0x11),
        placeholder: Color::from_rgb8(0x66, 0x66, 0x66),
        selection: Color::from_rgba8(0x33, 0x66, 0xcc, 0x33),
        caret: Color::from_rgb8(0x11, 0x11, 0x11),
        scrollbar: Color::from_rgb8(0x99, 0x99, 0x99),
        scrollbar_track: Color::from_rgb8(0xee, 0xee, 0xee),
        status_bg: Color::from_rgb8(0xf0, 0xf0, 0xf0),
        status_text: Color::from_rgb8(0x22, 0x22, 0x22),
        diagnostic_error: Color::from_rgb8(0xcc, 0x22, 0x22),
        diagnostic_warning: Color::from_rgb8(0xaa, 0x66, 0x00),
        diagnostic_info: Color::from_rgb8(0x22, 0x55, 0xaa),
        accent: Some(Color::from_rgb8(0x2f, 0x6f, 0x4f)),
        border_hairline: Some(Color::from_rgba8(0x11, 0x11, 0x11, 0x57)),
        border_subtle: Some(Color::from_rgb8(0x33, 0x33, 0x33)),
        border_strong: Some(Color::from_rgb8(0x00, 0x00, 0x00)),
    };
    let ui = ResolvedUiTheme::from_active_theme(&[])
        .expect("empty ok")
        .with_base_ui(&base);

    assert_eq!(ui.color("accent.primary"), base.accent);
    assert_eq!(ui.color("focus.ring"), base.accent);
    assert_eq!(ui.color("border.focus"), base.accent);
    assert_eq!(ui.color("border.hairline"), base.border_hairline);
    assert_eq!(ui.color("border.subtle"), base.border_subtle);
    assert_eq!(ui.color("border.strong"), base.border_strong);

    // The same gate judges the declared ladder: identical shell and border
    // colors are an invisible boundary and are refused by name.
    let same = Some([0x10, 0x0f, 0x17, 0xff]);
    let override_of = |token: &str| crate::protocol::TextThemeOverride {
        token: token.to_string(),
        color: same,
        background: None,
        bold: None,
        italic: None,
        underline: None,
        strike: None,
        scale: None,
        provenance: "theme-legacy-ladder".to_string(),
    };
    let snapshot = crate::protocol::ActiveTheme {
        specifier: "@clay/theme-legacy-ladder".to_string(),
        overrides: vec![
            override_of("shellBg"),
            override_of("panelBg"),
            override_of("borderSubtle"),
        ],
        design_tokens: Vec::new(),
    };
    let failure = validate_active_theme_contrast(&snapshot)
        .expect_err("an invisible legacy boundary must be refused");
    assert_eq!(failure.foreground, "border.subtle");
    assert_eq!(failure.background, "surface.main");
    assert!(failure.ratio < UI_CONTRAST_MIN);
}

#[test]
fn invalid_or_oversized_theme_values_fail_before_install() {
    use crate::protocol::WireDesignTokenValue as W;
    // Unknown token.
    assert_eq!(
        validate_design_token_override("nope.token", &W::Color([0; 4])),
        Err(DesignTokenError::UnknownToken)
    );
    // Type mismatch: color value for a spacing token.
    assert_eq!(
        validate_design_token_override("spacing.md", &W::Color([0; 4])),
        Err(DesignTokenError::TypeMismatch)
    );
    // Out-of-range dimension.
    assert_eq!(
        validate_design_token_override("dimension.sidebar.default", &W::Scalar(f64::INFINITY),),
        Err(DesignTokenError::TypeMismatch)
    );
    assert_eq!(
        validate_design_token_override("dimension.sidebar.default", &W::Scalar(-5.0),),
        Err(DesignTokenError::TypeMismatch)
    );
    // Out-of-range opacity.
    assert_eq!(
        validate_design_token_override("opacity.full", &W::Opacity(2.0)),
        Err(DesignTokenError::TypeMismatch)
    );
    // Phase 24.4 centered Command Centre tokens fail closed: type
    // mismatch, out-of-range dimension, and out-of-range opacity.
    assert_eq!(
        validate_design_token_override("surface.scrim", &W::Scalar(0.5)),
        Err(DesignTokenError::TypeMismatch)
    );
    assert_eq!(
        validate_design_token_override("dimension.overlay.centered.width", &W::Color([0; 4])),
        Err(DesignTokenError::TypeMismatch)
    );
    assert_eq!(
        validate_design_token_override(
            "dimension.overlay.centered.width",
            &W::Scalar(f64::INFINITY)
        ),
        Err(DesignTokenError::TypeMismatch)
    );
    assert_eq!(
        validate_design_token_override("dimension.overlay.centered.width", &W::Scalar(-5.0)),
        Err(DesignTokenError::TypeMismatch)
    );
    assert_eq!(
        validate_design_token_override("opacity.scrim", &W::Opacity(1.5)),
        Err(DesignTokenError::TypeMismatch)
    );
    // Out-of-range motion duration.
    assert_eq!(
        validate_design_token_override("motion.fast", &W::Scalar(5000.0)),
        Err(DesignTokenError::InvalidScalar)
    );
    // Invalid level name.
    assert_eq!(
        validate_design_token_override("elevation.raised", &W::Level("huge".to_string())),
        Err(DesignTokenError::InvalidLevel)
    );
    // Typography overrides are not allowed via design tokens.
    assert_eq!(
        validate_design_token_override("typography.body", &W::Color([0; 4])),
        Err(DesignTokenError::TypographyNotOverridable)
    );
    // Duplicate tokens rejected by from_active_theme.
    let dup = vec![
        override_entry("surface.hover", W::Color([0; 4])),
        override_entry("surface.hover", W::Color([1; 4])),
    ];
    assert_eq!(
        ResolvedUiTheme::from_active_theme(&dup),
        Err(DesignTokenError::DuplicateToken)
    );
}

#[test]
fn theme_switch_does_not_parse_or_execute_package_code_in_paint_paths() {
    // Building the cached registry is a cold-path operation that resolves
    // inert values once; the cached accessors do no parsing. This test locks
    // the cached contract: a second resolution of the same token is a direct
    // map lookup with no allocation visible to the caller.
    let overrides = vec![override_entry(
        "surface.hover",
        WireDesignTokenValue::Color([0xaa, 0xbb, 0xcc, 0xff]),
    )];
    let ui = ResolvedUiTheme::from_active_theme(&overrides).expect("valid");
    // Repeated reads return the same cached value without re-validating.
    for _ in 0..1000 {
        assert_eq!(
            ui.color("surface.hover"),
            Some(Color::from_rgba8(0xaa, 0xbb, 0xcc, 0xff))
        );
        // Unchanged tokens still resolve from core fallbacks.
        assert_eq!(ui.scalar_f64("spacing.panel"), Some(14.0));
    }
}

#[test]
fn packages_cannot_supply_concrete_hierarchy_scales() {
    // The only package channel for typed UI values is `designTokens`. Every
    // `typography.*` token names a semantic variant, not a scale: a package
    // may select the variant name (via component `style.typography`) but can
    // never ship a concrete scale ratio. Hierarchy scales are user-owned
    // (`setTypography().hierarchy`) and live only in `ActiveTypography`.
    use crate::protocol::WireDesignTokenValue as W;
    for token in [
        "typography.body",
        "typography.title",
        "typography.status",
        "typography.display",
        "typography.section",
        "typography.detail",
        "typography.caption",
    ] {
        // A scalar (scale) override is rejected as a typography override.
        assert_eq!(
            validate_design_token_override(token, &W::Scalar(2.0)),
            Err(DesignTokenError::TypographyNotOverridable),
            "packages must not ship concrete hierarchy scale for {token}"
        );
        // A color/level override is likewise rejected: typography tokens are
        // variant selectors, not styled values.
        assert_eq!(
            validate_design_token_override(token, &W::Color([0; 4])),
            Err(DesignTokenError::TypographyNotOverridable)
        );
    }
    // New typography.* tokens resolve to their additive semantic variants.
    assert_eq!(
        core_theme_value("typography.display").unwrap().value,
        ResolvedThemeValue::Typography(crate::editor::typography::UiTextVariant::Display)
    );
    assert_eq!(
        core_theme_value("typography.caption").unwrap().value,
        ResolvedThemeValue::Typography(crate::editor::typography::UiTextVariant::Caption)
    );
}

// --- Phase 20.1 task 6: panel/sidebar/density defaults behind tokens ---

use crate::protocol::WireDesignTokenValue as Wire;
use crate::shell::layout::{FixedSlotId, FixedSlotState};

fn dimension_override(token: &str, value: f64) -> UiDesignTokenOverride {
    UiDesignTokenOverride {
        token: token.to_string(),
        value: Wire::Scalar(value),
        provenance: "test".to_string(),
    }
}

fn density_override(level: &str) -> UiDesignTokenOverride {
    UiDesignTokenOverride {
        token: "density.default".to_string(),
        value: Wire::Level(level.to_string()),
        provenance: "test".to_string(),
    }
}

#[test]
fn legacy_sidebar_and_package_left_panel_share_default_dimension_token() {
    // The SDUI left-slot bridge reads sidebar_width; package fixed-panel
    // state reads side_default for Left/Right. Both draw from one
    // PanelDefaults built from the same core dimension tokens, so the
    // legacy 240px sidebar and package side panel default stay in lockstep.
    let defaults = ResolvedUiTheme::default().panel_defaults();
    assert!((defaults.sidebar_width - SIDEBAR_DEFAULT_WIDTH).abs() < f64::EPSILON);
    assert!((defaults.side_default - PANEL_SIDE_DEFAULT).abs() < f64::EPSILON);
    assert!((defaults.sidebar_width - defaults.side_default).abs() < f64::EPSILON);
    assert!((defaults.side_min - PANEL_SIDE_MIN).abs() < f64::EPSILON);
    assert!((defaults.side_max - PANEL_SIDE_MAX).abs() < f64::EPSILON);
}

#[test]
fn default_panel_geometry_is_unchanged() {
    // No overrides: core fallbacks reproduce the pre-20.1 hardcoded
    // 240/120/48/480/240 geometry exactly, including ordered fixed slot
    // states for all four slots.
    let defaults = ResolvedUiTheme::default().panel_defaults();
    assert!((defaults.vertical_default - PANEL_VERTICAL_DEFAULT).abs() < f64::EPSILON);
    assert!((defaults.vertical_min - PANEL_VERTICAL_MIN).abs() < f64::EPSILON);
    assert!((defaults.vertical_max - PANEL_VERTICAL_MAX).abs() < f64::EPSILON);
    for (slot, size, max) in [
        (FixedSlotId::Left, PANEL_SIDE_DEFAULT, PANEL_SIDE_MAX),
        (FixedSlotId::Right, PANEL_SIDE_DEFAULT, PANEL_SIDE_MAX),
        (FixedSlotId::Top, PANEL_VERTICAL_DEFAULT, PANEL_VERTICAL_MAX),
        (
            FixedSlotId::Bottom,
            PANEL_VERTICAL_DEFAULT,
            PANEL_VERTICAL_MAX,
        ),
    ] {
        let state = FixedSlotState::new(
            slot,
            defaults.default_size(slot),
            defaults.min_size(slot),
            defaults.max_size(slot),
        )
        .expect("ordered default slot state");
        assert!((state.size - size).abs() < f64::EPSILON);
        assert!((state.max_size - max).abs() < f64::EPSILON);
        assert!(state.min_size <= state.size);
        assert!(state.size <= state.max_size);
    }
}

#[test]
fn invalid_panel_token_order_falls_back_before_layout() {
    // A side triple with min > default is misordered; the resolver falls back
    // to the Clay core tuple for that domain rather than constructing a
    // misordered FixedSlotState. Vertical max below its default likewise
    // falls back. Sidebar stays at the override (still valid alone).
    let ui = ResolvedUiTheme::from_active_theme(&[
        dimension_override("dimension.panel.side.min", 400.0),
        dimension_override("dimension.panel.side.default", 200.0),
        dimension_override("dimension.panel.side.max", 480.0),
        dimension_override("dimension.panel.vertical.default", 120.0),
        dimension_override("dimension.panel.vertical.max", 100.0),
    ])
    .expect("each override is individually valid");
    let defaults = ui.panel_defaults();
    assert_eq!(
        (defaults.side_default, defaults.side_min, defaults.side_max),
        (PANEL_SIDE_DEFAULT, PANEL_SIDE_MIN, PANEL_SIDE_MAX),
        "misordered side triple falls back to core"
    );
    assert_eq!(
        (
            defaults.vertical_default,
            defaults.vertical_min,
            defaults.vertical_max
        ),
        (
            PANEL_VERTICAL_DEFAULT,
            PANEL_VERTICAL_MIN,
            PANEL_VERTICAL_MAX
        ),
        "vertical max below default falls back to core"
    );
}

#[test]
fn density_change_scales_token_owned_spacing_without_changing_document_typography() {
    // density.default selects the active level; it scales only the spacing
    // rhythm multiplier. Panel dimensions (which feed document layout and
    // accessibility geometry) are unchanged, and document typography lives
    // on the separate TypographyRegistry, never the UI theme.
    let compact = ResolvedUiTheme::from_active_theme(&[density_override("compact")])
        .expect("compact density");
    let spacious = ResolvedUiTheme::from_active_theme(&[density_override("spacious")])
        .expect("spacious density");
    let base = ResolvedUiTheme::default();

    assert_eq!(base.active_density(), DensityLevel::Default);
    assert!((base.spacing_scale() - 1.0).abs() < f32::EPSILON);
    assert_eq!(compact.active_density(), DensityLevel::Compact);
    assert!(compact.spacing_scale() < 1.0);
    assert_eq!(spacious.active_density(), DensityLevel::Spacious);
    assert!(spacious.spacing_scale() > 1.0);

    // Density never alters panel/sidebar geometry.
    assert_eq!(spacious.panel_defaults(), base.panel_defaults());
    assert_eq!(compact.panel_defaults(), base.panel_defaults());
}

#[test]
fn panel_token_update_changes_geometry_once_and_idempotently() {
    // A panel dimension override yields a distinct geometry view (forces one
    // relayout/refresh on install); re-resolving the same override set is
    // idempotent (no churn). The atomic install boundary in set_ui_theme
    // swaps exactly one view, so the geometry delta is observable once.
    let base = ResolvedUiTheme::default().panel_defaults();
    let widened = ResolvedUiTheme::from_active_theme(&[
        dimension_override("dimension.panel.side.default", 320.0),
        dimension_override("dimension.panel.side.max", 520.0),
    ])
    .expect("valid widen")
    .panel_defaults();
    assert!((widened.side_default - base.side_default).abs() >= f64::EPSILON);
    assert!((widened.side_max - base.side_max).abs() >= f64::EPSILON);
    // Ordered and clamped geometry from the override.
    assert!(widened.side_min <= widened.side_default);
    assert!(widened.side_default <= widened.side_max);
    // Idempotent: re-resolving the same produce an equal view (no churn).
    let again = ResolvedUiTheme::from_active_theme(&[
        dimension_override("dimension.panel.side.default", 320.0),
        dimension_override("dimension.panel.side.max", 520.0),
    ])
    .expect("valid widen")
    .panel_defaults();
    assert_eq!(widened, again);
}
