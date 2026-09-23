//! Resolution and compositing of validated theme tokens into paint/layout
//! values: the SDUI style projection, the cached registry, panel geometry, and
//! the flat token snapshot the React client installs (plan 133 task 7 split of
//! `src/shell/theme.rs`).

use std::collections::BTreeMap;

use crate::color::Color;

use crate::editor::typography::UiTextVariant;
use crate::shell::layout::FixedSlotId;

use super::*;

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

pub(crate) fn resolve_f64(
    resolver: &ThemeTokenResolver,
    token: &str,
    token_type: ThemeTokenType,
) -> f64 {
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
/// the accessors without parsing strings or allocating maps. Legacy `textStyles`
/// colors are layered below typed overrides through `with_base_ui` so existing
/// themes also drive modern state/focus/feedback tokens.
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
    pub(crate) fn resolved(&self, token: &str) -> Option<ResolvedThemeValue> {
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
    /// This compatibility projection keeps legacy `textStyles` themes coherent
    /// across modern state/focus/feedback tokens until a theme supplies a typed
    /// `designTokens` override. Low-contrast legacy placeholders are promoted to
    /// the base text color for UI muted text; the editor's own placeholder color
    /// remains unchanged in `StyleRegistry`.
    fn base_color(&self, token: &str) -> Option<Color> {
        let base = self.base_ui.as_ref()?;
        let muted_text = if crate::editor::theme::contrast_ratio(base.placeholder, base.panel_bg)
            >= TEXT_CONTRAST_MIN
        {
            base.placeholder
        } else {
            base.text
        };
        Some(match token {
            "surface.main" => base.shell_bg,
            "surface.panel" | "surface.list" | "surface.overlay" | "surface.tooltip" => {
                base.panel_bg
            }
            "surface.control" | "surface.badge" | "surface.kbd" => base.status_bg,
            "surface.selected" | "surface.hover" | "surface.active" => base.selection,
            "surface.disabled" => base.panel_bg,
            "surface.scrollbar" => base.scrollbar,
            "surface.scrollbar.track" => base.scrollbar_track,
            "text.primary" | "text.tooltip" => base.text,
            "text.muted" => muted_text,
            "text.disabled" | "accent.muted" | "text.icon" | "border.kbd" => base.placeholder,
            "text.badge" | "text.kbd" => base.status_text,
            // The accent and the border ladder come from their own keys when a
            // theme declares them, and keep the pre-vocabulary projection
            // (caret / scrollbar) when it does not — so nothing shipped changes
            // and a legacy theme can still express both (plan 118 task E7).
            "accent.primary" | "focus.ring" | "border.focus" => base.accent.unwrap_or(base.caret),
            "border.hairline" => base.border_hairline.unwrap_or(base.scrollbar),
            "border.subtle" => base.border_subtle.unwrap_or(base.scrollbar),
            "border.strong" => base.border_strong.unwrap_or(base.scrollbar),
            "diagnostic.error" => base.diagnostic_error,
            "diagnostic.warning" => base.diagnostic_warning,
            "diagnostic.info" => base.diagnostic_info,
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

// ---------------------------------------------------------------------------
// Public resolved-theme snapshot for the React client (Plan 097 Phase 4)
//
// The Tauri bridge resolves the active [`ActiveTheme`] once per install into
// a flat token map and projects it into CSS custom properties. Resolution,
// validation, and level naming stay Rust authority; the frontend adapter only
// performs the mechanical name → `--clay-*` projection.

/// Wire form of one resolved typed token value.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum ThemeTokenValueDto {
    /// CSS-ready `#rrggbb` (opaque) or `#rrggbbaa`.
    Color(String),
    /// Bounded non-negative scalar (spacing/radius/dimension/motion ms).
    Scalar(f64),
    /// Opacity `[0, 1]`.
    Opacity(f32),
    /// Validated level name (elevation/z-level/density).
    Level(String),
    /// Semantic text variant name (`body`, `title`, …).
    Variant(String),
}

fn color_to_css(color: Color) -> String {
    let rgba = color.to_rgba8();
    if rgba.a == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

impl ResolvedThemeValue {
    fn into_dto(self) -> ThemeTokenValueDto {
        match self {
            Self::Color(color) => ThemeTokenValueDto::Color(color_to_css(color)),
            Self::F64(value) | Self::Dimension(value) => ThemeTokenValueDto::Scalar(value),
            Self::F32(value) => ThemeTokenValueDto::Opacity(value),
            Self::MotionDuration(duration) => ThemeTokenValueDto::Scalar(duration.as_millis()),
            Self::Typography(variant) => ThemeTokenValueDto::Variant(
                match variant {
                    UiTextVariant::Body => "body",
                    UiTextVariant::Status => "status",
                    UiTextVariant::Title => "title",
                    UiTextVariant::Detail => "detail",
                    UiTextVariant::Display => "display",
                    UiTextVariant::Section => "section",
                    UiTextVariant::Caption => "caption",
                }
                .to_string(),
            ),
            Self::Elevation(level) => ThemeTokenValueDto::Level(
                match level {
                    ElevationLevel::None => "none",
                    ElevationLevel::Raised => "raised",
                    ElevationLevel::Overlay => "overlay",
                }
                .to_string(),
            ),
            Self::ZLevel(level) => ThemeTokenValueDto::Level(level.as_str().to_string()),
            Self::Density(level) => ThemeTokenValueDto::Level(
                match level {
                    DensityLevel::Compact => "compact",
                    DensityLevel::Default => "default",
                    DensityLevel::Spacious => "spacious",
                }
                .to_string(),
            ),
        }
    }
}

/// Resolve the complete core-token surface for an [`ActiveTheme`] snapshot:
/// design-token overrides first, then the legacy editor base palette
/// projection, then core fallbacks — exactly the layering the native client
/// paints with. Contrast validation runs first so a below-AA theme never
/// produces a snapshot.
pub fn resolve_theme_token_snapshot(
    snapshot: &crate::protocol::ActiveTheme,
) -> Result<std::collections::BTreeMap<String, ThemeTokenValueDto>, ContrastFailure> {
    validate_active_theme_contrast(snapshot)?;
    let base = crate::editor::theme::StyleRegistry::from_active_theme(snapshot).base;
    let resolved = ResolvedUiTheme::from_active_theme(&snapshot.design_tokens)
        .map_err(|_| ContrastFailure {
            foreground: "<malformed override>",
            background: "<malformed override>",
            ratio: 0.0,
            threshold: TEXT_CONTRAST_MIN,
        })?
        .with_base_ui(&base);
    let mut map = std::collections::BTreeMap::new();
    for name in CORE_TOKEN_NAMES {
        let Some(value) = resolved.resolved(name) else {
            continue;
        };
        map.insert((*name).to_string(), value.into_dto());
    }
    Ok(map)
}

/// Spacing rhythm multiplier for the active density level: `0.875` compact /
/// `1.0` default / `1.125` spacious. Read from a resolved snapshot map.
pub fn density_spacing_scale(
    tokens: &std::collections::BTreeMap<String, ThemeTokenValueDto>,
) -> f64 {
    match tokens.get("density.default") {
        Some(ThemeTokenValueDto::Level(level)) => match level.as_str() {
            "compact" => 0.875,
            "spacious" => 1.125,
            _ => 1.0,
        },
        _ => 1.0,
    }
}
