//! Client-owned resolved typography profiles.
//!
//! Wire profiles are validated and converted once when a bootstrap or live
//! update arrives. Paint and layout consume the cached Parley family lists.

use std::borrow::Cow;

use masonry::parley::style::{FontFamily, FontStack, GenericFamily};

use crate::protocol::{ActiveTypography, ActiveTypographyValidationError, FontProfile, FontRole};

/// Shared conservative line-height policy for viewport extraction and logical
/// scroll progression. Visible Parley layout/caret metrics remain exact.
pub(crate) const DOCUMENT_LINE_HEIGHT_MULTIPLIER: f64 = 1.4;

/// Native UI text variants are semantic scale choices, never package-provided
/// point sizes. The configured role selects both its family stack and base size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiTextVariant {
    Body,
    Status,
    Title,
    Detail,
}

impl UiTextVariant {
    pub(crate) fn from_typography_token(token: &str) -> Self {
        match token {
            "typography.title" => Self::Title,
            "typography.status" => Self::Status,
            _ => Self::Body,
        }
    }

    const fn scale(self) -> f32 {
        match self {
            Self::Body | Self::Status => 1.0,
            Self::Title => 14.0 / 12.0,
            Self::Detail => 10.0 / 12.0,
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
        UiTextMetrics::new(self.profile(role).size() * variant.scale())
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
}
