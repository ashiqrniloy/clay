//! Clay-owned typed UI design-system recipes, theme color-role references,
//! non-color values, inheritance, and deterministic fallback resolution.
//!
//! Separates UI geometry, materials, state mappings, and motion from content-theme
//! color authority (Decision Log 2026-08-28-2234).

use crate::str_enum::string_enum_impl;

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::shell::theme::core_token_type;

/// Schema version for package UI design-system contributions.
pub const SCHEMA_VERSION: u32 = 1;

/// Maximum number of recipes allowed in a single design-system contribution.
pub const MAX_RECIPES: usize = 512;

/// Maximum number of namespaced non-color values in a single design system.
pub const MAX_VALUES: usize = 128;

/// Maximum number of shadow layers allowed per component recipe.
pub const MAX_SHADOW_LAYERS: usize = 3;

/// Maximum string length for identifiers, slot names, and variants.
pub const MAX_STRING_LEN: usize = 64;

/// Upper bound for dimensions in logical pixels.
pub const MAX_DIMENSION_PX: f64 = 8192.0;

/// Upper bound for border radius in logical pixels (9999.0 represents full-pill).
pub const MAX_RADIUS_PX: f64 = 9999.0;

/// Upper bound for standard non-pill border radius in logical pixels.
pub const MAX_STANDARD_RADIUS_PX: f64 = 32.0;

/// Upper bound for border width in logical pixels.
pub const MAX_BORDER_WIDTH_PX: f64 = 8.0;

/// Upper bound for backdrop blur in logical pixels.
pub const MAX_BLUR_PX: f64 = 32.0;

/// Minimum and maximum backdrop saturation factors.
pub const MIN_SATURATE: f64 = 1.0;
pub const MAX_SATURATE: f64 = 2.0;

/// Maximum transition motion duration in milliseconds.
pub const MAX_MOTION_MILLIS: f64 = 1000.0;

/// Structured error for UI design-system validation and recipe resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignSystemError {
    pub field: String,
    pub value: Option<String>,
    pub expected: String,
    pub message: String,
}

impl fmt::Display for DesignSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref val) = self.value {
            write!(
                f,
                "{} = `{}` rejected: expected {}; {}",
                self.field, val, self.expected, self.message
            )
        } else {
            write!(
                f,
                "{} rejected: expected {}; {}",
                self.field, self.expected, self.message
            )
        }
    }
}

impl std::error::Error for DesignSystemError {}

impl DesignSystemError {
    pub fn reject(
        field: impl Into<String>,
        value: Option<impl Into<String>>,
        expected: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            value: value.map(Into::into),
            expected: expected.into(),
            message: message.into(),
        }
    }

    pub fn invalid_version(version: u32) -> Self {
        Self::reject(
            "schema_version",
            Some(version.to_string()),
            format!("version {SCHEMA_VERSION}"),
            format!("unsupported design-system schema version {version}"),
        )
    }

    pub fn color_authority_violation(
        field: impl Into<String>,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let val = value.into();
        let reason_str = reason.into();
        Self::reject(
            field,
            Some(val),
            "semantic active-theme color-role reference (e.g. `surface.control`, `accent.primary`) or `transparent`",
            format!(
                "content themes are the sole color authority; literal colors and non-theme values are prohibited ({reason_str})"
            ),
        )
    }
}

/// A validated reference to an active content-theme color role.
///
/// Under normal rendering, design systems may never declare literal colors,
/// palettes, hex codes, RGB/HSL values, or local color aliases. All color
/// properties must reference known semantic active-theme color roles or `transparent`.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
#[serde(try_from = "String", into = "String")]
pub struct ThemeColorRef(pub String);

impl TryFrom<String> for ThemeColorRef {
    type Error = DesignSystemError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ThemeColorRef::parse(&value)
    }
}

impl TryFrom<&str> for ThemeColorRef {
    type Error = DesignSystemError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        ThemeColorRef::parse(value)
    }
}

impl From<ThemeColorRef> for String {
    fn from(r: ThemeColorRef) -> Self {
        r.0
    }
}

impl ThemeColorRef {
    /// Validates and constructs a theme color-role reference.
    pub fn parse(s: &str) -> Result<Self, DesignSystemError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(DesignSystemError::reject(
                "themeColor",
                Some(s.to_string()),
                "non-empty theme color role",
                "color-role reference cannot be empty",
            ));
        }

        // Prohibit literal hex colors
        if trimmed.starts_with('#') {
            return Err(DesignSystemError::color_authority_violation(
                "themeColor",
                s,
                "hex color literal detected",
            ));
        }

        // Prohibit rgb/rgba/hsl/hsla function strings
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("rgb(")
            || lower.starts_with("rgba(")
            || lower.starts_with("hsl(")
            || lower.starts_with("hsla(")
        {
            return Err(DesignSystemError::color_authority_violation(
                "themeColor",
                s,
                "color function literal detected",
            ));
        }

        // Prohibit common literal web color names
        if matches!(
            lower.as_str(),
            "black"
                | "white"
                | "red"
                | "green"
                | "blue"
                | "yellow"
                | "cyan"
                | "magenta"
                | "gray"
                | "grey"
                | "orange"
                | "purple"
                | "pink"
        ) {
            return Err(DesignSystemError::color_authority_violation(
                "themeColor",
                s,
                "named web color literal detected",
            ));
        }

        if Self::is_valid_color_role(trimmed) {
            Ok(Self(trimmed.to_string()))
        } else {
            Err(DesignSystemError::color_authority_violation(
                "themeColor",
                s,
                "unknown or non-color theme token",
            ))
        }
    }

    /// Check if the string is a valid active-theme color role or `transparent`.
    pub fn is_valid_color_role(role: &str) -> bool {
        if role == "transparent" {
            return true;
        }
        // Verify that the role exists in Clay's core token catalog as a ColorRole
        core_token_type(role) == Some(crate::shell::theme::ThemeTokenType::ColorRole)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn transparent() -> Self {
        Self("transparent".to_string())
    }

    pub fn validate(&self, field: &str) -> Result<(), DesignSystemError> {
        let trimmed = self.0.trim();
        if trimmed.is_empty() {
            return Err(DesignSystemError::reject(
                field,
                Some(self.0.clone()),
                "non-empty theme color role",
                "color-role reference cannot be empty",
            ));
        }
        if !Self::is_valid_color_role(trimmed) {
            return Err(DesignSystemError::color_authority_violation(
                field,
                &self.0,
                "invalid theme color role or color literal",
            ));
        }
        Ok(())
    }
}

impl fmt::Display for ThemeColorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Closed enum of supported border styles for component recipes.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
}

string_enum_impl! {
    pub BorderStyle {
        None => "none",
        Solid => "solid",
        Dashed => "dashed",
        Dotted => "dotted",
    }
}

/// Closed enum of supported outline styles for focus rings.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum OutlineStyle {
    None,
    Solid,
}

string_enum_impl! {
    pub OutlineStyle {
        None => "none",
        Solid => "solid",
    }
}

/// Closed enum of transition timing and easing curves.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum TransitionTiming {
    Linear,
    EaseOut,
    SpringSnappy,
    SpringSmooth,
}

string_enum_impl! {
    pub TransitionTiming {
        Linear => "linear",
        EaseOut => "ease-out",
        SpringSnappy => "spring-snappy",
        SpringSmooth => "spring-smooth",
    }
}

/// Closed enum of safe, bounded tactile transform presets.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum TransformPreset {
    None,
    PressSubtle,
    PressShiftDown,
    HoverLift,
}

string_enum_impl! {
    pub TransformPreset {
        None => "none",
        PressSubtle => "press-subtle",
        PressShiftDown => "press-shift-down",
        HoverLift => "hover-lift",
    }
}

/// Interaction and contextual states for recipe resolution.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[rkyv(derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[serde(rename_all = "kebab-case")]
pub enum RecipeState {
    Rest,
    Hover,
    Active,
    Focus,
    Disabled,
    Selected,
    Expanded,
    Open,
    Invalid,
}

string_enum_impl! {
    pub RecipeState {
        Rest => "rest",
        Hover => "hover",
        Active => "active",
        Focus => "focus",
        Disabled => "disabled",
        Selected => "selected",
        Expanded => "expanded",
        Open => "open",
        Invalid => "invalid",
    }
}

/// A structured shadow layer definition.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ShadowLayer {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub spread: f64,
    pub color_role: ThemeColorRef,
    pub opacity: f64,
    #[serde(default)]
    pub inset: bool,
}

impl ShadowLayer {
    pub fn validate(&self, field: &str) -> Result<(), DesignSystemError> {
        if !self.x.is_finite() || !(-32.0..=32.0).contains(&self.x) {
            return Err(DesignSystemError::reject(
                format!("{field}.x"),
                Some(self.x.to_string()),
                "f64 in range [-32.0, 32.0]",
                "shadow x offset out of bounds",
            ));
        }
        if !self.y.is_finite() || !(-32.0..=32.0).contains(&self.y) {
            return Err(DesignSystemError::reject(
                format!("{field}.y"),
                Some(self.y.to_string()),
                "f64 in range [-32.0, 32.0]",
                "shadow y offset out of bounds",
            ));
        }
        if !self.blur.is_finite() || !(0.0..=64.0).contains(&self.blur) {
            return Err(DesignSystemError::reject(
                format!("{field}.blur"),
                Some(self.blur.to_string()),
                "f64 in range [0.0, 64.0]",
                "shadow blur out of bounds",
            ));
        }
        if !self.spread.is_finite() || !(-16.0..=16.0).contains(&self.spread) {
            return Err(DesignSystemError::reject(
                format!("{field}.spread"),
                Some(self.spread.to_string()),
                "f64 in range [-16.0, 16.0]",
                "shadow spread out of bounds",
            ));
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DesignSystemError::reject(
                format!("{field}.opacity"),
                Some(self.opacity.to_string()),
                "f64 in range [0.0, 1.0]",
                "shadow opacity out of bounds",
            ));
        }
        Ok(())
    }
}

/// Structured inner highlight rim definition (for glass/bezel effects).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct InnerHighlight {
    pub color_role: ThemeColorRef,
    pub opacity: f64,
    pub width: f64,
}

impl InnerHighlight {
    pub fn validate(&self, field: &str) -> Result<(), DesignSystemError> {
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DesignSystemError::reject(
                format!("{field}.opacity"),
                Some(self.opacity.to_string()),
                "f64 in range [0.0, 1.0]",
                "inner highlight opacity out of bounds",
            ));
        }
        if !self.width.is_finite() || !(1.0..=4.0).contains(&self.width) {
            return Err(DesignSystemError::reject(
                format!("{field}.width"),
                Some(self.width.to_string()),
                "f64 in range [1.0, 4.0]",
                "inner highlight width out of bounds",
            ));
        }
        Ok(())
    }
}

/// Namespaced, non-color value definition within a design system's `values` dictionary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum DesignSystemValue {
    Dimension(f64),
    Radius(f64),
    BorderWidth(f64),
    Opacity(f64),
    BackdropBlur(f64),
    BackdropSaturate(f64),
    MotionDuration(f64),
    BorderStyle(BorderStyle),
    TransitionTiming(TransitionTiming),
    TransformPreset(TransformPreset),
}

impl DesignSystemValue {
    pub fn validate(&self, name: &str) -> Result<(), DesignSystemError> {
        match self {
            Self::Dimension(v) => {
                if !v.is_finite() || !(0.0..=MAX_DIMENSION_PX).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!("finite dimension in [0.0, {MAX_DIMENSION_PX}]"),
                        "dimension out of bounds",
                    ));
                }
            }
            Self::Radius(v) => {
                if !v.is_finite()
                    || (!((0.0..=MAX_STANDARD_RADIUS_PX).contains(v))
                        && (*v - MAX_RADIUS_PX).abs() > 0.001)
                {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!(
                            "finite radius in [0.0, {MAX_STANDARD_RADIUS_PX}] or {MAX_RADIUS_PX}"
                        ),
                        "radius out of bounds",
                    ));
                }
            }
            Self::BorderWidth(v) => {
                if !v.is_finite() || !(0.0..=MAX_BORDER_WIDTH_PX).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!("finite border width in [0.0, {MAX_BORDER_WIDTH_PX}]"),
                        "border width out of bounds",
                    ));
                }
            }
            Self::Opacity(v) => {
                if !v.is_finite() || !(0.0..=1.0).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        "finite opacity in [0.0, 1.0]",
                        "opacity out of bounds",
                    ));
                }
            }
            Self::BackdropBlur(v) => {
                if !v.is_finite() || !(0.0..=MAX_BLUR_PX).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!("finite blur in [0.0, {MAX_BLUR_PX}]"),
                        "backdrop blur out of bounds",
                    ));
                }
            }
            Self::BackdropSaturate(v) => {
                if !v.is_finite() || !(MIN_SATURATE..=MAX_SATURATE).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!("finite saturation in [{MIN_SATURATE}, {MAX_SATURATE}]"),
                        "backdrop saturation out of bounds",
                    ));
                }
            }
            Self::MotionDuration(v) => {
                if !v.is_finite() || !(0.0..=MAX_MOTION_MILLIS).contains(v) {
                    return Err(DesignSystemError::reject(
                        format!("values.{name}"),
                        Some(v.to_string()),
                        format!("finite duration in [0.0, {MAX_MOTION_MILLIS}]"),
                        "motion duration out of bounds",
                    ));
                }
            }
            Self::BorderStyle(_) | Self::TransitionTiming(_) | Self::TransformPreset(_) => {}
        }
        Ok(())
    }
}

/// A 4-tuple key identifying a specific component, variant, slot, and state recipe.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord, Hash))]
pub struct RecipeKey {
    pub component: String,
    pub variant: String,
    pub slot: String,
    pub state: RecipeState,
}

impl RecipeKey {
    pub fn new(
        component: impl Into<String>,
        variant: impl Into<String>,
        slot: impl Into<String>,
        state: RecipeState,
    ) -> Self {
        Self {
            component: component.into(),
            variant: variant.into(),
            slot: slot.into(),
            state,
        }
    }

    /// Parse a dotted string representation.
    ///
    /// Accepts:
    /// - 4 parts: `"{component}.{variant}.{slot}.{state}"`
    /// - 3 parts: `"{component}.{slot}.{state}"` (defaults `variant` to `"default"`)
    pub fn parse(s: &str) -> Result<Self, DesignSystemError> {
        let parts: Vec<&str> = s.split('.').collect();
        match parts.len() {
            4 => {
                let state = RecipeState::parse(parts[3]).ok_or_else(|| {
                    DesignSystemError::reject(
                        "recipe_key",
                        Some(s.to_string()),
                        "valid state (e.g. rest, hover, active, focus, disabled, selected, expanded, open, invalid)",
                        format!("unknown state `{}`", parts[3]),
                    )
                })?;
                Ok(Self {
                    component: parts[0].to_string(),
                    variant: parts[1].to_string(),
                    slot: parts[2].to_string(),
                    state,
                })
            }
            3 => {
                let state = RecipeState::parse(parts[2]).ok_or_else(|| {
                    DesignSystemError::reject(
                        "recipe_key",
                        Some(s.to_string()),
                        "valid state (e.g. rest, hover, active, focus, disabled, selected, expanded, open, invalid)",
                        format!("unknown state `{}`", parts[2]),
                    )
                })?;
                Ok(Self {
                    component: parts[0].to_string(),
                    variant: "default".to_string(),
                    slot: parts[1].to_string(),
                    state,
                })
            }
            _ => Err(DesignSystemError::reject(
                "recipe_key",
                Some(s.to_string()),
                "`component.variant.slot.state` or `component.slot.state`",
                "invalid recipe key segment count",
            )),
        }
    }

    pub fn to_key_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.component,
            self.variant,
            self.slot,
            self.state.as_str()
        )
    }

    pub fn validate(&self) -> Result<(), DesignSystemError> {
        if self.component.trim().is_empty() || self.component.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "recipe_key.component",
                Some(self.component.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid component name in recipe key",
            ));
        }
        if self.variant.trim().is_empty() || self.variant.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "recipe_key.variant",
                Some(self.variant.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid variant name in recipe key",
            ));
        }
        if self.slot.trim().is_empty() || self.slot.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "recipe_key.slot",
                Some(self.slot.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid slot name in recipe key",
            ));
        }
        Ok(())
    }
}

impl fmt::Display for RecipeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_key_string())
    }
}

impl Serialize for RecipeKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_key_string())
    }
}

impl<'de> Deserialize<'de> for RecipeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Raw declarative component recipe as declared in a package manifest.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ComponentRecipeDeclaration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<ThemeColorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<ThemeColorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<ThemeColorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_style: Option<BorderStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Vec<ShadowLayer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_blur: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_saturate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_highlight: Option<InnerHighlight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<ThemeColorRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_style: Option<OutlineStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_timing: Option<TransitionTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform_preset: Option<TransformPreset>,
}

impl ComponentRecipeDeclaration {
    pub fn validate(&self, key: &RecipeKey) -> Result<(), DesignSystemError> {
        let prefix = key.to_key_string();
        if let Some(opacity) = self.background_opacity
            && (!opacity.is_finite() || !(0.0..=1.0).contains(&opacity))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backgroundOpacity"),
                Some(opacity.to_string()),
                "f64 in range [0.0, 1.0]",
                "background opacity out of bounds",
            ));
        }
        if let Some(w) = self.border_width
            && (!w.is_finite() || !(0.0..=MAX_BORDER_WIDTH_PX).contains(&w))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.borderWidth"),
                Some(w.to_string()),
                format!("f64 in range [0.0, {MAX_BORDER_WIDTH_PX}]"),
                "border width out of bounds",
            ));
        }
        if let Some(r) = self.border_radius
            && (!r.is_finite()
                || (!((0.0..=MAX_STANDARD_RADIUS_PX).contains(&r))
                    && (r - MAX_RADIUS_PX).abs() > 0.001))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.borderRadius"),
                Some(r.to_string()),
                format!(
                    "f64 in range [0.0, {MAX_STANDARD_RADIUS_PX}] or full pill {MAX_RADIUS_PX}"
                ),
                "border radius out of bounds",
            ));
        }
        if let Some(ref layers) = self.shadow {
            if layers.len() > MAX_SHADOW_LAYERS {
                return Err(DesignSystemError::reject(
                    format!("{prefix}.shadow"),
                    Some(format!("{} layers", layers.len())),
                    format!("at most {MAX_SHADOW_LAYERS} shadow layers"),
                    "too many shadow layers",
                ));
            }
            for (idx, layer) in layers.iter().enumerate() {
                layer.validate(&format!("{prefix}.shadow[{idx}]"))?;
            }
        }
        if let Some(blur) = self.backdrop_blur
            && (!blur.is_finite() || !(0.0..=MAX_BLUR_PX).contains(&blur))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backdropBlur"),
                Some(blur.to_string()),
                format!("f64 in range [0.0, {MAX_BLUR_PX}]"),
                "backdrop blur out of bounds",
            ));
        }
        if let Some(sat) = self.backdrop_saturate
            && (!sat.is_finite() || !(MIN_SATURATE..=MAX_SATURATE).contains(&sat))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backdropSaturate"),
                Some(sat.to_string()),
                format!("f64 in range [{MIN_SATURATE}, {MAX_SATURATE}]"),
                "backdrop saturation out of bounds",
            ));
        }
        if let Some(ref highlight) = self.inner_highlight {
            highlight.validate(&format!("{prefix}.innerHighlight"))?;
        }
        if let Some(op) = self.opacity
            && (!op.is_finite() || !(0.0..=1.0).contains(&op))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.opacity"),
                Some(op.to_string()),
                "f64 in range [0.0, 1.0]",
                "opacity out of bounds",
            ));
        }
        if let Some(w) = self.outline_width
            && (!w.is_finite() || !(0.0..=4.0).contains(&w))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.outlineWidth"),
                Some(w.to_string()),
                "f64 in range [0.0, 4.0]",
                "outline width out of bounds",
            ));
        }
        if let Some(offset) = self.outline_offset
            && (!offset.is_finite() || !(-4.0..=4.0).contains(&offset))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.outlineOffset"),
                Some(offset.to_string()),
                "f64 in range [-4.0, 4.0]",
                "outline offset out of bounds",
            ));
        }
        if let Some(dur) = self.transition_duration
            && (!dur.is_finite() || !(0.0..=MAX_MOTION_MILLIS).contains(&dur))
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.transitionDuration"),
                Some(dur.to_string()),
                format!("f64 in range [0.0, {MAX_MOTION_MILLIS}]"),
                "transition duration out of bounds",
            ));
        }
        Ok(())
    }
}

/// Package-contributed UI design-system declaration (`clay.contributions.uiDesignSystem`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDesignSystemDeclaration {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, DesignSystemValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub recipes: BTreeMap<RecipeKey, ComponentRecipeDeclaration>,
}

impl UiDesignSystemDeclaration {
    pub fn validate(&self) -> Result<(), DesignSystemError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(DesignSystemError::invalid_version(self.schema_version));
        }
        if self.id.trim().is_empty() || self.id.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "id",
                Some(self.id.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid design system id",
            ));
        }
        if self.display_name.trim().is_empty() || self.display_name.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "displayName",
                Some(self.display_name.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid display name",
            ));
        }
        if self.values.len() > MAX_VALUES {
            return Err(DesignSystemError::reject(
                "values",
                Some(format!("{} entries", self.values.len())),
                format!("at most {MAX_VALUES} values"),
                "too many design-system values",
            ));
        }
        for (name, val) in &self.values {
            if name.trim().is_empty() || name.len() > MAX_STRING_LEN {
                return Err(DesignSystemError::reject(
                    format!("values.{name}"),
                    Some(name.clone()),
                    format!("identifier <= {MAX_STRING_LEN} chars"),
                    "invalid value name",
                ));
            }
            val.validate(name)?;
        }
        if self.recipes.len() > MAX_RECIPES {
            return Err(DesignSystemError::reject(
                "recipes",
                Some(format!("{} recipes", self.recipes.len())),
                format!("at most {MAX_RECIPES} recipes"),
                "too many component recipes",
            ));
        }
        for (key, recipe) in &self.recipes {
            recipe.validate(key)?;
        }
        Ok(())
    }
}

/// Fully resolved visual recipe for a component slot and state.
///
/// Guaranteed to contain non-empty, validated, fallback-resolved values for
/// every visual property family.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedComponentRecipe {
    pub background_color: ThemeColorRef,
    pub background_opacity: f64,
    pub text_color: ThemeColorRef,
    pub border_color: ThemeColorRef,
    pub border_width: f64,
    pub border_style: BorderStyle,
    pub border_radius: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadow: Vec<ShadowLayer>,
    pub backdrop_blur: f64,
    pub backdrop_saturate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_highlight: Option<InnerHighlight>,
    pub opacity: f64,
    pub outline_color: ThemeColorRef,
    pub outline_width: f64,
    pub outline_offset: f64,
    pub outline_style: OutlineStyle,
    pub transition_duration: f64,
    pub transition_timing: TransitionTiming,
    pub transform_preset: TransformPreset,
}

impl ResolvedComponentRecipe {
    pub fn validate(&self, prefix: &str) -> Result<(), DesignSystemError> {
        self.background_color
            .validate(&format!("{prefix}.backgroundColor"))?;
        if !self.background_opacity.is_finite() || !(0.0..=1.0).contains(&self.background_opacity) {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backgroundOpacity"),
                Some(self.background_opacity.to_string()),
                "f64 in range [0.0, 1.0]",
                "background opacity out of bounds",
            ));
        }
        self.text_color.validate(&format!("{prefix}.textColor"))?;
        self.border_color
            .validate(&format!("{prefix}.borderColor"))?;
        if !self.border_width.is_finite()
            || !(0.0..=MAX_BORDER_WIDTH_PX).contains(&self.border_width)
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.borderWidth"),
                Some(self.border_width.to_string()),
                format!("f64 in range [0.0, {MAX_BORDER_WIDTH_PX}]"),
                "border width out of bounds",
            ));
        }
        if !self.border_radius.is_finite() || !(0.0..=MAX_RADIUS_PX).contains(&self.border_radius) {
            return Err(DesignSystemError::reject(
                format!("{prefix}.borderRadius"),
                Some(self.border_radius.to_string()),
                format!("f64 in range [0.0, {MAX_RADIUS_PX}]"),
                "border radius out of bounds",
            ));
        }
        for (i, layer) in self.shadow.iter().enumerate() {
            layer.validate(&format!("{prefix}.shadow[{i}]"))?;
        }
        if !self.backdrop_blur.is_finite() || !(0.0..=MAX_BLUR_PX).contains(&self.backdrop_blur) {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backdropBlur"),
                Some(self.backdrop_blur.to_string()),
                format!("f64 in range [0.0, {MAX_BLUR_PX}]"),
                "backdrop blur out of bounds",
            ));
        }
        if !self.backdrop_saturate.is_finite()
            || !(MIN_SATURATE..=MAX_SATURATE).contains(&self.backdrop_saturate)
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.backdropSaturate"),
                Some(self.backdrop_saturate.to_string()),
                format!("f64 in range [{MIN_SATURATE}, {MAX_SATURATE}]"),
                "backdrop saturate out of bounds",
            ));
        }
        if let Some(ref ih) = self.inner_highlight {
            ih.validate(&format!("{prefix}.innerHighlight"))?;
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(DesignSystemError::reject(
                format!("{prefix}.opacity"),
                Some(self.opacity.to_string()),
                "f64 in range [0.0, 1.0]",
                "opacity out of bounds",
            ));
        }
        self.outline_color
            .validate(&format!("{prefix}.outlineColor"))?;
        if !self.outline_width.is_finite()
            || !(0.0..=MAX_BORDER_WIDTH_PX).contains(&self.outline_width)
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.outlineWidth"),
                Some(self.outline_width.to_string()),
                format!("f64 in range [0.0, {MAX_BORDER_WIDTH_PX}]"),
                "outline width out of bounds",
            ));
        }
        if !self.outline_offset.is_finite() || !(-16.0..=16.0).contains(&self.outline_offset) {
            return Err(DesignSystemError::reject(
                format!("{prefix}.outlineOffset"),
                Some(self.outline_offset.to_string()),
                "f64 in range [-16.0, 16.0]",
                "outline offset out of bounds",
            ));
        }
        if !self.transition_duration.is_finite()
            || !(0.0..=MAX_MOTION_MILLIS).contains(&self.transition_duration)
        {
            return Err(DesignSystemError::reject(
                format!("{prefix}.transitionDuration"),
                Some(self.transition_duration.to_string()),
                format!("f64 in range [0.0, {MAX_MOTION_MILLIS}]"),
                "transition duration out of bounds",
            ));
        }
        Ok(())
    }
}

/// The terminal of the resolution chain: the values a recipe gets when neither the
/// package nor the core fallbacks say anything. It carries the language's neutral
/// values (DESIGN.md §9 focus ring at offset 2; §7 "nothing linear", so an
/// undeclared region declares no transition rather than a linear 100ms one).
impl Default for ResolvedComponentRecipe {
    fn default() -> Self {
        Self {
            background_color: ThemeColorRef::transparent(),
            background_opacity: 1.0,
            text_color: ThemeColorRef("text.primary".to_string()),
            border_color: ThemeColorRef("border.subtle".to_string()),
            border_width: 1.0,
            border_style: BorderStyle::Solid,
            border_radius: 0.0,
            padding: None,
            gap: None,
            shadow: Vec::new(),
            backdrop_blur: 0.0,
            backdrop_saturate: 1.0,
            inner_highlight: None,
            opacity: 1.0,
            outline_color: ThemeColorRef("focus.ring".to_string()),
            outline_width: 2.0,
            outline_offset: 2.0,
            outline_style: OutlineStyle::Solid,
            transition_duration: 0.0,
            transition_timing: TransitionTiming::Linear,
            transform_preset: TransformPreset::None,
        }
    }
}

/// A resolved UI design system with all recipes fully computed through the
/// fallback inheritance chain.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedUiDesignSystem {
    pub id: String,
    pub display_name: String,
    pub recipes: BTreeMap<RecipeKey, ResolvedComponentRecipe>,
}

impl ResolvedUiDesignSystem {
    pub fn core_fallback() -> Self {
        Self {
            id: "@clay/core".to_string(),
            display_name: "Clay Built-in Baseline".to_string(),
            recipes: core_design_system_fallbacks(),
        }
    }
}

/// Provenance of an active UI design-system contribution.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct DesignSystemProvenance {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub trust_domain: crate::protocol::PackageUiTrustDomain,
}

impl DesignSystemProvenance {
    pub fn builtin_core() -> Self {
        Self {
            package_name: "core".to_string(),
            package_version: "1.0.0".to_string(),
            api_prefix: "clay".to_string(),
            trust_domain: crate::protocol::PackageUiTrustDomain::Trusted,
        }
    }

    pub fn from_record(record: &crate::packages::record::PackageRecord) -> Self {
        Self {
            package_name: record.manifest.name.clone(),
            package_version: record.manifest.version.clone(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
            trust_domain: match record.runtime_domain {
                crate::packages::bundled::RuntimeDomain::Trusted => {
                    crate::protocol::PackageUiTrustDomain::Trusted
                }
                crate::packages::bundled::RuntimeDomain::ThirdParty => {
                    crate::protocol::PackageUiTrustDomain::ThirdParty
                }
            },
        }
    }
}

/// Fully resolved active UI design system installed for the current runtime generation.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDesignSystem {
    pub specifier: String,
    pub schema_version: u32,
    pub generation: u64,
    pub provenance: DesignSystemProvenance,
    pub recipes: BTreeMap<RecipeKey, ResolvedComponentRecipe>,
}

impl ActiveDesignSystem {
    pub fn core_fallback(generation: u64) -> Self {
        Self {
            specifier: "@clay/core".to_string(),
            schema_version: SCHEMA_VERSION,
            generation,
            provenance: DesignSystemProvenance::builtin_core(),
            recipes: core_design_system_fallbacks(),
        }
    }

    pub fn from_resolved(
        specifier: impl Into<String>,
        generation: u64,
        provenance: DesignSystemProvenance,
        resolved: ResolvedUiDesignSystem,
    ) -> Self {
        Self {
            specifier: specifier.into(),
            schema_version: SCHEMA_VERSION,
            generation,
            provenance,
            recipes: resolved.recipes,
        }
    }

    pub fn validate(&self) -> Result<(), DesignSystemError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(DesignSystemError::invalid_version(self.schema_version));
        }
        if self.specifier.trim().is_empty() || self.specifier.len() > MAX_STRING_LEN {
            return Err(DesignSystemError::reject(
                "specifier",
                Some(self.specifier.clone()),
                format!("non-empty string <= {MAX_STRING_LEN} chars"),
                "invalid design system specifier",
            ));
        }
        if self.recipes.len() > MAX_RECIPES {
            return Err(DesignSystemError::reject(
                "recipes",
                Some(self.recipes.len().to_string()),
                format!("recipes count <= {MAX_RECIPES}"),
                "too many recipes in active design system",
            ));
        }
        for (key, recipe) in &self.recipes {
            key.validate()?;
            recipe.validate(&key.to_key_string())?;
        }
        Ok(())
    }
}

/// Quiet Instrument geometry ladder (DESIGN.md §4/§5).
///
/// Literals rather than variables: `@clay/core` paints before any package is
/// installed, and a build that has no design system installed must resolve the
/// same values `@clay/design-instrument` activates — otherwise the activation
/// swap would move geometry (plan 118 task 16).
const RADIUS_XS: f64 = 5.0;
const RADIUS_CONTROL: f64 = 8.0;
const RADIUS_PANEL: f64 = 12.0;
const RADIUS_SURFACE: f64 = 16.0;
const RADIUS_PILL: f64 = 9999.0;
/// Full-bleed regions keep a square outer edge (§5), and a text-only or
/// layout-only recipe has no visible radius at all.
const RADIUS_FLUSH: f64 = 0.0;

/// Motion tiers (DESIGN.md §7): state transitions, surfaces entering, and — for a
/// region that never animates — no transition at all. Nothing in the language is
/// linear, so an inert region declares 0 rather than a linear transition.
const MOTION_FAST: f64 = 150.0;
const MOTION_ENTER: f64 = 240.0;
const MOTION_NONE: f64 = 0.0;

/// The three approved soft elevation stacks (DESIGN.md §6). Static regions get
/// neither: a region that never floats must not look pressable (§14 bans hard
/// offset shadows outright). `Halo` is the composer palette's and the `@`
/// mentions menu's even, zero-offset glow over the shared veil (plan 125).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Elevation {
    Flat,
    Pop,
    Overlay,
    Halo,
}

fn elevation_stack(elevation: Elevation) -> Vec<ShadowLayer> {
    // (x, y, blur, spread, color role, opacity)
    let layers: &[(f64, f64, f64, f64, &str, f64)] = match elevation {
        Elevation::Flat => return Vec::new(),
        Elevation::Pop => &[
            (0.0, 14.0, 34.0, -14.0, "text.primary", 0.34),
            (0.0, 1.0, 3.0, -1.0, "text.primary", 0.16),
        ],
        Elevation::Overlay => &[
            (0.0, 24.0, 60.0, -16.0, "text.primary", 0.42),
            (0.0, 2.0, 10.0, -4.0, "text.primary", 0.22),
        ],
        Elevation::Halo => &[
            (0.0, 0.0, 14.0, -2.0, "text.primary", 0.14),
            (0.0, 0.0, 3.0, 0.0, "text.primary", 0.08),
        ],
    };
    layers
        .iter()
        .map(|&(x, y, blur, spread, role, opacity)| ShadowLayer {
            x,
            y,
            blur,
            spread,
            color_role: ThemeColorRef(role.to_string()),
            opacity,
            inset: false,
        })
        .collect()
}

/// One component kind or shell surface and the language values for its `root`
/// rest recipe.
struct FallbackKind {
    component: &'static str,
    fill: &'static str,
    text: &'static str,
    border: &'static str,
    border_width: f64,
    radius: f64,
    padding: Option<&'static str>,
    gap: Option<&'static str>,
    background_opacity: f64,
    motion: f64,
    timing: TransitionTiming,
    elevation: Elevation,
}

/// Component kinds the host shell consumes before a design-system snapshot exists.
/// The shipped package declares most of these too; the values here are the same ones
/// (asserted by `package_ui_conformance`), so installing a design system cannot move
/// the pre-bootstrap paint.
#[rustfmt::skip]
const FALLBACK_KINDS: &[FallbackKind] = &[
    FallbackKind { component: "dropdown", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: Some("spacing.xxs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "checkbox", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_XS, padding: None, gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "switch", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_PILL, padding: None, gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "slider", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "label", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "badge", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_XS, padding: Some("spacing.xxs"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "progressBar", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "tab", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "collapse", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_CONTROL, padding: Some("spacing.sm"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "table", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "tree", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "flex", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: Some("spacing.sm"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "grid", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "scroll", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: Some("spacing.sm"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "seg", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_PILL, padding: Some("spacing.xxs"), gap: Some("spacing.none"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "sessionRow", fill: "transparent", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_CONTROL, padding: Some("spacing.xs"), gap: Some("spacing.sm"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "statRow", fill: "transparent", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_XS, padding: Some("spacing.xxs"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat },
    FallbackKind { component: "empty", fill: "transparent", text: "text.muted", border: "border.subtle", border_width: 0.0, radius: RADIUS_FLUSH, padding: Some("spacing.lg"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "keyHint", fill: "transparent", text: "text.muted", border: "border.subtle", border_width: 0.0, radius: RADIUS_FLUSH, padding: Some("spacing.xxs"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
];

/// Internal shell surfaces. The shipped package covers most of the shell chrome;
/// these are the surfaces the host owns outright, plus the ones the chat removal
/// (plan 118 tasks 23-24) deletes from the package.
#[rustfmt::skip]
const FALLBACK_SURFACES: &[FallbackKind] = &[
    FallbackKind { component: "tooltip", fill: "surface.overlay", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_CONTROL, padding: Some("spacing.tooltip"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Pop },
    FallbackKind { component: "tabBar", fill: "transparent", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_FLUSH, padding: Some("spacing.xs"), gap: Some("spacing.xxs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "paneSplitTree", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "statusBar", fill: "transparent", text: "text.muted", border: "border.hairline", border_width: 1.0, radius: RADIUS_FLUSH, padding: Some("spacing.xxs"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "commandCentre", fill: "surface.overlay", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_SURFACE, padding: Some("spacing.sm"), gap: Some("spacing.xs"), background_opacity: 1.0, motion: MOTION_ENTER, timing: TransitionTiming::SpringSnappy, elevation: Elevation::Halo },
    FallbackKind { component: "fileBrowser", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "settingsPanel", fill: "surface.panel", text: "text.primary", border: "border.hairline", border_width: 1.0, radius: RADIUS_PANEL, padding: Some("spacing.sm"), gap: Some("spacing.xs"), background_opacity: 0.55, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "chatPanel", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "welcome", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "transientMenu", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "completion", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
    FallbackKind { component: "editorChrome", fill: "transparent", text: "text.primary", border: "transparent", border_width: 0.0, radius: RADIUS_PANEL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat },
];

/// Auxiliary families whose consumed rest key is not the `<component>.default.root.rest`
/// shape the loop above builds: the agent view's picker trigger, the key-hint
/// slots, and the status dot's tone variants (plan 118 task E4). Values mirror what
/// the shipped package resolves for those keys, so the core baseline stays the
/// host-consumed subset of one language.
#[rustfmt::skip]
const FALLBACK_SLOT_EXTRAS: &[(&str, &str, &str, FallbackKind)] = &[
    ("agentPicker", "default", "trigger", FallbackKind { component: "agentPicker", fill: "transparent", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_CONTROL, padding: Some("spacing.xxs"), gap: Some("spacing.xxs"), background_opacity: 1.0, motion: MOTION_FAST, timing: TransitionTiming::EaseOut, elevation: Elevation::Flat }),
    ("keyHint", "default", "keys", FallbackKind { component: "keyHint", fill: "transparent", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_FLUSH, padding: None, gap: Some("spacing.xxs"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("keyHint", "default", "row", FallbackKind { component: "keyHint", fill: "transparent", text: "text.muted", border: "border.subtle", border_width: 0.0, radius: RADIUS_FLUSH, padding: Some("spacing.xxs"), gap: Some("spacing.sm"), background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("statusDot", "busy", "root", FallbackKind { component: "statusDot", fill: "accent.primary", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("statusDot", "error", "root", FallbackKind { component: "statusDot", fill: "diagnostic.error", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("statusDot", "muted", "root", FallbackKind { component: "statusDot", fill: "text.muted", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("statusDot", "success", "root", FallbackKind { component: "statusDot", fill: "diagnostic.success", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
    ("statusDot", "warning", "root", FallbackKind { component: "statusDot", fill: "diagnostic.warning", text: "text.primary", border: "border.subtle", border_width: 0.0, radius: RADIUS_PILL, padding: None, gap: None, background_opacity: 1.0, motion: MOTION_NONE, timing: TransitionTiming::Linear, elevation: Elevation::Flat }),
];

/// The complete map of Clay core default recipes: the values the shell paints
/// before (or without) a design-system snapshot, and the base every package recipe
/// resolves against. Keys and values are asserted against the shipped package by
/// `tests/package_ui_conformance.rs`.
pub fn core_design_system_fallbacks() -> BTreeMap<RecipeKey, ResolvedComponentRecipe> {
    let mut map = BTreeMap::new();

    // 1. Buttons: rest / hover / active / focus / disabled for each variant.
    #[rustfmt::skip]
    let button_variants = [
        // variant, rest fill, text, rest border, rest border width, hover fill,
        // hover fill opacity, hover text, hover border width, hover border,
        // active fill, active fill opacity, disabled fill
        ("default", "transparent", "text.primary", "border.hairline", 1.0, "surface.hover", 1.0, "text.primary", 1.0, "border.subtle", "surface.active", 1.0, "transparent"),
        ("primary", "accent.primary", "surface.main", "transparent", 0.0, "accent.primary", 0.92, "surface.main", 0.0, "transparent", "accent.primary", 1.0, "surface.disabled"),
        ("muted", "transparent", "text.muted", "transparent", 0.0, "surface.hover", 1.0, "text.primary", 0.0, "transparent", "surface.active", 1.0, "transparent"),
        ("danger", "transparent", "diagnostic.error", "border.hairline", 1.0, "diagnostic.error", 0.16, "diagnostic.error", 0.0, "border.hairline", "diagnostic.error", 0.24, "transparent"),
    ];

    for (
        variant,
        fill,
        text,
        border,
        border_width,
        hover_fill,
        hover_opacity,
        hover_text,
        hover_border_width,
        hover_border,
        active_fill,
        active_opacity,
        disabled_fill,
    ) in button_variants
    {
        let rest = ResolvedComponentRecipe {
            background_color: ThemeColorRef(fill.to_string()),
            background_opacity: 1.0,
            text_color: ThemeColorRef(text.to_string()),
            border_color: ThemeColorRef(border.to_string()),
            border_width,
            border_style: if border_width > 0.0 {
                BorderStyle::Solid
            } else {
                BorderStyle::None
            },
            border_radius: RADIUS_CONTROL,
            padding: Some("spacing.xs".to_string()),
            gap: Some("spacing.xs".to_string()),
            shadow: Vec::new(),
            backdrop_blur: 0.0,
            backdrop_saturate: 1.0,
            inner_highlight: None,
            opacity: 1.0,
            outline_color: ThemeColorRef("focus.ring".to_string()),
            outline_width: 2.0,
            outline_offset: 2.0,
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_FAST,
            transition_timing: TransitionTiming::EaseOut,
            transform_preset: TransformPreset::None,
        };
        map.insert(
            RecipeKey::new("button", variant, "root", RecipeState::Rest),
            rest.clone(),
        );

        // Hover and active change fill/text/border only; the fill is a state role,
        // never a lift, so no shadow appears (§6).
        map.insert(
            RecipeKey::new("button", variant, "root", RecipeState::Hover),
            ResolvedComponentRecipe {
                background_color: ThemeColorRef(hover_fill.to_string()),
                background_opacity: hover_opacity,
                text_color: ThemeColorRef(hover_text.to_string()),
                border_color: ThemeColorRef(hover_border.to_string()),
                border_width: hover_border_width,
                ..rest.clone()
            },
        );
        map.insert(
            RecipeKey::new("button", variant, "root", RecipeState::Active),
            ResolvedComponentRecipe {
                background_color: ThemeColorRef(active_fill.to_string()),
                background_opacity: active_opacity,
                border_width: hover_border_width,
                transform_preset: TransformPreset::PressShiftDown,
                ..rest.clone()
            },
        );
        map.insert(
            RecipeKey::new("button", variant, "root", RecipeState::Focus),
            ResolvedComponentRecipe {
                outline_color: ThemeColorRef("focus.ring".to_string()),
                outline_width: 2.0,
                outline_offset: 2.0,
                outline_style: OutlineStyle::Solid,
                ..rest.clone()
            },
        );
        map.insert(
            RecipeKey::new("button", variant, "root", RecipeState::Disabled),
            ResolvedComponentRecipe {
                background_color: ThemeColorRef(disabled_fill.to_string()),
                text_color: ThemeColorRef("text.disabled".to_string()),
                opacity: 0.5,
                outline_style: OutlineStyle::None,
                ..rest
            },
        );
    }

    // 2. Text inputs: the composer shell (`field`/`root`) and the single-line well.
    // `root` is the composer/textarea shell (DESIGN.md §11: `surface.control` fill,
    // one hairline, radius 12); `input` is the single-line well inside it.
    map.insert(
        RecipeKey::new("textInput", "default", "root", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef("surface.control".to_string()),
            border_color: ThemeColorRef("border.hairline".to_string()),
            border_width: 1.0,
            border_style: BorderStyle::Solid,
            border_radius: RADIUS_PANEL,
            padding: Some("spacing.xs".to_string()),
            gap: Some("spacing.xs".to_string()),
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_FAST,
            transition_timing: TransitionTiming::EaseOut,
            ..ResolvedComponentRecipe::default()
        },
    );
    map.insert(
        RecipeKey::new("textInput", "default", "input", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef("surface.control".to_string()),
            text_color: ThemeColorRef("text.primary".to_string()),
            border_color: ThemeColorRef("border.hairline".to_string()),
            border_width: 1.0,
            border_style: BorderStyle::Solid,
            border_radius: RADIUS_CONTROL,
            padding: Some("spacing.xs".to_string()),
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_FAST,
            transition_timing: TransitionTiming::EaseOut,
            ..ResolvedComponentRecipe::default()
        },
    );

    // 3. Modal: the portal container, the scrim, and the dialog that lifts.
    map.insert(
        RecipeKey::new("modal", "default", "root", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef::transparent(),
            border_color: ThemeColorRef::transparent(),
            border_width: 0.0,
            border_style: BorderStyle::None,
            border_radius: RADIUS_FLUSH,
            outline_style: OutlineStyle::None,
            ..ResolvedComponentRecipe::default()
        },
    );
    map.insert(
        RecipeKey::new("modal", "default", "scrim", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef("surface.scrim".to_string()),
            background_opacity: 0.5,
            border_color: ThemeColorRef::transparent(),
            border_width: 0.0,
            border_style: BorderStyle::None,
            border_radius: RADIUS_FLUSH,
            backdrop_blur: 3.0,
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_ENTER,
            transition_timing: TransitionTiming::SpringSnappy,
            ..ResolvedComponentRecipe::default()
        },
    );
    map.insert(
        RecipeKey::new("modal", "default", "dialog", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef("surface.overlay".to_string()),
            text_color: ThemeColorRef("text.primary".to_string()),
            border_color: ThemeColorRef("border.hairline".to_string()),
            border_width: 1.0,
            border_style: BorderStyle::Solid,
            border_radius: RADIUS_SURFACE,
            padding: Some("spacing.md".to_string()),
            gap: Some("spacing.sm".to_string()),
            shadow: elevation_stack(Elevation::Overlay),
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_ENTER,
            transition_timing: TransitionTiming::SpringSnappy,
            ..ResolvedComponentRecipe::default()
        },
    );

    // 4. Panel: one veil fill, one hairline, radius 12, never a shadow (§6).
    map.insert(
        RecipeKey::new("panel", "default", "root", RecipeState::Rest),
        ResolvedComponentRecipe {
            background_color: ThemeColorRef("surface.panel".to_string()),
            background_opacity: 0.55,
            text_color: ThemeColorRef("text.primary".to_string()),
            border_color: ThemeColorRef("border.hairline".to_string()),
            border_width: 1.0,
            border_style: BorderStyle::Solid,
            border_radius: RADIUS_PANEL,
            padding: Some("spacing.sm".to_string()),
            gap: Some("spacing.xs".to_string()),
            outline_style: OutlineStyle::None,
            transition_duration: MOTION_NONE,
            transition_timing: TransitionTiming::Linear,
            ..ResolvedComponentRecipe::default()
        },
    );

    // 5. The remaining host-consumed kinds and shell surfaces: rest only.
    for kind in FALLBACK_KINDS.iter().chain(FALLBACK_SURFACES) {
        map.insert(
            RecipeKey::new(kind.component, "default", "root", RecipeState::Rest),
            ResolvedComponentRecipe {
                background_color: ThemeColorRef(kind.fill.to_string()),
                background_opacity: kind.background_opacity,
                text_color: ThemeColorRef(kind.text.to_string()),
                border_color: ThemeColorRef(kind.border.to_string()),
                border_width: kind.border_width,
                border_style: if kind.border_width > 0.0 {
                    BorderStyle::Solid
                } else {
                    BorderStyle::None
                },
                border_radius: kind.radius,
                padding: kind.padding.map(str::to_string),
                gap: kind.gap.map(str::to_string),
                shadow: elevation_stack(kind.elevation),
                outline_style: OutlineStyle::None,
                transition_duration: kind.motion,
                transition_timing: kind.timing,
                ..ResolvedComponentRecipe::default()
            },
        );
    }

    // 6. Consumed slots outside the `default/root` shape (picker trigger,
    // key-hint slots, status-dot tones).
    for (component, variant, slot, kind) in FALLBACK_SLOT_EXTRAS {
        map.insert(
            RecipeKey::new(*component, *variant, *slot, RecipeState::Rest),
            ResolvedComponentRecipe {
                background_color: ThemeColorRef(kind.fill.to_string()),
                background_opacity: kind.background_opacity,
                text_color: ThemeColorRef(kind.text.to_string()),
                border_color: ThemeColorRef(kind.border.to_string()),
                border_width: kind.border_width,
                border_style: if kind.border_width > 0.0 {
                    BorderStyle::Solid
                } else {
                    BorderStyle::None
                },
                border_radius: kind.radius,
                padding: kind.padding.map(str::to_string),
                gap: kind.gap.map(str::to_string),
                shadow: elevation_stack(kind.elevation),
                outline_style: OutlineStyle::None,
                transition_duration: kind.motion,
                transition_timing: kind.timing,
                ..ResolvedComponentRecipe::default()
            },
        );
    }

    map
}

/// Resolves a single recipe key through the deterministic 5-step fallback chain.
pub fn resolve_single_recipe(
    key: &RecipeKey,
    declaration: &UiDesignSystemDeclaration,
    parent: Option<&ResolvedUiDesignSystem>,
    core_fallbacks: &BTreeMap<RecipeKey, ResolvedComponentRecipe>,
) -> ResolvedComponentRecipe {
    // 1. Exact match in package recipes
    if let Some(decl) = declaration.recipes.get(key) {
        return merge_recipe_declaration(decl, key, declaration, parent, core_fallbacks);
    }

    // 2. Rest state in package recipes
    if key.state != RecipeState::Rest {
        let rest_key = RecipeKey::new(&key.component, &key.variant, &key.slot, RecipeState::Rest);
        if let Some(decl) = declaration.recipes.get(&rest_key) {
            return merge_recipe_declaration(decl, key, declaration, parent, core_fallbacks);
        }
    }

    // 3. Default variant in package recipes
    if key.variant != "default" {
        let default_key = RecipeKey::new(&key.component, "default", &key.slot, key.state);
        if let Some(decl) = declaration.recipes.get(&default_key) {
            return merge_recipe_declaration(decl, key, declaration, parent, core_fallbacks);
        }
    }

    // 4. Extended parent design system
    if let Some(parent_ds) = parent
        && let Some(recipe) = parent_ds.recipes.get(key)
    {
        return recipe.clone();
    }

    // 5. Exact match in core fallbacks
    if let Some(recipe) = core_fallbacks.get(key) {
        return recipe.clone();
    }

    // 6. Rest state in core fallbacks
    if key.state != RecipeState::Rest {
        let rest_key = RecipeKey::new(&key.component, &key.variant, &key.slot, RecipeState::Rest);
        if let Some(recipe) = core_fallbacks.get(&rest_key) {
            return recipe.clone();
        }
    }

    // 7. Universal fallback default
    ResolvedComponentRecipe::default()
}

fn merge_recipe_declaration(
    decl: &ComponentRecipeDeclaration,
    key: &RecipeKey,
    declaration: &UiDesignSystemDeclaration,
    parent: Option<&ResolvedUiDesignSystem>,
    core_fallbacks: &BTreeMap<RecipeKey, ResolvedComponentRecipe>,
) -> ResolvedComponentRecipe {
    // Compute base fallback to fill missing properties
    let base = if key.state != RecipeState::Rest {
        let rest_key = RecipeKey::new(&key.component, &key.variant, &key.slot, RecipeState::Rest);
        resolve_single_recipe(&rest_key, declaration, parent, core_fallbacks)
    } else if let Some(parent_ds) = parent {
        parent_ds
            .recipes
            .get(key)
            .cloned()
            .or_else(|| core_fallbacks.get(key).cloned())
            .unwrap_or_default()
    } else {
        core_fallbacks.get(key).cloned().unwrap_or_default()
    };

    ResolvedComponentRecipe {
        background_color: decl
            .background_color
            .clone()
            .unwrap_or(base.background_color),
        background_opacity: decl.background_opacity.unwrap_or(base.background_opacity),
        text_color: decl.text_color.clone().unwrap_or(base.text_color),
        border_color: decl.border_color.clone().unwrap_or(base.border_color),
        border_width: decl.border_width.unwrap_or(base.border_width),
        border_style: decl.border_style.unwrap_or(base.border_style),
        border_radius: decl.border_radius.unwrap_or(base.border_radius),
        padding: decl.padding.clone().or(base.padding),
        gap: decl.gap.clone().or(base.gap),
        shadow: decl.shadow.clone().unwrap_or(base.shadow),
        backdrop_blur: decl.backdrop_blur.unwrap_or(base.backdrop_blur),
        backdrop_saturate: decl.backdrop_saturate.unwrap_or(base.backdrop_saturate),
        inner_highlight: decl.inner_highlight.clone().or(base.inner_highlight),
        opacity: decl.opacity.unwrap_or(base.opacity),
        outline_color: decl.outline_color.clone().unwrap_or(base.outline_color),
        outline_width: decl.outline_width.unwrap_or(base.outline_width),
        outline_offset: decl.outline_offset.unwrap_or(base.outline_offset),
        outline_style: decl.outline_style.unwrap_or(base.outline_style),
        transition_duration: decl.transition_duration.unwrap_or(base.transition_duration),
        transition_timing: decl.transition_timing.unwrap_or(base.transition_timing),
        transform_preset: decl.transform_preset.unwrap_or(base.transform_preset),
    }
}

/// Resolves a full UI design-system declaration into a complete `ResolvedUiDesignSystem`.
pub fn resolve_design_system(
    declaration: &UiDesignSystemDeclaration,
    parent: Option<&ResolvedUiDesignSystem>,
) -> Result<ResolvedUiDesignSystem, DesignSystemError> {
    declaration.validate()?;
    let core_fallbacks = core_design_system_fallbacks();

    let mut resolved_recipes = BTreeMap::new();

    // 1. Resolve all explicitly declared recipes
    for key in declaration.recipes.keys() {
        let resolved = resolve_single_recipe(key, declaration, parent, &core_fallbacks);
        resolved_recipes.insert(key.clone(), resolved);
    }

    // 2. Populate missing core fallback recipes
    for core_key in core_fallbacks.keys() {
        if !resolved_recipes.contains_key(core_key) {
            let resolved = resolve_single_recipe(core_key, declaration, parent, &core_fallbacks);
            resolved_recipes.insert(core_key.clone(), resolved);
        }
    }

    Ok(ResolvedUiDesignSystem {
        id: declaration.id.clone(),
        display_name: declaration.display_name.clone(),
        recipes: resolved_recipes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_color_ref_accepts_valid_color_roles_and_transparent() {
        assert!(ThemeColorRef::parse("surface.control").is_ok());
        assert!(ThemeColorRef::parse("accent.primary").is_ok());
        assert!(ThemeColorRef::parse("border.focus").is_ok());
        assert!(ThemeColorRef::parse("diagnostic.error").is_ok());
        assert!(ThemeColorRef::parse("transparent").is_ok());
    }

    #[test]
    fn theme_color_ref_rejects_literal_colors_and_non_color_tokens() {
        // Hex
        assert!(ThemeColorRef::parse("#ffffff").is_err());
        assert!(ThemeColorRef::parse("#1a1a1a").is_err());
        // RGB / HSL
        assert!(ThemeColorRef::parse("rgb(255, 0, 0)").is_err());
        assert!(ThemeColorRef::parse("hsl(120, 50%, 50%)").is_err());
        // Named web colors
        assert!(ThemeColorRef::parse("red").is_err());
        assert!(ThemeColorRef::parse("black").is_err());
        // Non-color tokens
        assert!(ThemeColorRef::parse("spacing.sm").is_err());
        assert!(ThemeColorRef::parse("radius.xs").is_err());
        assert!(ThemeColorRef::parse("typography.body").is_err());
        // Empty / whitespace
        assert!(ThemeColorRef::parse("").is_err());
        assert!(ThemeColorRef::parse("   ").is_err());
    }

    #[test]
    fn recipe_key_parses_3_and_4_part_strings() {
        let k4 = RecipeKey::parse("button.primary.root.hover").unwrap();
        assert_eq!(k4.component, "button");
        assert_eq!(k4.variant, "primary");
        assert_eq!(k4.slot, "root");
        assert_eq!(k4.state, RecipeState::Hover);
        assert_eq!(k4.to_key_string(), "button.primary.root.hover");

        let k3 = RecipeKey::parse("button.root.rest").unwrap();
        assert_eq!(k3.component, "button");
        assert_eq!(k3.variant, "default");
        assert_eq!(k3.slot, "root");
        assert_eq!(k3.state, RecipeState::Rest);
        assert_eq!(k3.to_key_string(), "button.default.root.rest");

        assert!(RecipeKey::parse("button.root").is_err());
        assert!(RecipeKey::parse("button.primary.root.rest.extra").is_err());
        assert!(RecipeKey::parse("button.root.unknown_state").is_err());
    }

    #[test]
    fn recipe_declaration_round_trip() {
        let mut recipes = BTreeMap::new();
        recipes.insert(
            RecipeKey::parse("button.primary.root.rest").unwrap(),
            ComponentRecipeDeclaration {
                background_color: Some(ThemeColorRef::parse("accent.primary").unwrap()),
                border_width: Some(2.0),
                border_radius: Some(4.0),
                border_style: Some(BorderStyle::Solid),
                shadow: Some(vec![ShadowLayer {
                    x: 0.0,
                    y: 2.0,
                    blur: 4.0,
                    spread: 0.0,
                    color_role: ThemeColorRef::parse("surface.overlay").unwrap(),
                    opacity: 0.5,
                    inset: false,
                }]),
                ..Default::default()
            },
        );

        let mut values = BTreeMap::new();
        values.insert(
            "buttonBorder".to_string(),
            DesignSystemValue::BorderWidth(2.0),
        );

        let decl = UiDesignSystemDeclaration {
            schema_version: SCHEMA_VERSION,
            id: "@clay/test-design-system".to_string(),
            display_name: "Test Design System".to_string(),
            extends: None,
            values,
            recipes,
        };

        let json = serde_json::to_string(&decl).unwrap();
        let deserialized: UiDesignSystemDeclaration = serde_json::from_str(&json).unwrap();
        assert_eq!(decl, deserialized);
    }

    #[test]
    fn bounds_and_limits_are_enforced() {
        // Out of bounds border width
        let invalid_border = ComponentRecipeDeclaration {
            border_width: Some(10.0), // max is 8.0
            ..Default::default()
        };
        let key = RecipeKey::parse("button.root.rest").unwrap();
        assert!(invalid_border.validate(&key).is_err());

        // Out of bounds blur
        let invalid_blur = ComponentRecipeDeclaration {
            backdrop_blur: Some(40.0), // max is 32.0
            ..Default::default()
        };
        assert!(invalid_blur.validate(&key).is_err());

        // Out of bounds shadow layers
        let invalid_shadow = ComponentRecipeDeclaration {
            shadow: Some(vec![
                ShadowLayer {
                    x: 0.0,
                    y: 1.0,
                    blur: 2.0,
                    spread: 0.0,
                    color_role: ThemeColorRef::parse("surface.control").unwrap(),
                    opacity: 0.5,
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 2.0,
                    blur: 4.0,
                    spread: 0.0,
                    color_role: ThemeColorRef::parse("surface.control").unwrap(),
                    opacity: 0.5,
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 3.0,
                    blur: 6.0,
                    spread: 0.0,
                    color_role: ThemeColorRef::parse("surface.control").unwrap(),
                    opacity: 0.5,
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 4.0,
                    blur: 8.0,
                    spread: 0.0,
                    color_role: ThemeColorRef::parse("surface.control").unwrap(),
                    opacity: 0.5,
                    inset: false,
                }, // 4 layers exceeds MAX_SHADOW_LAYERS (3)
            ]),
            ..Default::default()
        };
        assert!(invalid_shadow.validate(&key).is_err());

        // Unsupported schema version
        let invalid_version = UiDesignSystemDeclaration {
            schema_version: 99,
            id: "@clay/invalid".to_string(),
            display_name: "Invalid".to_string(),
            extends: None,
            values: BTreeMap::new(),
            recipes: BTreeMap::new(),
        };
        assert!(invalid_version.validate().is_err());
    }

    #[test]
    fn resolution_fills_missing_properties_from_core_fallbacks() {
        let mut recipes = BTreeMap::new();
        // Override only background_color on button.primary.root.rest
        recipes.insert(
            RecipeKey::parse("button.primary.root.rest").unwrap(),
            ComponentRecipeDeclaration {
                background_color: Some(ThemeColorRef::parse("accent.muted").unwrap()),
                border_radius: Some(8.0),
                ..Default::default()
            },
        );

        let decl = UiDesignSystemDeclaration {
            schema_version: SCHEMA_VERSION,
            id: "@clay/custom-ds".to_string(),
            display_name: "Custom DS".to_string(),
            extends: None,
            values: BTreeMap::new(),
            recipes,
        };

        let resolved = resolve_design_system(&decl, None).unwrap();
        let button_primary = resolved
            .recipes
            .get(&RecipeKey::parse("button.primary.root.rest").unwrap())
            .unwrap();

        // Custom override applied
        assert_eq!(button_primary.background_color.as_str(), "accent.muted");
        assert!((button_primary.border_radius - 8.0).abs() < f64::EPSILON);
        // Fallback properties filled in from core: the shipped primary button is a
        // fill-only control with no border of its own (DESIGN.md §11).
        assert!((button_primary.border_width - 0.0).abs() < f64::EPSILON);
        assert_eq!(button_primary.border_style, BorderStyle::None);
        assert_eq!(button_primary.text_color.as_str(), "surface.main");

        // Non-overridden recipes (e.g. modal) populated from core
        let modal_dialog = resolved
            .recipes
            .get(&RecipeKey::parse("modal.default.dialog.rest").unwrap())
            .unwrap();
        assert_eq!(modal_dialog.background_color.as_str(), "surface.overlay");
        // The core fallback supplies the dialog's own geometry: one hairline, radius 16,
        // and the approved overlay stack (DESIGN.md §11 panels/overlays).
        assert!((modal_dialog.border_width - 1.0).abs() < f64::EPSILON);
        assert!((modal_dialog.border_radius - 16.0).abs() < f64::EPSILON);
        assert_eq!(modal_dialog.shadow.len(), 2);
    }
}
