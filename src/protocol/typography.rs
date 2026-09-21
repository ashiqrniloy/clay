//! Typography contract: font roles, profiles, ligatures, and the active
//! typography snapshot.

/// Closed semantic profile selected by Clay-owned typography configuration.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FontRole {
    Monospace,
    Proportional,
    Ui,
}

/// Document-only role used for a document default or syntax/semantic override.
/// `Inherit` leaves a decoration on its document default; UI is deliberately
/// absent because diagnostics/search and document syntax cannot select it.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum DocumentFontRole {
    Inherit,
    Monospace,
    Proportional,
}

impl DocumentFontRole {
    pub const fn font_role(self) -> Option<FontRole> {
        match self {
            Self::Inherit => None,
            Self::Monospace => Some(FontRole::Monospace),
            Self::Proportional => Some(FontRole::Proportional),
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "inherit" => Some(Self::Inherit),
            "monospace" => Some(Self::Monospace),
            "proportional" => Some(Self::Proportional),
            _ => None,
        }
    }
}

impl FontRole {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "monospace" => Some(Self::Monospace),
            "proportional" => Some(Self::Proportional),
            "ui" => Some(Self::Ui),
            _ => None,
        }
    }
}

/// Wire form of one inert text-style override declared by a theme package
/// (`clay.contributions.textStyles`). Colors travel as RGBA bytes so the
/// protocol never depends on a peniko `Color`; the client reconstructs a
/// [`crate::editor::theme::StyleRegistry`] at the point the active theme is
/// applied. This is pure style data: no code, ops, widgets, or CSS (Plan 046,
/// decision 2026-07-09-0352).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct TextThemeOverride {
    /// Override target: a [`TokenType`] variant name or a base-UI color key
    /// (e.g. `Keyword`, `panelBg`).
    pub token: String,
    /// RGBA override, present only when the entry declares a color.
    pub color: Option<[u8; 4]>,
    /// Optional fill override (`#rrggbbaa`). Theme-resolved only; never a
    /// decoration-span wire field.
    pub background: Option<[u8; 4]>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// Size-ladder thousandths (`1500` = 1.5). Theme-owned; never a span field.
    pub scale: Option<u16>,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// A bounded ordered font-family fallback stack and logical-pixel size.
pub const MAX_FONT_FAMILIES_PER_PROFILE: usize = 8;

pub const MAX_FONT_FAMILY_BYTES: usize = 128;

pub const MIN_FONT_SIZE: f32 = 6.0;

pub const MAX_FONT_SIZE: f32 = 96.0;

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Default,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct FontProfile {
    pub families: Vec<String>,
    pub size: f32,
    /// Semantic OpenType feature policy for this profile. Boxed so growth of
    /// `LigaturePolicy` (it carries `Vec<String>` feature lists) does not
    /// inflate the `ServerMessage` union floor that small payloads like
    /// `EditAck` pay. User-owned typography data; packages declare semantic
    /// policy via behavior manifests, never concrete families/sizes.
    pub ligatures: Box<LigaturePolicy>,
}

/// Bounded, semantic OpenType feature policy. Carries no concrete family or
/// size data (those stay user-owned per `typography-role-ownership`), only
/// feature toggles a package or user may declare. Resolved client-side into a
/// `parley` `FontSettings<FontFeature>` list at typography install time.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct LigaturePolicy {
    /// Enable standard ligatures (`liga`, `clig`). Default `true` keeps the
    /// historical ligature-on shaping Clay relied on implicitly.
    pub enable_standard: bool,
    /// Enable contextual ligatures (`calt`). Default `true`.
    pub enable_contextual: bool,
    /// Additional discretionary feature tags to enable (value 1), e.g. `ss01`.
    pub discretionary_features: Vec<String>,
    /// Raw CSS feature-settings source passthrough, e.g. `"'calt' 1, 'liga' 0"`.
    /// Parsed by `swash` `Setting::parse_list`; applied before `disable_features`.
    pub raw_features: Option<String>,
    /// Feature tags to force off (value 0), applied last so they override the
    /// semantic toggles and `raw_features`.
    pub disable_features: Vec<String>,
}

impl Default for LigaturePolicy {
    /// Ligatures on by default reproduces the implicit shaping Clay relied on
    /// before feature control was exposed; users or packages opt out by setting
    /// `enable_standard`/`enable_contextual` false or listing `disable_features`.
    fn default() -> Self {
        Self {
            enable_standard: true,
            enable_contextual: true,
            discretionary_features: Vec::new(),
            raw_features: None,
            disable_features: Vec::new(),
        }
    }
}

/// Upper bound on the number of feature tags per kind (`discretionary_features`
/// and `disable_features`). OpenType fonts expose a handful of named features;
/// 32 is generous and keeps the archived payload bounded.
pub const MAX_LIGATURE_FEATURES_PER_KIND: usize = 32;

/// Upper bound on the raw CSS feature-settings source string length.
pub const MAX_LIGATURE_RAW_FEATURE_BYTES: usize = 256;

/// OpenType feature tags are exactly four ASCII bytes; shorter tags are
/// space-padded by `swash::tag_from_str_lossy`.
pub const MAX_LIGATURE_FEATURE_NAME_BYTES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontProfileValidationError {
    EmptyFamilyStack,
    TooManyFamilies,
    EmptyFamily,
    FamilyTooLong,
    ControlCharacter,
    MissingGenericFallback,
    InvalidSize,
    TooManyDiscretionaryFeatures,
    TooManyDisabledFeatures,
    RawFeaturesTooLong,
    InvalidFeatureName,
}

impl FontProfile {
    pub fn validate(&self) -> Result<(), FontProfileValidationError> {
        if self.families.is_empty() {
            return Err(FontProfileValidationError::EmptyFamilyStack);
        }
        if self.families.len() > MAX_FONT_FAMILIES_PER_PROFILE {
            return Err(FontProfileValidationError::TooManyFamilies);
        }
        for family in &self.families {
            if family.trim().is_empty() {
                return Err(FontProfileValidationError::EmptyFamily);
            }
            if family.len() > MAX_FONT_FAMILY_BYTES {
                return Err(FontProfileValidationError::FamilyTooLong);
            }
            if family.chars().any(char::is_control) {
                return Err(FontProfileValidationError::ControlCharacter);
            }
        }
        if !self
            .families
            .last()
            .is_some_and(|family| is_generic_font_family(family))
        {
            return Err(FontProfileValidationError::MissingGenericFallback);
        }
        if !self.size.is_finite() || !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.size) {
            return Err(FontProfileValidationError::InvalidSize);
        }
        self.ligatures.validate()
    }
}

impl LigaturePolicy {
    /// Validate bounds and feature-tag shape. Packages supply these strings, so
    /// they are a trust boundary: counts and lengths are capped and each tag
    /// must be 1..=4 ASCII bytes (no control characters) so it maps to a real
    /// OpenType tag without carrying arbitrary payload.
    pub fn validate(&self) -> Result<(), FontProfileValidationError> {
        if self.discretionary_features.len() > MAX_LIGATURE_FEATURES_PER_KIND {
            return Err(FontProfileValidationError::TooManyDiscretionaryFeatures);
        }
        if self.disable_features.len() > MAX_LIGATURE_FEATURES_PER_KIND {
            return Err(FontProfileValidationError::TooManyDisabledFeatures);
        }
        if let Some(raw) = &self.raw_features
            && raw.len() > MAX_LIGATURE_RAW_FEATURE_BYTES
        {
            return Err(FontProfileValidationError::RawFeaturesTooLong);
        }
        for feature in self
            .discretionary_features
            .iter()
            .chain(&self.disable_features)
        {
            if feature.is_empty()
                || feature.len() > MAX_LIGATURE_FEATURE_NAME_BYTES
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b' ' || byte == b'-')
            {
                return Err(FontProfileValidationError::InvalidFeatureName);
            }
        }
        Ok(())
    }
}

fn is_generic_font_family(family: &str) -> bool {
    matches!(
        family,
        "system-ui" | "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy"
    )
}

/// Complete typography snapshot transported separately from [`ActiveTheme`].
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct ActiveTypography {
    pub revision: u64,
    pub monospace: FontProfile,
    pub proportional: FontProfile,
    pub ui: FontProfile,
    /// Phase 20.1: user-owned bounded hierarchy of UI variant scale ratios
    /// carried atomically with the typography snapshot. Packages/components
    /// select a semantic role plus variant only; concrete scales stay
    /// user-owned here. Defaults give the host baseline rhythm:
    /// Title 15/13, Body/Status = 1, Detail 12/13 (plan 110 task 9).
    pub hierarchy: UiTypographyHierarchy,
}

/// Bounded hierarchy of UI text-variant scale ratios. Each scale multiplies
/// the selected role's base size; packages cannot supply these values. All
/// scales must be finite and within `(0, HIERARCHY_SCALE_MAX]`.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct UiTypographyHierarchy {
    pub display: f32,
    pub title: f32,
    pub section: f32,
    pub body: f32,
    pub status: f32,
    pub detail: f32,
    pub caption: f32,
}

/// Inclusive upper bound for any hierarchy scale ratio. Generous enough for
/// large display headings while keeping cached geometry within sane layout
/// bounds. Lower bound is exclusive-zero (scales must be strictly positive).
pub const HIERARCHY_SCALE_MIN: f32 = 0.0;

pub const HIERARCHY_SCALE_MAX: f32 = 4.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiTypographyHierarchyValidationError {
    InvalidScale { field: &'static str },
}

impl UiTypographyHierarchy {
    /// Default hierarchy for the host baseline (ui base size 13): Display
    /// 19.5px, Title 15px semibold, Section ≈14px, Body/Status 13px regular,
    /// Detail/label 12px medium. Concrete sizes stay user-owned through
    /// `theme.setTypography`; these scales only set the default rhythm.
    pub const DEFAULT: Self = Self {
        display: 1.5,
        title: 15.0 / 13.0,
        section: 13.0 / 12.0,
        body: 1.0,
        status: 1.0,
        detail: 12.0 / 13.0,
        caption: 0.75,
    };

    pub fn validate(&self) -> Result<(), UiTypographyHierarchyValidationError> {
        let scales: [(&'static str, f32); 7] = [
            ("display", self.display),
            ("title", self.title),
            ("section", self.section),
            ("body", self.body),
            ("status", self.status),
            ("detail", self.detail),
            ("caption", self.caption),
        ];
        for (field, scale) in scales {
            if !scale.is_finite() || scale <= HIERARCHY_SCALE_MIN || scale > HIERARCHY_SCALE_MAX {
                return Err(UiTypographyHierarchyValidationError::InvalidScale { field });
            }
        }
        Ok(())
    }
}

impl Default for UiTypographyHierarchy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTypographyValidationError {
    InvalidProfile {
        role: FontRole,
        source: FontProfileValidationError,
    },
    InvalidHierarchy {
        source: UiTypographyHierarchyValidationError,
    },
}

impl ActiveTypography {
    pub fn profile(&self, role: FontRole) -> &FontProfile {
        match role {
            FontRole::Monospace => &self.monospace,
            FontRole::Proportional => &self.proportional,
            FontRole::Ui => &self.ui,
        }
    }

    pub fn validate(&self) -> Result<(), ActiveTypographyValidationError> {
        for role in [FontRole::Monospace, FontRole::Proportional, FontRole::Ui] {
            self.profile(role).validate().map_err(|source| {
                ActiveTypographyValidationError::InvalidProfile { role, source }
            })?;
        }
        self.hierarchy
            .validate()
            .map_err(|source| ActiveTypographyValidationError::InvalidHierarchy { source })?;
        Ok(())
    }
}

impl Default for ActiveTypography {
    fn default() -> Self {
        Self {
            revision: 0,
            monospace: FontProfile {
                families: vec!["monospace".to_string()],
                size: 20.0,
                ligatures: Box::new(LigaturePolicy::default()),
            },
            proportional: FontProfile {
                families: vec!["sans-serif".to_string()],
                size: 20.0,
                ligatures: Box::new(LigaturePolicy::default()),
            },
            ui: FontProfile {
                families: vec!["system-ui".to_string()],
                size: 13.0,
                ligatures: Box::new(LigaturePolicy::default()),
            },
            hierarchy: UiTypographyHierarchy::DEFAULT,
        }
    }
}
