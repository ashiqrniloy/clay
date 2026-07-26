//! Client-owned resolved typography profiles.
//!
//! Wire profiles are validated and converted once when a bootstrap or live
//! update arrives. Paint and layout consume the cached Parley family lists.

use std::borrow::Cow;

use masonry::parley::style::{FontFamily, FontStack, GenericFamily};

use crate::protocol::{
    ActiveTypography, ActiveTypographyValidationError, FontProfile, FontRole, UiTypographyHierarchy,
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
        }
    }

    pub(crate) fn font_stack(&self) -> FontStack<'_> {
        FontStack::List(Cow::Borrowed(&self.families))
    }

    pub(crate) fn size(&self) -> f32 {
        self.size
    }

    #[cfg(test)]
    fn families(&self) -> &[FontFamily<'static>] {
        &self.families
    }
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
}
