//! Typed theme-token parsing: token/level enums, the core token catalog, and
//! the package-token resolver (plan 133 task 7 split of `src/shell/theme.rs`).

use crate::str_enum::string_enum_impl;

use std::collections::BTreeMap;

use crate::color::Color;

use crate::editor::typography::UiTextVariant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ThemeTokenType {
    ColorRole,
    Spacing,
    Radius,
    Typography,
    Opacity,
    // Phase 20.1: distinct non-color scalar domains. Each is a closed typed
    // category so duration, z-level, elevation, and density cannot masquerade
    // as spacing, opacity, or an untyped f64. Existing types and values stay
    // valid; these are additive-only.
    Dimension,
    Elevation,
    MotionDuration,
    ZLevel,
    Density,
}

string_enum_impl! {
    pub(crate) ThemeTokenType {
        ColorRole => "color-role",
        Spacing => "spacing",
        Radius => "radius",
        Typography => "typography",
        Opacity => "opacity",
        Dimension => "dimension",
        Elevation => "elevation",
        MotionDuration => "motion-duration",
        ZLevel => "z-level",
        Density => "density",
    }
    all_as_str
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedThemeValue {
    Color(Color),
    F64(f64),
    F32(f32),
    Typography(UiTextVariant),
    // Phase 20.1 typed scalar domains. Concrete package/theme values are
    // validated through the constructors below before client installation so
    // NaN/non-finite/out-of-range dimensions, negative durations, or invalid
    // level/shape strings never reach paint/layout.
    Dimension(f64),
    Elevation(ElevationLevel),
    MotionDuration(MotionDuration),
    ZLevel(ZLevel),
    Density(DensityLevel),
}

/// Near-invisible elevation levels. Minimalist direction: shadows stay
/// barely perceptible; the enum pins order, not a shadow string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ElevationLevel {
    None,
    Raised,
    Overlay,
}

string_enum_impl! {
    #[allow(dead_code)]
    pub(crate) ElevationLevel {
        None => "none",
        Raised => "raised",
        Overlay => "overlay",
    }
}

/// Ordered overlay stacking levels. Higher is closer to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ZLevel {
    Base,
    Panel,
    Overlay,
    Modal,
    Tooltip,
}

string_enum_impl! {
    #[allow(dead_code)]
    pub(crate) ZLevel {
        Base => "base",
        Panel => "panel",
        Overlay => "overlay",
        Modal => "modal",
        Tooltip => "tooltip",
    }
}

/// Information density intent. Concrete geometry is resolved by the shell
/// layout view; the token only carries the semantic level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DensityLevel {
    Compact,
    Default,
    Spacious,
}

string_enum_impl! {
    #[allow(dead_code)]
    pub(crate) DensityLevel {
        Compact => "compact",
        Default => "default",
        Spacious => "spacious",
    }
}

#[allow(dead_code)]
impl DensityLevel {
    /// Spacing-rhythm multiplier for the density level. The shell applies it to
    /// token-owned UI spacing (Phase 20.4 component uplift); panel dimensions
    /// and document typography are never scaled by density.
    pub(crate) const fn spacing_scale(self) -> f32 {
        match self {
            Self::Compact => 0.875,
            Self::Default => 1.0,
            Self::Spacious => 1.125,
        }
    }
}

/// Bounded motion duration in milliseconds. Used for deliberate, restrained
/// transitions only; instant (`0`) is the minimalist default.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct MotionDuration(f64);

#[allow(dead_code)]
impl MotionDuration {
    /// Upper bound keeps motion perceptible and bounded for reduced-motion
    /// and budget guards; adjust only with a measured, documented rationale.
    pub(crate) const MAX_MILLIS: f64 = 1000.0;

    pub(crate) fn as_millis(self) -> f64 {
        self.0
    }

    pub(crate) fn from_millis(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=Self::MAX_MILLIS).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Construct from a compile-time-known-valid millis value. Skips the
    /// validation that [`from_millis`] performs; use only for trusted catalog
    /// constants. Untrusted package/theme input must go through `from_millis`.
    pub(crate) const fn const_millis(value: f64) -> Self {
        Self(value)
    }

    pub(crate) const fn millis(self) -> f64 {
        self.0
    }
}

/// Upper bound for a logical-pixel dimension. Panel bounds fit well under
/// this; the ceiling just rejects NaN/infinite/huge values before install.
pub(crate) const MAX_DIMENSION_PX: f64 = 8192.0;

/// Returns `true` when `value` is a finite, non-negative, bounded dimension.
pub(crate) fn is_valid_dimension(value: f64) -> bool {
    value.is_finite() && (0.0..=MAX_DIMENSION_PX).contains(&value)
}

// Phase 20.1: shared Clay panel/sidebar geometry source. These are the only
// panel-dimension authority; the legacy SDUI left-slot bridge and package
// fixed-panel state both read them through PanelDefaults so one override
// source feeds both. Values mirror the pre-20.1 hardcoded geometry so default
// rendered geometry is unchanged unless an active theme override replaces it.
pub(crate) const SIDEBAR_DEFAULT_WIDTH: f64 = 244.0;
pub(crate) const PANEL_SIDE_DEFAULT: f64 = 244.0;
pub(crate) const PANEL_SIDE_MIN: f64 = 48.0;
pub(crate) const PANEL_SIDE_MAX: f64 = 480.0;
pub(crate) const PANEL_VERTICAL_DEFAULT: f64 = 120.0;
pub(crate) const PANEL_VERTICAL_MIN: f64 = 48.0;
pub(crate) const PANEL_VERTICAL_MAX: f64 = 240.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedThemeToken {
    pub(crate) requested_token: String,
    pub(crate) core_token: String,
    pub(crate) token_type: ThemeTokenType,
    pub(crate) value: ResolvedThemeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageThemeToken {
    pub(crate) token: String,
    pub(crate) token_type: ThemeTokenType,
    pub(crate) fallback: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThemeTokenResolver {
    package_tokens: BTreeMap<String, PackageThemeToken>,
}

impl ThemeTokenResolver {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert_package_token(
        &mut self,
        token: PackageThemeToken,
    ) -> Option<PackageThemeToken> {
        self.package_tokens.insert(token.token.clone(), token)
    }

    pub(crate) fn token_type(&self, token: &str) -> Option<ThemeTokenType> {
        self.package_tokens
            .get(token)
            .map(|declaration| declaration.token_type)
            .or_else(|| core_token_type(token))
    }

    pub(crate) fn resolves_as(&self, token: &str, expected: ThemeTokenType) -> bool {
        self.token_type(token) == Some(expected) && self.resolve(token, expected).is_some()
    }

    pub(crate) fn resolve(
        &self,
        token: &str,
        expected: ThemeTokenType,
    ) -> Option<ResolvedThemeToken> {
        if let Some(package_token) = self.package_tokens.get(token) {
            if package_token.token_type != expected {
                return None;
            }
            let fallback = core_theme_value(&package_token.fallback)?;
            if fallback.token_type != expected {
                return None;
            }
            return Some(ResolvedThemeToken {
                requested_token: token.to_string(),
                core_token: package_token.fallback.clone(),
                token_type: expected,
                value: fallback.value,
            });
        }

        let core = core_theme_value(token)?;
        if core.token_type != expected {
            return None;
        }
        Some(ResolvedThemeToken {
            requested_token: token.to_string(),
            core_token: token.to_string(),
            token_type: expected,
            value: core.value,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CoreThemeValue {
    pub(crate) token_type: ThemeTokenType,
    pub(crate) value: ResolvedThemeValue,
}

pub(crate) fn core_token_type(token: &str) -> Option<ThemeTokenType> {
    core_theme_value(token).map(|value| value.token_type)
}

pub(crate) fn core_fallback_matches_type(fallback: &str, token_type: ThemeTokenType) -> bool {
    core_token_type(fallback) == Some(token_type)
}

pub(crate) fn core_theme_value(token: &str) -> Option<CoreThemeValue> {
    use ResolvedThemeValue::{
        Color as ColorValue, Density as DensityValue, Dimension as DimensionValue,
        Elevation as ElevationValue, F32, F64, MotionDuration as MotionValue, ZLevel as ZValue,
    };
    // `MotionDuration` and `ZLevel` variant names collide with the enum types of
    // the same name in this module, so those two token types are referenced
    // fully-qualified (`ThemeTokenType::MotionDuration`, `ThemeTokenType::ZLevel`).
    use ThemeTokenType::{
        ColorRole, Density, Dimension, Elevation, Opacity, Radius, Spacing, Typography,
    };

    let value = match token {
        // --- Existing color roles (unchanged) ---
        "surface.panel" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x21, 0x20, 0x2b)),
        },
        "surface.overlay" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x18, 0x17, 0x20)),
        },
        "surface.main" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x10, 0x0f, 0x17)),
        },
        "surface.control" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x39, 0x35, 0x4a)),
        },
        "surface.list" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x29, 0x28, 0x35)),
        },
        "surface.selected" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3d, 0x38, 0x5c)),
        },
        "text.primary" | "border.strong" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xee, 0xea, 0xff)),
        },
        "text.muted" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xb9, 0xb2, 0xcf)),
        },
        "accent.primary" | "border.focus" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x7c, 0x6f, 0xff)),
        },
        "diagnostic.error" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xff, 0x6b, 0x6b)),
        },
        // --- Phase 20.1: state/border/focus/muted color roles ---
        "surface.hover" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x2d, 0x2b, 0x3d)),
        },
        "surface.active" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x34, 0x31, 0x47)),
        },
        "surface.disabled" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x1b, 0x1a, 0x24)),
        },
        "text.disabled" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x80, 0x7a, 0x9b)),
        },
        "accent.muted" => CoreThemeValue {
            token_type: ColorRole,
            // The accent at 75 %, the ladder every shipped theme uses: quiet
            // enough to read as a secondary signal, still >= 3:1 on the canvas.
            value: ColorValue(Color::from_rgba8(0x7c, 0x6f, 0xff, 0xbf)),
        },
        "focus.ring" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x96, 0x8a, 0xff)),
        },
        // Border ladder (`DESIGN.md` §10.1): the border grey at 34 % for the
        // decorative hairline, the same grey at 100 % for the structural
        // boundary, and ink for explicit separators. The grey is mid-tone so
        // `border.subtle` clears 3:1 on both the canvas and the panel; the
        // hairline is required only to stay visible (>= 1.2:1).
        "border.hairline" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgba8(0x72, 0x6b, 0x98, 0x57)),
        },
        "border.subtle" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x72, 0x6b, 0x98)),
        },
        "diagnostic.warning" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xff, 0xc6, 0x6b)),
        },
        "diagnostic.info" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x6b, 0xb2, 0xff)),
        },
        "diagnostic.success" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x6b, 0xcf, 0x9b)),
        },
        // --- Existing spacing/radius/typography/opacity (unchanged) ---
        "spacing.none" => CoreThemeValue {
            token_type: Spacing,
            value: F64(0.0),
        },
        "spacing.inline" => CoreThemeValue {
            token_type: Spacing,
            value: F64(6.0),
        },
        "spacing.panel" => CoreThemeValue {
            token_type: Spacing,
            value: F64(14.0),
        },
        "spacing.row" => CoreThemeValue {
            token_type: Spacing,
            value: F64(26.0),
        },
        // Phase 20.1: explicit 4pt spacing scale (4/8/12/16/24/32/48).
        "spacing.xxs" | "spacing.badge" => CoreThemeValue {
            token_type: Spacing,
            value: F64(4.0),
        },
        "spacing.xs" | "spacing.tooltip" => CoreThemeValue {
            token_type: Spacing,
            value: F64(8.0),
        },
        "spacing.sm" => CoreThemeValue {
            token_type: Spacing,
            value: F64(12.0),
        },
        "spacing.md" => CoreThemeValue {
            token_type: Spacing,
            value: F64(16.0),
        },
        "spacing.lg" => CoreThemeValue {
            token_type: Spacing,
            value: F64(24.0),
        },
        "spacing.xl" => CoreThemeValue {
            token_type: Spacing,
            value: F64(32.0),
        },
        "spacing.xxl" => CoreThemeValue {
            token_type: Spacing,
            value: F64(48.0),
        },
        "radius.none" => CoreThemeValue {
            token_type: Radius,
            value: F64(0.0),
        },
        "radius.panel" => CoreThemeValue {
            token_type: Radius,
            value: F64(6.0),
        },
        // Phase 20.1: restrained extra radii.
        "radius.xs" => CoreThemeValue {
            token_type: Radius,
            value: F64(2.0),
        },
        "radius.sm" => CoreThemeValue {
            token_type: Radius,
            value: F64(4.0),
        },
        "radius.lg" => CoreThemeValue {
            token_type: Radius,
            value: F64(8.0),
        },
        "typography.body" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Body),
        },
        "typography.title" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Title),
        },
        "typography.status" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Status),
        },
        // Phase 20.1 additive semantic variants; defaults resolve the same
        // legacy scale ratios until a user-owned hierarchy overrides them.
        "typography.display" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Display),
        },
        "typography.section" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Section),
        },
        "typography.detail" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Detail),
        },
        "typography.caption" => CoreThemeValue {
            token_type: Typography,
            value: ResolvedThemeValue::Typography(UiTextVariant::Caption),
        },
        "opacity.disabled" => CoreThemeValue {
            token_type: Opacity,
            value: F32(0.55),
        },
        "opacity.full" => CoreThemeValue {
            token_type: Opacity,
            value: F32(1.0),
        },
        // --- Phase 20.1: typed dimensions for panel/border defaults ---
        "dimension.border.hairline" | "dimension.border.thin" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(1.0),
        },
        "dimension.border.thick" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(2.0),
        },
        "dimension.panel.side.default" | "dimension.sidebar.default" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(244.0),
        },
        "dimension.panel.side.min" | "dimension.panel.vertical.min" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(48.0),
        },
        "dimension.panel.side.max" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(480.0),
        },
        "dimension.panel.vertical.default" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(120.0),
        },
        "dimension.panel.vertical.max" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(240.0),
        },
        // Mid-width windows keep a narrower sidebar so the reading measure
        // survives (DESIGN.md §5; plan 118 task E1).
        "dimension.sidebar.compact" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(224.0),
        },
        // --- Phase 20.1: near-invisible elevation levels ---
        "elevation.none" => CoreThemeValue {
            token_type: Elevation,
            value: ElevationValue(ElevationLevel::None),
        },
        "elevation.raised" => CoreThemeValue {
            token_type: Elevation,
            value: ElevationValue(ElevationLevel::Raised),
        },
        "elevation.overlay" => CoreThemeValue {
            token_type: Elevation,
            value: ElevationValue(ElevationLevel::Overlay),
        },
        // --- Phase 20.1: bounded motion durations ---
        "motion.instant" => CoreThemeValue {
            token_type: ThemeTokenType::MotionDuration,
            value: MotionValue(MotionDuration::const_millis(0.0)),
        },
        "motion.fast" => CoreThemeValue {
            token_type: ThemeTokenType::MotionDuration,
            value: MotionValue(MotionDuration::const_millis(100.0)),
        },
        "motion.normal" => CoreThemeValue {
            token_type: ThemeTokenType::MotionDuration,
            value: MotionValue(MotionDuration::const_millis(200.0)),
        },
        "motion.slow" => CoreThemeValue {
            token_type: ThemeTokenType::MotionDuration,
            value: MotionValue(MotionDuration::const_millis(400.0)),
        },
        // --- Phase 20.1: ordered overlay z-levels ---
        "z.base" => CoreThemeValue {
            token_type: ThemeTokenType::ZLevel,
            value: ZValue(ZLevel::Base),
        },
        "z.panel" => CoreThemeValue {
            token_type: ThemeTokenType::ZLevel,
            value: ZValue(ZLevel::Panel),
        },
        "z.overlay" => CoreThemeValue {
            token_type: ThemeTokenType::ZLevel,
            value: ZValue(ZLevel::Overlay),
        },
        "z.modal" => CoreThemeValue {
            token_type: ThemeTokenType::ZLevel,
            value: ZValue(ZLevel::Modal),
        },
        "z.tooltip" => CoreThemeValue {
            token_type: ThemeTokenType::ZLevel,
            value: ZValue(ZLevel::Tooltip),
        },
        // --- Phase 20.1: density intent levels ---
        "density.compact" => CoreThemeValue {
            token_type: Density,
            value: DensityValue(DensityLevel::Compact),
        },
        "density.default" => CoreThemeValue {
            token_type: Density,
            value: DensityValue(DensityLevel::Default),
        },
        "density.spacious" => CoreThemeValue {
            token_type: Density,
            value: DensityValue(DensityLevel::Spacious),
        },
        // --- Phase 20.2: primitive chrome tokens ---
        "dimension.scrollbar.width" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(8.0),
        },
        "dimension.icon.size" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(16.0),
        },
        "dimension.kbd.height" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(20.0),
        },
        "surface.badge" | "surface.kbd" | "surface.tooltip" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3c, 0x38, 0x36)),
        },
        "text.badge" | "text.tooltip" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xeb, 0xdb, 0xb2)),
        },
        "text.kbd" | "text.icon" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xa8, 0x99, 0x84)),
        },
        "border.kbd" | "surface.scrollbar" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x50, 0x49, 0x45)),
        },
        "surface.scrollbar.track" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x28, 0x28, 0x28)),
        },
        // --- Phase 24.4: centered Command Centre surface tokens ---
        "surface.scrim" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x00, 0x00, 0x00)),
        },
        "opacity.scrim" => CoreThemeValue {
            token_type: Opacity,
            value: F32(0.5),
        },
        "dimension.overlay.centered.width" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(640.0),
        },
        _ => return None,
    };
    Some(value)
}

/// Every core token name, mirroring `core_theme_value` and the Core Tokens
/// tables in `.agents/skills/clay-execution/references/tokens.md` (drift is pinned
/// by conformance tests).
pub const CORE_TOKEN_NAMES: &[&str] = &[
    "accent.muted",
    "accent.primary",
    "border.focus",
    "border.hairline",
    "border.kbd",
    "border.strong",
    "border.subtle",
    "density.compact",
    "density.default",
    "density.spacious",
    "diagnostic.error",
    "diagnostic.info",
    "diagnostic.success",
    "diagnostic.warning",
    "dimension.border.hairline",
    "dimension.border.thick",
    "dimension.border.thin",
    "dimension.icon.size",
    "dimension.kbd.height",
    "dimension.overlay.centered.width",
    "dimension.panel.side.default",
    "dimension.panel.side.max",
    "dimension.panel.side.min",
    "dimension.panel.vertical.default",
    "dimension.panel.vertical.max",
    "dimension.panel.vertical.min",
    "dimension.scrollbar.width",
    "dimension.sidebar.default",
    "elevation.none",
    "elevation.overlay",
    "elevation.raised",
    "focus.ring",
    "motion.fast",
    "motion.instant",
    "motion.normal",
    "motion.slow",
    "opacity.disabled",
    "opacity.full",
    "opacity.scrim",
    "radius.lg",
    "radius.none",
    "radius.panel",
    "radius.sm",
    "radius.xs",
    "spacing.badge",
    "spacing.inline",
    "spacing.lg",
    "spacing.md",
    "spacing.none",
    "spacing.panel",
    "spacing.row",
    "spacing.sm",
    "spacing.tooltip",
    "spacing.xl",
    "spacing.xs",
    "spacing.xxl",
    "spacing.xxs",
    "surface.active",
    "surface.badge",
    "surface.control",
    "surface.disabled",
    "surface.hover",
    "surface.kbd",
    "surface.list",
    "surface.main",
    "surface.overlay",
    "surface.panel",
    "surface.scrim",
    "surface.scrollbar",
    "surface.scrollbar.track",
    "surface.selected",
    "surface.tooltip",
    "text.badge",
    "text.disabled",
    "text.icon",
    "text.kbd",
    "text.muted",
    "text.primary",
    "text.tooltip",
    "typography.body",
    "typography.caption",
    "typography.detail",
    "typography.display",
    "typography.section",
    "typography.status",
    "typography.title",
    "z.base",
    "z.modal",
    "z.overlay",
    "z.panel",
    "z.tooltip",
];
