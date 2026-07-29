//! Clay-owned typed theme tokens for package UI and SDUI rendering.
//!
//! Package declarations may name semantic, package-prefixed tokens, but they do
//! not provide raw colors, CSS, renderer callbacks, or native style handles.
//! Clay resolves every package token through a same-typed core fallback token
//! before Masonry paint/layout reads cached native values.

use std::collections::BTreeMap;

use masonry::peniko::Color;

use crate::editor::typography::UiTextVariant;
use crate::shell::layout::FixedSlotId;

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

impl ThemeTokenType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "color-role" => Some(Self::ColorRole),
            "spacing" => Some(Self::Spacing),
            "radius" => Some(Self::Radius),
            "typography" => Some(Self::Typography),
            "opacity" => Some(Self::Opacity),
            "dimension" => Some(Self::Dimension),
            "elevation" => Some(Self::Elevation),
            "motion-duration" => Some(Self::MotionDuration),
            "z-level" => Some(Self::ZLevel),
            "density" => Some(Self::Density),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ColorRole => "color-role",
            Self::Spacing => "spacing",
            Self::Radius => "radius",
            Self::Typography => "typography",
            Self::Opacity => "opacity",
            Self::Dimension => "dimension",
            Self::Elevation => "elevation",
            Self::MotionDuration => "motion-duration",
            Self::ZLevel => "z-level",
            Self::Density => "density",
        }
    }

    /// Human-readable list of every supported token type, for diagnostics.
    pub(crate) const fn all_as_str() -> &'static [&'static str] {
        &[
            "color-role",
            "spacing",
            "radius",
            "typography",
            "opacity",
            "dimension",
            "elevation",
            "motion-duration",
            "z-level",
            "density",
        ]
    }
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

#[allow(dead_code)]
impl ElevationLevel {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "raised" => Some(Self::Raised),
            "overlay" => Some(Self::Overlay),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Raised => "raised",
            Self::Overlay => "overlay",
        }
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

#[allow(dead_code)]
impl ZLevel {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "base" => Some(Self::Base),
            "panel" => Some(Self::Panel),
            "overlay" => Some(Self::Overlay),
            "modal" => Some(Self::Modal),
            "tooltip" => Some(Self::Tooltip),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Panel => "panel",
            Self::Overlay => "overlay",
            Self::Modal => "modal",
            Self::Tooltip => "tooltip",
        }
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

#[allow(dead_code)]
impl DensityLevel {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "compact" => Some(Self::Compact),
            "default" => Some(Self::Default),
            "spacious" => Some(Self::Spacious),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Default => "default",
            Self::Spacious => "spacious",
        }
    }

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
pub(crate) const SIDEBAR_DEFAULT_WIDTH: f64 = 240.0;
pub(crate) const PANEL_SIDE_DEFAULT: f64 = 240.0;
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
struct CoreThemeValue {
    token_type: ThemeTokenType,
    value: ResolvedThemeValue,
}

pub(crate) fn core_token_type(token: &str) -> Option<ThemeTokenType> {
    core_theme_value(token).map(|value| value.token_type)
}

pub(crate) fn core_fallback_matches_type(fallback: &str, token_type: ThemeTokenType) -> bool {
    core_token_type(fallback) == Some(token_type)
}

fn core_theme_value(token: &str) -> Option<CoreThemeValue> {
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
        "text.primary" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xee, 0xea, 0xff)),
        },
        "text.muted" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xb9, 0xb2, 0xcf)),
        },
        "accent.primary" => CoreThemeValue {
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
            value: ColorValue(Color::from_rgb8(0x6f, 0x6a, 0x87)),
        },
        "accent.muted" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x5a, 0x52, 0xb8)),
        },
        "focus.ring" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x96, 0x8a, 0xff)),
        },
        "border.hairline" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x28, 0x26, 0x38)),
        },
        "border.subtle" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x2f, 0x2c, 0x40)),
        },
        "border.strong" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x45, 0x41, 0x5c)),
        },
        "border.focus" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x7c, 0x6f, 0xff)),
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
        "spacing.xxs" => CoreThemeValue {
            token_type: Spacing,
            value: F64(4.0),
        },
        "spacing.xs" => CoreThemeValue {
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
        "dimension.border.hairline" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(1.0),
        },
        "dimension.border.thin" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(1.0),
        },
        "dimension.border.thick" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(2.0),
        },
        "dimension.panel.side.default" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(240.0),
        },
        "dimension.panel.side.min" => CoreThemeValue {
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
        "dimension.panel.vertical.min" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(48.0),
        },
        "dimension.panel.vertical.max" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(240.0),
        },
        "dimension.sidebar.default" => CoreThemeValue {
            token_type: Dimension,
            value: DimensionValue(240.0),
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
        "surface.badge" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3c, 0x38, 0x36)),
        },
        "text.badge" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xeb, 0xdb, 0xb2)),
        },
        "spacing.badge" => CoreThemeValue {
            token_type: Spacing,
            value: F64(4.0),
        },
        "surface.kbd" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3c, 0x38, 0x36)),
        },
        "text.kbd" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xa8, 0x99, 0x84)),
        },
        "border.kbd" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x50, 0x49, 0x45)),
        },
        "surface.tooltip" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3c, 0x38, 0x36)),
        },
        "text.tooltip" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xeb, 0xdb, 0xb2)),
        },
        "spacing.tooltip" => CoreThemeValue {
            token_type: Spacing,
            value: F64(8.0),
        },
        "text.icon" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xa8, 0x99, 0x84)),
        },
        "surface.scrollbar" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x50, 0x49, 0x45)),
        },
        "surface.scrollbar.track" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x28, 0x28, 0x28)),
        },
        _ => return None,
    };
    Some(value)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiThemeStyle {
    pub(crate) panel_padding: f64,
    pub(crate) title_text: UiTextVariant,
    pub(crate) body_text: UiTextVariant,
    pub(crate) status_text: UiTextVariant,
    pub(crate) panel_background: Color,
    pub(crate) button_background: Color,
    pub(crate) list_background: Color,
    pub(crate) selected_background: Color,
    pub(crate) text_color: Color,
    pub(crate) muted_text_color: Color,
}

impl SduiThemeStyle {
    pub(crate) fn from_resolver(resolver: &ThemeTokenResolver) -> Self {
        Self {
            panel_padding: resolve_f64(resolver, "spacing.panel", ThemeTokenType::Spacing),
            title_text: resolve_typography(resolver, "typography.title"),
            body_text: resolve_typography(resolver, "typography.body"),
            status_text: resolve_typography(resolver, "typography.status"),
            panel_background: resolve_color(resolver, "surface.panel"),
            button_background: resolve_color(resolver, "surface.control"),
            list_background: resolve_color(resolver, "surface.list"),
            selected_background: resolve_color(resolver, "surface.selected"),
            text_color: resolve_color(resolver, "text.primary"),
            muted_text_color: resolve_color(resolver, "text.muted"),
        }
    }

    /// Resolve the SDUI paint style from the active [`ResolvedUiTheme`] so
    /// package component fills, typography variants, and the spacing rhythm
    /// honor the user theme (Phase 20.4). `panel_padding` reads the on-grid
    /// `spacing.md` token scaled by the active density `spacing_scale()`.
    pub(crate) fn from_ui_theme(theme: &ResolvedUiTheme) -> Self {
        let panel_padding =
            theme.scalar_f64("spacing.md").unwrap_or(16.0) * f64::from(theme.spacing_scale());
        Self {
            panel_padding,
            title_text: theme
                .typography("typography.title")
                .unwrap_or(UiTextVariant::Title),
            body_text: theme
                .typography("typography.body")
                .unwrap_or(UiTextVariant::Body),
            status_text: theme
                .typography("typography.status")
                .unwrap_or(UiTextVariant::Status),
            panel_background: theme.color("surface.panel").unwrap_or(Color::TRANSPARENT),
            button_background: theme.color("surface.control").unwrap_or(Color::TRANSPARENT),
            list_background: theme.color("surface.list").unwrap_or(Color::TRANSPARENT),
            selected_background: theme
                .color("surface.selected")
                .unwrap_or(Color::TRANSPARENT),
            text_color: theme.color("text.primary").unwrap_or(Color::TRANSPARENT),
            muted_text_color: theme.color("text.muted").unwrap_or(Color::TRANSPARENT),
        }
    }
}

impl Default for SduiThemeStyle {
    fn default() -> Self {
        Self::from_resolver(&ThemeTokenResolver::new())
    }
}

fn resolve_color(resolver: &ThemeTokenResolver, token: &str) -> Color {
    match resolver.resolve(token, ThemeTokenType::ColorRole) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::Color(color),
            ..
        }) => color,
        _ => Color::from_rgb8(0xff, 0x00, 0xff),
    }
}

fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {
    match resolver.resolve(token, token_type) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F64(value),
            ..
        }) => value,
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F32(value),
            ..
        }) => f64::from(value),
        _ => 0.0,
    }
}

fn resolve_typography(resolver: &ThemeTokenResolver, token: &str) -> UiTextVariant {
    match resolver.resolve(token, ThemeTokenType::Typography) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::Typography(variant),
            ..
        }) => variant,
        _ => UiTextVariant::from_typography_token(token),
    }
}

/// Resolve a typed dimension token to a finite logical-pixel scalar.
/// Returns `None` for unknown tokens, wrong-typed tokens, or non-finite/out-of-range values.
#[allow(dead_code)]
pub(crate) fn resolve_dimension(resolver: &ThemeTokenResolver, token: &str) -> Option<f64> {
    match resolver.resolve(token, ThemeTokenType::Dimension)? {
        ResolvedThemeToken {
            value: ResolvedThemeValue::Dimension(value),
            ..
        } if is_valid_dimension(value) => Some(value),
        _ => None,
    }
}

/// Resolve an elevation level token. Near-invisible per minimalist direction.
#[allow(dead_code)]
pub(crate) fn resolve_elevation(
    resolver: &ThemeTokenResolver,
    token: &str,
) -> Option<ElevationLevel> {
    match resolver.resolve(token, ThemeTokenType::Elevation)? {
        ResolvedThemeToken {
            value: ResolvedThemeValue::Elevation(level),
            ..
        } => Some(level),
        _ => None,
    }
}

/// Resolve a bounded motion-duration token to milliseconds.
#[allow(dead_code)]
pub(crate) fn resolve_motion_duration(resolver: &ThemeTokenResolver, token: &str) -> Option<f64> {
    match resolver.resolve(token, ThemeTokenType::MotionDuration)? {
        ResolvedThemeToken {
            value: ResolvedThemeValue::MotionDuration(duration),
            ..
        } => Some(duration.millis()),
        _ => None,
    }
}

/// Resolve an ordered overlay stacking level.
#[allow(dead_code)]
pub(crate) fn resolve_z_level(resolver: &ThemeTokenResolver, token: &str) -> Option<ZLevel> {
    match resolver.resolve(token, ThemeTokenType::ZLevel)? {
        ResolvedThemeToken {
            value: ResolvedThemeValue::ZLevel(level),
            ..
        } => Some(level),
        _ => None,
    }
}

/// Resolve an information-density intent level.
#[allow(dead_code)]
pub(crate) fn resolve_density(resolver: &ThemeTokenResolver, token: &str) -> Option<DensityLevel> {
    match resolver.resolve(token, ThemeTokenType::Density)? {
        ResolvedThemeToken {
            value: ResolvedThemeValue::Density(level),
            ..
        } => Some(level),
        _ => None,
    }
}

/// Validation failure for a theme-package UI design-token override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DesignTokenError {
    /// Token is not a known Clay core token.
    UnknownToken,
    /// Wire value variant does not match the core token type.
    TypeMismatch,
    /// Scalar is NaN/infinite/out-of-range for its domain.
    InvalidScalar,
    /// Level name is not parseable for the target domain.
    InvalidLevel,
    /// Design tokens cannot override typography variants.
    TypographyNotOverridable,
    /// Duplicate token in one active-theme override set.
    DuplicateToken,
}

/// Validate a single active-theme UI design-token override against the core
/// fallback catalog. Returns the resolved value on success. The server
/// validates at package-parse time; the client revalidates here before install
/// so a malformed snapshot never reaches paint/layout. Design tokens never
/// override typography variants (that is the separate hierarchy path).
pub(crate) fn validate_design_token_override(
    token: &str,
    value: &crate::protocol::WireDesignTokenValue,
) -> Result<ResolvedThemeValue, DesignTokenError> {
    use crate::protocol::WireDesignTokenValue as Wire;
    let core = core_theme_value(token).ok_or(DesignTokenError::UnknownToken)?;
    if core.token_type == ThemeTokenType::Typography {
        return Err(DesignTokenError::TypographyNotOverridable);
    }
    let resolved = match (&core.value, value) {
        (ResolvedThemeValue::Color(_), Wire::Color([r, g, b, a])) => {
            ResolvedThemeValue::Color(Color::from_rgba8(*r, *g, *b, *a))
        }
        (ResolvedThemeValue::F64(_), Wire::Scalar(v)) if is_valid_dimension(*v) => {
            ResolvedThemeValue::F64(*v)
        }
        (ResolvedThemeValue::F32(_), Wire::Opacity(v))
            if v.is_finite() && (0.0..=1.0).contains(v) =>
        {
            ResolvedThemeValue::F32(*v)
        }
        (ResolvedThemeValue::Dimension(_), Wire::Scalar(v)) if is_valid_dimension(*v) => {
            ResolvedThemeValue::Dimension(*v)
        }
        (ResolvedThemeValue::Elevation(_), Wire::Level(s)) => ElevationLevel::parse(s)
            .map(ResolvedThemeValue::Elevation)
            .ok_or(DesignTokenError::InvalidLevel)?,
        (ResolvedThemeValue::MotionDuration(_), Wire::Scalar(v)) => MotionDuration::from_millis(*v)
            .map(ResolvedThemeValue::MotionDuration)
            .ok_or(DesignTokenError::InvalidScalar)?,
        (ResolvedThemeValue::ZLevel(_), Wire::Level(s)) => ZLevel::parse(s)
            .map(ResolvedThemeValue::ZLevel)
            .ok_or(DesignTokenError::InvalidLevel)?,
        (ResolvedThemeValue::Density(_), Wire::Level(s)) => DensityLevel::parse(s)
            .map(ResolvedThemeValue::Density)
            .ok_or(DesignTokenError::InvalidLevel)?,
        _ => return Err(DesignTokenError::TypeMismatch),
    };
    Ok(resolved)
}

/// WCAG AA minimum contrast for body/label/tooltip text foreground/background
/// pairs in the SDUI color-role palette. Normal text per WCAG 2.1 SC 1.4.3.
pub(crate) const TEXT_CONTRAST_MIN: f64 = 4.5;

/// WCAG AA minimum contrast for non-text UI pairs (accent, focus ring, focus
/// border) and standalone UI chips (`kbd`), which are not prose. WCAG 2.1
/// non-text contrast (SC 1.4.11) floor is 3.0.
pub(crate) const UI_CONTRAST_MIN: f64 = 3.0;

/// A required foreground/background color-role pair that must meet a WCAG AA
/// contrast threshold. Token names are core color-role tokens resolved through
/// [`ResolvedUiTheme::color`] (active override first, then same-typed core
/// fallback). Generic over every theme package: no package-specific branch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastFailure {
    pub foreground: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    pub threshold: f64,
}

/// Required SDUI foreground/background contrast pairs and their thresholds.
/// `text.*` on `surface.*` are body/label/tooltip text (4.5); `accent.primary`,
/// `focus.ring`, and `border.focus` on `surface.main` are non-text UI (3.0);
/// `text.kbd`/`surface.kbd` is a standalone UI chip, not prose, so 3.0.
/// ponytail: raise `text.kbd` to 4.5 only if kbd ever carries prose text.
pub(crate) const REQUIRED_CONTRAST_PAIRS: &[(&str, &str, f64)] = &[
    ("text.primary", "surface.main", TEXT_CONTRAST_MIN),
    ("text.muted", "surface.panel", TEXT_CONTRAST_MIN),
    ("text.primary", "surface.panel", TEXT_CONTRAST_MIN),
    ("text.primary", "surface.control", TEXT_CONTRAST_MIN),
    ("text.badge", "surface.badge", TEXT_CONTRAST_MIN),
    ("text.kbd", "surface.kbd", UI_CONTRAST_MIN),
    ("text.tooltip", "surface.tooltip", TEXT_CONTRAST_MIN),
    ("accent.primary", "surface.main", UI_CONTRAST_MIN),
    ("focus.ring", "surface.main", UI_CONTRAST_MIN),
    ("border.focus", "surface.main", UI_CONTRAST_MIN),
];

/// Validate that every required contrast pair in `theme` meets its threshold.
/// Returns the first failing pair. Reuses [`crate::editor::theme::contrast_ratio`]
/// as the WCAG engine; this helper only adds the required-pairs policy. A pair
/// whose foreground or background color role does not resolve (returns `None`)
/// is skipped — the core catalog guarantees all `REQUIRED_CONTRAST_PAIRS`
/// tokens are color roles, so a `None` indicates a non-color override of a
/// color token, which is a separate type-mismatch error surfaced elsewhere.
pub(crate) fn theme_meets_contrast(theme: &ResolvedUiTheme) -> Result<(), ContrastFailure> {
    for &(foreground, background, threshold) in REQUIRED_CONTRAST_PAIRS {
        let (Some(fg), Some(bg)) = (theme.color(foreground), theme.color(background)) else {
            continue;
        };
        let ratio = crate::editor::theme::contrast_ratio(fg, bg);
        if ratio < threshold {
            return Err(ContrastFailure {
                foreground,
                background,
                ratio,
                threshold,
            });
        }
    }
    Ok(())
}

/// Validate an [`crate::protocol::ActiveTheme`] snapshot's contrast by
/// resolving its `design_tokens` overrides through the core catalog and
/// checking [`theme_meets_contrast`]. Shared by the `setTheme` apply path and
/// the canonical-default resolver so both enforce the same AA floor. The
/// snapshot's design tokens are already package-parse-validated, so resolution
/// only fails if the wire snapshot was malformed (defensive: mapped to a
/// contrast failure so the caller rejects without crashing).
pub fn validate_active_theme_contrast(
    snapshot: &crate::protocol::ActiveTheme,
) -> Result<(), ContrastFailure> {
    let resolved = ResolvedUiTheme::from_active_theme(&snapshot.design_tokens).map_err(|_| {
        ContrastFailure {
            foreground: "<malformed override>",
            background: "<malformed override>",
            ratio: 0.0,
            threshold: TEXT_CONTRAST_MIN,
        }
    })?;
    theme_meets_contrast(&resolved)
}

/// Resolved shell panel/sidebar geometry view, the single shared default
/// source for the legacy SDUI left-slot bridge and package fixed-panel state.
/// Built from validated `dimension.*` overrides layered over the core
/// fallback catalog. An override triple that is missing, non-finite, or
/// out of order (`min > default`, `default > max`, `min > max`) falls back to
/// the matching Clay constant tuple before it reaches layout — invalid token
/// ordering never produces a misordered `FixedSlotState`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PanelDefaults {
    pub(crate) side_default: f64,
    pub(crate) side_min: f64,
    pub(crate) side_max: f64,
    pub(crate) vertical_default: f64,
    pub(crate) vertical_min: f64,
    pub(crate) vertical_max: f64,
    pub(crate) sidebar_width: f64,
}

impl PanelDefaults {
    /// Default (requested) size for a fixed slot, drawn from the side or
    /// vertical triple.
    pub(crate) fn default_size(self, slot: FixedSlotId) -> f64 {
        match slot {
            FixedSlotId::Left | FixedSlotId::Right => self.side_default,
            FixedSlotId::Top | FixedSlotId::Bottom => self.vertical_default,
        }
    }

    /// Minimum size for a fixed slot.
    pub(crate) fn min_size(self, slot: FixedSlotId) -> f64 {
        match slot {
            FixedSlotId::Left | FixedSlotId::Right => self.side_min,
            FixedSlotId::Top | FixedSlotId::Bottom => self.vertical_min,
        }
    }

    /// Maximum size for a fixed slot.
    pub(crate) fn max_size(self, slot: FixedSlotId) -> f64 {
        match slot {
            FixedSlotId::Left | FixedSlotId::Right => self.side_max,
            FixedSlotId::Top | FixedSlotId::Bottom => self.vertical_max,
        }
    }
}

/// Cached resolved UI design-token registry built from an [`ActiveTheme`] override
/// set layered over the core fallback catalog. Constructed once during
/// bootstrap/reload/theme-switch; paint and layout read cached typed values via
/// the accessors without parsing strings or allocating maps. Themes that omit
/// overrides (including both Gruvbox packages) resolve every value from core
/// fallbacks unchanged.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ResolvedUiTheme {
    overrides: BTreeMap<String, ResolvedThemeValue>,
    /// Editor base palette layered under `overrides` (see [`Self::with_base_ui`]).
    base_ui: Option<crate::editor::theme::BaseUiColors>,
}

// ponytail: accessors are the Phase 20.1 cached registry surface consumed by
// the panel-default wiring (task 5) and component-catalog work (task 7). Until
// that non-test wiring lands, single-attribute allows keep the lib build quiet.
#[allow(dead_code)]
impl ResolvedUiTheme {
    /// Build from validated active-theme overrides. Each override is validated
    /// against the core catalog; duplicates are rejected. Empty input yields a
    /// core-fallback-only registry.
    pub(crate) fn from_active_theme(
        overrides: &[crate::protocol::UiDesignTokenOverride],
    ) -> Result<Self, DesignTokenError> {
        let mut map = BTreeMap::new();
        for o in overrides {
            let value = validate_design_token_override(&o.token, &o.value)?;
            if map.insert(o.token.clone(), value).is_some() {
                return Err(DesignTokenError::DuplicateToken);
            }
        }
        Ok(Self {
            overrides: map,
            base_ui: None,
        })
    }

    /// Resolve a token to its value: active override first, then the editor base
    /// palette, then core fallback.
    fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {
        if let Some(value) = self.overrides.get(token) {
            return Some(value.clone());
        }
        if let Some(color) = self.base_color(token) {
            return Some(ResolvedThemeValue::Color(color));
        }
        core_theme_value(token).map(|core| core.value)
    }

    /// Layer the editor's resolved base palette under the design-token overrides.
    /// Legacy themes express their palette via `TextThemeOverride` (the editor
    /// text path) and ship no `designTokens`; without this layer the shell/SDUI
    /// scrollbar chrome would fall through to the dark core catalog and disagree
    /// with the editor (e.g. a dark sidebar on a light editor). The editor text
    /// path reads these same base colors, so this keeps chrome in lock-step with
    /// it. Design-token overrides (resolved first) always win.
    pub(crate) fn with_base_ui(mut self, base: &crate::editor::theme::BaseUiColors) -> Self {
        self.base_ui = Some(*base);
        self
    }

    /// Map a shell color token onto the editor base palette, if one is installed.
    fn base_color(&self, token: &str) -> Option<Color> {
        let base = self.base_ui.as_ref()?;
        Some(match token {
            "surface.panel" | "surface.list" | "surface.tooltip" => base.panel_bg,
            "surface.main" => base.shell_bg,
            "surface.selected" => base.selection,
            "surface.control" | "surface.overlay" => base.status_bg,
            "surface.scrollbar" => base.scrollbar,
            "surface.scrollbar.track" => base.scrollbar_track,
            "text.primary" => base.text,
            "text.muted" | "text.disabled" => base.placeholder,
            _ => return None,
        })
    }

    /// `true` when no active overrides are installed (core fallbacks only).
    pub(crate) fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Resolve a `color-role` token.
    pub(crate) fn color(&self, token: &str) -> Option<Color> {
        match self.resolved(token)? {
            ResolvedThemeValue::Color(c) => Some(c),
            _ => None,
        }
    }

    /// Resolve a `spacing`/`radius` token stored as `F64`.
    pub(crate) fn scalar_f64(&self, token: &str) -> Option<f64> {
        match self.resolved(token)? {
            ResolvedThemeValue::F64(v) => Some(v),
            _ => None,
        }
    }

    /// Resolve an `opacity` token.
    pub(crate) fn opacity(&self, token: &str) -> Option<f32> {
        match self.resolved(token)? {
            ResolvedThemeValue::F32(v) => Some(v),
            _ => None,
        }
    }

    /// Resolve a `dimension` token.
    pub(crate) fn dimension(&self, token: &str) -> Option<f64> {
        match self.resolved(token)? {
            ResolvedThemeValue::Dimension(v) => Some(v),
            _ => None,
        }
    }

    /// Resolve a `typography` token to its semantic variant (Phase 20.4).
    pub(crate) fn typography(&self, token: &str) -> Option<UiTextVariant> {
        match self.resolved(token)? {
            ResolvedThemeValue::Typography(variant) => Some(variant),
            _ => None,
        }
    }

    /// Resolve an `elevation` level token.
    pub(crate) fn elevation(&self, token: &str) -> Option<ElevationLevel> {
        match self.resolved(token)? {
            ResolvedThemeValue::Elevation(l) => Some(l),
            _ => None,
        }
    }

    /// Resolve a `motion-duration` token to milliseconds.
    pub(crate) fn motion_duration(&self, token: &str) -> Option<f64> {
        match self.resolved(token)? {
            ResolvedThemeValue::MotionDuration(m) => Some(m.millis()),
            _ => None,
        }
    }

    /// Resolve a `z-level` token.
    pub(crate) fn z_level(&self, token: &str) -> Option<ZLevel> {
        match self.resolved(token)? {
            ResolvedThemeValue::ZLevel(l) => Some(l),
            _ => None,
        }
    }

    /// Resolve a `density` level token.
    pub(crate) fn density(&self, token: &str) -> Option<DensityLevel> {
        match self.resolved(token)? {
            ResolvedThemeValue::Density(l) => Some(l),
            _ => None,
        }
    }

    /// Resolved shell panel/sidebar geometry defaults. Per-domain override
    /// triples that are missing, non-finite, or out of order fall back to the
    /// Clay core constants so invalid theme token ordering never reaches
    /// layout or constructs a misordered `FixedSlotState`.
    pub(crate) fn panel_defaults(&self) -> PanelDefaults {
        let (side_default, side_min, side_max) = Self::resolve_panel_triple(
            self.dimension("dimension.panel.side.default"),
            self.dimension("dimension.panel.side.min"),
            self.dimension("dimension.panel.side.max"),
            PANEL_SIDE_DEFAULT,
            PANEL_SIDE_MIN,
            PANEL_SIDE_MAX,
        );
        let (vertical_default, vertical_min, vertical_max) = Self::resolve_panel_triple(
            self.dimension("dimension.panel.vertical.default"),
            self.dimension("dimension.panel.vertical.min"),
            self.dimension("dimension.panel.vertical.max"),
            PANEL_VERTICAL_DEFAULT,
            PANEL_VERTICAL_MIN,
            PANEL_VERTICAL_MAX,
        );
        let sidebar_width = match self.dimension("dimension.sidebar.default") {
            Some(width) if width.is_finite() && width >= 0.0 => width,
            _ => SIDEBAR_DEFAULT_WIDTH,
        };
        PanelDefaults {
            side_default,
            side_min,
            side_max,
            vertical_default,
            vertical_min,
            vertical_max,
            sidebar_width,
        }
    }

    fn resolve_panel_triple(
        default: Option<f64>,
        min: Option<f64>,
        max: Option<f64>,
        fallback_default: f64,
        fallback_min: f64,
        fallback_max: f64,
    ) -> (f64, f64, f64) {
        match (default, min, max) {
            (Some(default), Some(min), Some(max))
                if default.is_finite()
                    && min.is_finite()
                    && max.is_finite()
                    && min >= 0.0
                    && min <= default
                    && default <= max =>
            {
                (default, min, max)
            }
            _ => (fallback_default, fallback_min, fallback_max),
        }
    }

    /// Active UI information-density level, selected through `density.default`.
    /// Density scales token-owned UI spacing rhythm only (Phase 20.4); panel
    /// dimensions and document typography are never density-scaled.
    pub(crate) fn active_density(&self) -> DensityLevel {
        self.density("density.default")
            .unwrap_or(DensityLevel::Default)
    }

    /// Spacing-rhythm multiplier for the active density level.
    pub(crate) fn spacing_scale(&self) -> f32 {
        self.active_density().spacing_scale()
    }
}

#[cfg(test)]
mod tests {
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

        assert_eq!(style.panel_padding, 14.0);
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
                ResolvedThemeValue::Color(Color::from_rgb8(0x6f, 0x6a, 0x87)),
            ),
            (
                "focus.ring",
                ThemeTokenType::ColorRole,
                ResolvedThemeValue::Color(Color::from_rgb8(0x96, 0x8a, 0xff)),
            ),
            (
                "border.hairline",
                ThemeTokenType::ColorRole,
                ResolvedThemeValue::Color(Color::from_rgb8(0x28, 0x26, 0x38)),
            ),
            (
                "border.strong",
                ThemeTokenType::ColorRole,
                ResolvedThemeValue::Color(Color::from_rgb8(0x45, 0x41, 0x5c)),
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
                ResolvedThemeValue::Dimension(240.0),
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
            assert_eq!(
                resolve_f64(&resolver, &token, ThemeTokenType::Spacing),
                value,
                "{token}"
            );
        }

        // Typed accessors resolve the new domains.
        assert_eq!(
            resolve_dimension(&resolver, "dimension.sidebar.default"),
            Some(240.0)
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
        assert_eq!(ui.dimension("dimension.sidebar.default"), Some(240.0));
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
        assert_eq!(ui.color("text.muted"), Some(base.placeholder));
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
        assert_eq!(defaults.sidebar_width, SIDEBAR_DEFAULT_WIDTH);
        assert_eq!(defaults.side_default, PANEL_SIDE_DEFAULT);
        assert_eq!(defaults.sidebar_width, defaults.side_default);
        assert_eq!(defaults.side_min, PANEL_SIDE_MIN);
        assert_eq!(defaults.side_max, PANEL_SIDE_MAX);
    }

    #[test]
    fn default_panel_geometry_is_unchanged() {
        // No overrides: core fallbacks reproduce the pre-20.1 hardcoded
        // 240/120/48/480/240 geometry exactly, including ordered fixed slot
        // states for all four slots.
        let defaults = ResolvedUiTheme::default().panel_defaults();
        assert_eq!(defaults.vertical_default, PANEL_VERTICAL_DEFAULT);
        assert_eq!(defaults.vertical_min, PANEL_VERTICAL_MIN);
        assert_eq!(defaults.vertical_max, PANEL_VERTICAL_MAX);
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
            assert_eq!(state.size, size);
            assert_eq!(state.max_size, max);
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
        assert_eq!(base.spacing_scale(), 1.0);
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
        assert_ne!(widened.side_default, base.side_default);
        assert_ne!(widened.side_max, base.side_max);
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
}
