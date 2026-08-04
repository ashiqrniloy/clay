//! Client-owned resolved typography profiles.
//!
//! Wire profiles are validated and converted once when a bootstrap or live
//! update arrives. Paint and layout consume the cached Parley family lists.

use std::borrow::Cow;

use masonry::parley::style::{FontFamily, FontFeature, FontSettings, FontStack, GenericFamily};
use masonry::parley::swash::tag_from_str_lossy;

use crate::protocol::{
    ActiveTypography, ActiveTypographyValidationError, FontProfile, FontRole, LigaturePolicy,
    UiTypographyHierarchy,
};

/// Shared conservative line-height policy for viewport extraction and logical
/// scroll progression. Visible Parley layout/caret metrics remain exact.
pub(crate) const DOCUMENT_LINE_HEIGHT_MULTIPLIER: f64 = 1.4;

/// Native UI text variants are semantic scale choices, never package-provided
/// point sizes. The configured role selects both its family stack and base size;
/// the active [`UiTypographyHierarchy`] supplies the bounded scale ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiTextVariant {
    Body,
    Status,
    Title,
    Detail,
    Display,
    Section,
    Caption,
}

impl UiTextVariant {
    pub(crate) fn from_typography_token(token: &str) -> Self {
        match token {
            "typography.display" => Self::Display,
            "typography.title" => Self::Title,
            "typography.section" => Self::Section,
            "typography.status" => Self::Status,
            "typography.detail" => Self::Detail,
            "typography.caption" => Self::Caption,
            _ => Self::Body,
        }
    }

    /// Resolve this variant's bounded scale ratio against the active hierarchy.
    /// A cached numeric read; never parses strings or allocates.
    fn scale(self, hierarchy: &UiTypographyHierarchy) -> f32 {
        match self {
            Self::Body => hierarchy.body,
            Self::Status => hierarchy.status,
            Self::Title => hierarchy.title,
            Self::Detail => hierarchy.detail,
            Self::Display => hierarchy.display,
            Self::Section => hierarchy.section,
            Self::Caption => hierarchy.caption,
        }
    }
}

/// Cached-profile UI metrics shared by native text paint, rows, hit regions,
/// scroll increments, and accessibility bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UiTextMetrics {
    pub(crate) font_size: f32,
    pub(crate) line_height: f64,
    pub(crate) row_height: f64,
}

impl UiTextMetrics {
    pub(crate) const LINE_HEIGHT_MULTIPLIER: f64 = 1.2;
    const ROW_VERTICAL_PADDING: f64 = 11.6;
    pub(crate) const STATUS_VERTICAL_PADDING: f64 = 13.6;
    pub(crate) const BUTTON_VERTICAL_PADDING: f64 = 6.0;
    const LIST_VERTICAL_PADDING: f64 = 9.6;

    fn new(font_size: f32) -> Self {
        let line_height = f64::from(font_size) * Self::LINE_HEIGHT_MULTIPLIER;
        Self {
            font_size,
            line_height,
            row_height: line_height + Self::ROW_VERTICAL_PADDING,
        }
    }

    pub(crate) fn button_height(self) -> f64 {
        self.row_height + Self::BUTTON_VERTICAL_PADDING
    }

    pub(crate) fn list_height(self, detail: Self) -> f64 {
        self.line_height + detail.line_height + Self::LIST_VERTICAL_PADDING
    }

    pub(crate) fn status_height(self) -> f64 {
        self.line_height + Self::STATUS_VERTICAL_PADDING
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedFontProfile {
    families: Vec<FontFamily<'static>>,
    size: f32,
    /// Resolved OpenType feature list for this profile, sorted by tag with the
    /// last-declared value winning duplicates. Built once at install time from
    /// the user-owned `LigaturePolicy`; consumed by parley at shape time.
    features: Vec<FontFeature>,
}

impl ResolvedFontProfile {
    fn from_wire(profile: &FontProfile, fallback: GenericFamily) -> Self {
        let mut families = profile
            .families
            .iter()
            .map(|family| match GenericFamily::parse(family) {
                Some(generic) => FontFamily::Generic(generic),
                None => FontFamily::Named(Cow::Owned(family.clone())),
            })
            .collect::<Vec<_>>();
        if !families
            .iter()
            .any(|family| matches!(family, FontFamily::Generic(_)))
        {
            families.push(FontFamily::Generic(fallback));
        }
        Self {
            families,
            size: profile.size,
            features: resolve_font_features(&profile.ligatures),
        }
    }

    pub(crate) fn font_stack(&self) -> FontStack<'_> {
        FontStack::List(Cow::Borrowed(&self.families))
    }

    pub(crate) fn size(&self) -> f32 {
        self.size
    }

    /// Resolved OpenType feature settings for this profile, borrowed for the
    /// lifetime of the profile. Pushed into parley as `StyleProperty::FontFeatures`.
    pub(crate) fn font_features(&self) -> FontSettings<'_, FontFeature> {
        FontSettings::List(Cow::Borrowed(&self.features))
    }

    /// Stable hash of the resolved feature list, used to key the layout cache so
    /// a ligature-policy change invalidates cached glyphs even if the layout
    /// style revision is unchanged. Zero for the empty (font-default) policy.
    pub(crate) fn feature_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for feature in &self.features {
            hash ^= u64::from(feature.tag);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            hash ^= u64::from(feature.value);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    #[cfg(test)]
    fn families(&self) -> &[FontFamily<'static>] {
        &self.families
    }
}

/// Resolve a `LigaturePolicy` into a sorted, deduplicated feature list. Semantic
/// toggles emit explicit on/off values; `raw_features` is parsed via `swash`;
/// `disable_features` is applied last so it overrides everything else.
/// `BTreeMap` gives tag-sorted output with last-declared-wins dedup.
fn resolve_font_features(policy: &LigaturePolicy) -> Vec<FontFeature> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<u32, u16> = BTreeMap::new();
    if policy.enable_standard {
        map.insert(tag_from_str_lossy("liga"), 1);
        map.insert(tag_from_str_lossy("clig"), 1);
    } else {
        map.insert(tag_from_str_lossy("liga"), 0);
        map.insert(tag_from_str_lossy("clig"), 0);
    }
    if policy.enable_contextual {
        map.insert(tag_from_str_lossy("calt"), 1);
    } else {
        map.insert(tag_from_str_lossy("calt"), 0);
    }
    for feature in &policy.discretionary_features {
        map.insert(tag_from_str_lossy(feature), 1);
    }
    if let Some(raw) = &policy.raw_features {
        for parsed in FontFeature::parse_list(raw) {
            map.insert(parsed.tag, parsed.value);
        }
    }
    for feature in &policy.disable_features {
        map.insert(tag_from_str_lossy(feature), 0);
    }
    map.into_iter()
        .map(|(tag, value)| FontFeature { tag, value })
        .collect()
}

/// Installed typography snapshot plus cached Parley representations.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TypographyRegistry {
    active: ActiveTypography,
    monospace: ResolvedFontProfile,
    proportional: ResolvedFontProfile,
    ui: ResolvedFontProfile,
    hierarchy: UiTypographyHierarchy,
}

impl Default for TypographyRegistry {
    fn default() -> Self {
        Self::from_active_typography(ActiveTypography::default())
            .expect("default typography must be valid")
    }
}

impl TypographyRegistry {
    pub(crate) fn from_active_typography(
        active: ActiveTypography,
    ) -> Result<Self, ActiveTypographyValidationError> {
        active.validate()?;
        Ok(Self {
            monospace: ResolvedFontProfile::from_wire(&active.monospace, GenericFamily::Monospace),
            proportional: ResolvedFontProfile::from_wire(
                &active.proportional,
                GenericFamily::SansSerif,
            ),
            ui: ResolvedFontProfile::from_wire(&active.ui, GenericFamily::SystemUi),
            hierarchy: active.hierarchy,
            active,
        })
    }

    /// Install only a strictly newer validated server snapshot. Equal revisions
    /// are no-ops so duplicate broadcasts do not cause layout churn.
    pub(crate) fn install(
        &mut self,
        active: ActiveTypography,
    ) -> Result<bool, ActiveTypographyValidationError> {
        if active.revision <= self.active.revision {
            return Ok(false);
        }
        *self = Self::from_active_typography(active)?;
        Ok(true)
    }

    pub(crate) fn profile(&self, role: FontRole) -> &ResolvedFontProfile {
        match role {
            FontRole::Monospace => &self.monospace,
            FontRole::Proportional => &self.proportional,
            FontRole::Ui => &self.ui,
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.active.revision
    }

    /// Conservative document geometry for mixed prose/code lines. UI is not a
    /// document role, so only the two document profiles participate.
    pub(crate) fn document_line_height(&self) -> f64 {
        f64::from(self.monospace.size.max(self.proportional.size)) * DOCUMENT_LINE_HEIGHT_MULTIPLIER
    }

    pub(crate) fn ui_text_metrics(&self, role: FontRole, variant: UiTextVariant) -> UiTextMetrics {
        UiTextMetrics::new(self.profile(role).size() * variant.scale(&self.hierarchy))
    }

    /// Expose the active hierarchy for tests and revision/invalidation checks.
    #[cfg(test)]
    pub(crate) fn hierarchy(&self) -> UiTypographyHierarchy {
        self.hierarchy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_registry_resolves_each_role_and_revision() {
        let mut active = ActiveTypography {
            revision: 4,
            ..ActiveTypography::default()
        };
        active.monospace.size = 15.0;
        active.proportional.size = 17.0;
        active.ui.size = 13.0;
        let registry = TypographyRegistry::from_active_typography(active).unwrap();

        assert_eq!(registry.revision(), 4);
        assert_eq!(registry.profile(FontRole::Monospace).size(), 15.0);
        assert_eq!(registry.profile(FontRole::Proportional).size(), 17.0);
        assert_eq!(registry.profile(FontRole::Ui).size(), 13.0);
    }

    #[test]
    fn missing_named_family_retains_generic_fallback() {
        let mut active = ActiveTypography::default();
        active.monospace.families = vec!["not-installed".to_string(), "monospace".to_string()];
        let registry = TypographyRegistry::from_active_typography(active).unwrap();

        assert!(matches!(
            registry.profile(FontRole::Monospace).families().last(),
            Some(FontFamily::Generic(GenericFamily::Monospace))
        ));
    }

    #[test]
    fn unchanged_typography_revision_does_not_invalidate_layout() {
        let mut registry = TypographyRegistry::default();
        assert!(!registry.install(ActiveTypography::default()).unwrap());
    }

    #[test]
    fn document_line_height_uses_largest_document_profile_not_ui() {
        let mut active = ActiveTypography::default();
        active.monospace.size = 16.0;
        active.proportional.size = 24.0;
        active.ui.size = 96.0;
        let registry = TypographyRegistry::from_active_typography(active).unwrap();

        assert!((registry.document_line_height() - 33.6).abs() < 0.001);
    }

    #[test]
    fn ui_variants_scale_from_configured_role_size() {
        let mut active = ActiveTypography::default();
        active.ui.size = 20.0;
        let registry = TypographyRegistry::from_active_typography(active).unwrap();

        assert_eq!(
            registry
                .ui_text_metrics(FontRole::Ui, UiTextVariant::Body)
                .font_size,
            20.0
        );
        assert_eq!(
            registry
                .ui_text_metrics(FontRole::Ui, UiTextVariant::Status)
                .font_size,
            20.0
        );
        assert!(
            (registry
                .ui_text_metrics(FontRole::Ui, UiTextVariant::Title)
                .font_size
                - 20.0 * 14.0 / 12.0)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn ui_typography_hierarchy_defaults_preserve_existing_variant_metrics() {
        let h = UiTypographyHierarchy::DEFAULT;
        // Legacy scales are preserved exactly.
        assert!((h.body - 1.0).abs() < 1e-6);
        assert!((h.status - 1.0).abs() < 1e-6);
        assert!((h.title - 14.0 / 12.0).abs() < 1e-6);
        assert!((h.detail - 10.0 / 12.0).abs() < 1e-6);
        // New Phase 20.1 variants have restrained, monotonic defaults.
        assert!(h.display > h.title);
        assert!(h.title > h.section);
        assert!(h.section > h.body);
        assert!(h.caption < h.body);
        // Default ActiveTypography carries the default hierarchy.
        let registry =
            TypographyRegistry::from_active_typography(ActiveTypography::default()).unwrap();
        assert_eq!(registry.hierarchy(), UiTypographyHierarchy::DEFAULT);
        // Title/body/status still resolve to the legacy ratios through the registry.
        let ui = 12.0_f32; // default ActiveTypography.ui.size
        assert!(
            (registry
                .ui_text_metrics(FontRole::Ui, UiTextVariant::Body)
                .font_size
                - ui)
                .abs()
                < 1e-6
        );
        assert!(
            (registry
                .ui_text_metrics(FontRole::Ui, UiTextVariant::Title)
                .font_size
                - ui * 14.0 / 12.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn display_section_and_caption_scale_from_selected_font_role() {
        let mut active = ActiveTypography::default();
        active.ui.size = 20.0;
        active.monospace.size = 16.0;
        let registry = TypographyRegistry::from_active_typography(active).unwrap();
        let h = UiTypographyHierarchy::DEFAULT;

        // Each new variant scales the *selected* role's base size by its ratio.
        for (role, base) in [(FontRole::Ui, 20.0_f32), (FontRole::Monospace, 16.0_f32)] {
            assert!(
                (registry
                    .ui_text_metrics(role, UiTextVariant::Display)
                    .font_size
                    - base * h.display)
                    .abs()
                    < 1e-4
            );
            assert!(
                (registry
                    .ui_text_metrics(role, UiTextVariant::Section)
                    .font_size
                    - base * h.section)
                    .abs()
                    < 1e-4
            );
            assert!(
                (registry
                    .ui_text_metrics(role, UiTextVariant::Caption)
                    .font_size
                    - base * h.caption)
                    .abs()
                    < 1e-4
            );
            // Row/line geometry derives from the same scaled font size.
            let m = registry.ui_text_metrics(role, UiTextVariant::Display);
            assert!(
                (m.line_height - (base * h.display) as f64 * UiTextMetrics::LINE_HEIGHT_MULTIPLIER)
                    .abs()
                    < 1e-3
            );
            assert!(m.row_height > m.line_height);
        }
    }

    #[test]
    fn custom_hierarchy_updates_layout_hit_and_accessibility_geometry_together() {
        let mut active = ActiveTypography::default();
        active.ui.size = 20.0;
        active.hierarchy = UiTypographyHierarchy {
            display: 2.0,
            title: 1.5,
            section: 1.25,
            body: 1.0,
            status: 1.0,
            detail: 0.875,
            caption: 0.7,
        };
        let registry = TypographyRegistry::from_active_typography(active).unwrap();

        // A single hierarchy change moves font size, line height, and row
        // (hit/accessibility) height together, since they all derive from one
        // `ui_text_metrics` call.
        let display = registry.ui_text_metrics(FontRole::Ui, UiTextVariant::Display);
        assert!((display.font_size - 40.0).abs() < 1e-4);
        assert!(
            (display.line_height - 40.0_f64 * UiTextMetrics::LINE_HEIGHT_MULTIPLIER).abs() < 1e-3
        );
        assert!((display.row_height - display.line_height - 11.6).abs() < 1e-3);
        // Caption shrinks below body.
        let caption = registry.ui_text_metrics(FontRole::Ui, UiTextVariant::Caption);
        assert!((caption.font_size - 14.0).abs() < 1e-4);
        assert!(caption.row_height < display.row_height);
        // Custom hierarchy is cached on the registry.
        assert!((registry.hierarchy().display - 2.0).abs() < 1e-6);
    }

    #[test]
    fn unchanged_hierarchy_does_not_invalidate_layout() {
        // Equal snapshots (profiles + hierarchy) keep their revision in
        // `stage_typography`; the registry's `install` is a no-op for equal
        // revisions, so layout does not churn.
        let mut registry = TypographyRegistry::default();
        let same = ActiveTypography::default();
        assert!(!registry.install(same).unwrap());
        // A hierarchy-only change bumps the revision and invalidates once.
        let mut changed_hierarchy = UiTypographyHierarchy::DEFAULT;
        changed_hierarchy.display = 1.75;
        let changed = ActiveTypography {
            revision: 1,
            hierarchy: changed_hierarchy,
            ..ActiveTypography::default()
        };
        assert!(registry.install(changed).unwrap());
        // Installing the same changed revision again is a no-op.
        let again = ActiveTypography {
            revision: 1,
            hierarchy: changed_hierarchy,
            ..ActiveTypography::default()
        };
        assert!(!registry.install(again).unwrap());
    }

    #[test]
    fn invalid_partial_or_extreme_hierarchy_is_rejected_atomically() {
        use crate::protocol::{HIERARCHY_SCALE_MAX, UiTypographyHierarchyValidationError};

        let bad =
            |mut h: UiTypographyHierarchy, field: &str, value: f32| -> UiTypographyHierarchy {
                match field {
                    "display" => h.display = value,
                    "title" => h.title = value,
                    "section" => h.section = value,
                    "body" => h.body = value,
                    "status" => h.status = value,
                    "detail" => h.detail = value,
                    "caption" => h.caption = value,
                    _ => unreachable!(),
                }
                h
            };

        for (field, value, case) in [
            ("display", f32::NAN, "non-finite"),
            ("title", f32::INFINITY, "infinite"),
            ("body", 0.0, "non-positive zero"),
            ("status", -1.0, "negative"),
            ("caption", HIERARCHY_SCALE_MAX + 0.1, "extreme high"),
            ("detail", HIERARCHY_SCALE_MAX, "at-max boundary is allowed"),
        ] {
            let h = bad(UiTypographyHierarchy::DEFAULT, field, value);
            let result = h.validate();
            if case.contains("allowed") {
                assert!(result.is_ok(), "{case} should be valid");
            } else {
                assert_eq!(
                    result,
                    Err(UiTypographyHierarchyValidationError::InvalidScale { field }),
                    "{case} should be rejected for {field}"
                );
            }
        }

        // ActiveTypography.validate surfaces hierarchy errors atomically; a
        // bad hierarchy rejects the whole snapshot before any profile install.
        let mut active = ActiveTypography::default();
        active.hierarchy.display = f32::NAN;
        assert!(active.validate().is_err());
        // And a valid hierarchy on an otherwise-default snapshot still validates.
        assert!(ActiveTypography::default().validate().is_ok());
    }

    fn liga_value(features: &[super::FontFeature]) -> Option<u16> {
        features
            .iter()
            .find(|feature| feature.tag == super::tag_from_str_lossy("liga"))
            .map(|feature| feature.value)
    }

    fn calt_value(features: &[super::FontFeature]) -> Option<u16> {
        features
            .iter()
            .find(|feature| feature.tag == super::tag_from_str_lossy("calt"))
            .map(|feature| feature.value)
    }

    #[test]
    fn resolve_features_enables_standard_and_contextual_by_default() {
        let features = super::resolve_font_features(&LigaturePolicy::default());
        assert_eq!(liga_value(&features), Some(1));
        assert_eq!(calt_value(&features), Some(1));
        // clig is part of standard ligatures.
        assert!(
            features
                .iter()
                .any(|feature| feature.tag == super::tag_from_str_lossy("clig")
                    && feature.value == 1)
        );
    }

    #[test]
    fn resolve_features_disable_standard_turns_liga_off() {
        let policy = LigaturePolicy {
            enable_standard: false,
            ..LigaturePolicy::default()
        };
        let features = super::resolve_font_features(&policy);
        assert_eq!(liga_value(&features), Some(0));
        assert_eq!(calt_value(&features), Some(1));
    }

    #[test]
    fn resolve_features_disable_list_overrides_enable_toggle() {
        // enable_standard keeps liga on, but disable_features forces it off.
        let policy = LigaturePolicy {
            enable_standard: true,
            disable_features: vec!["liga".to_string()],
            ..LigaturePolicy::default()
        };
        let features = super::resolve_font_features(&policy);
        assert_eq!(liga_value(&features), Some(0));
        assert_eq!(calt_value(&features), Some(1));
    }

    #[test]
    fn resolve_features_raw_source_disables_liga_only() {
        // raw_features parses a CSS source; with standard on by default, the raw
        // 'liga' 0 disables liga while calt stays on.
        let policy = LigaturePolicy {
            raw_features: Some("'calt' 1, 'liga' 0".to_string()),
            ..LigaturePolicy::default()
        };
        let features = super::resolve_font_features(&policy);
        assert_eq!(liga_value(&features), Some(0));
        assert_eq!(calt_value(&features), Some(1));
        // clig untouched by the raw source.
        assert!(
            features
                .iter()
                .any(|feature| feature.tag == super::tag_from_str_lossy("clig")
                    && feature.value == 1)
        );
    }

    #[test]
    fn resolve_features_discretionary_tags_enabled() {
        let policy = LigaturePolicy {
            discretionary_features: vec!["ss01".to_string()],
            ..LigaturePolicy::default()
        };
        let features = super::resolve_font_features(&policy);
        assert!(
            features
                .iter()
                .any(|feature| feature.tag == super::tag_from_str_lossy("ss01")
                    && feature.value == 1)
        );
    }

    #[test]
    fn feature_hash_differs_for_on_vs_off_policy() {
        let on = super::resolve_font_features(&LigaturePolicy::default());
        let off = super::resolve_font_features(&LigaturePolicy {
            enable_standard: false,
            enable_contextual: false,
            ..LigaturePolicy::default()
        });
        let hash_on = {
            let p = ResolvedFontProfile {
                families: Vec::new(),
                size: 16.0,
                features: on,
            };
            p.feature_hash()
        };
        let hash_off = {
            let p = ResolvedFontProfile {
                families: Vec::new(),
                size: 16.0,
                features: off,
            };
            p.feature_hash()
        };
        assert_ne!(hash_on, hash_off);
    }

    #[test]
    fn per_role_policies_resolve_independently() {
        // A fixture typography where monospace disables ligatures and
        // proportional keeps them on: the two roles resolve different feature
        // lists (markdown-prose vs code get different shaping).
        let mut active = ActiveTypography::default();
        active.monospace.ligatures = Box::new(LigaturePolicy {
            enable_standard: false,
            enable_contextual: false,
            ..LigaturePolicy::default()
        });
        let registry = TypographyRegistry::from_active_typography(active).unwrap();
        let mono = registry.profile(FontRole::Monospace).font_features();
        let prop = registry.profile(FontRole::Proportional).font_features();
        let mono_list = match &mono {
            super::FontSettings::List(items) => items,
            _ => unreachable!("resolved features are always a list"),
        };
        let prop_list = match &prop {
            super::FontSettings::List(items) => items,
            _ => unreachable!("resolved features are always a list"),
        };
        assert_ne!(liga_value(mono_list), Some(1));
        assert_eq!(liga_value(prop_list), Some(1));
    }
}
