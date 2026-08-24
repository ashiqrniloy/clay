//! Renderer-neutral typography snapshot used by package geometry compatibility.
//!
//! React resolves families and OpenType features in CSS/CodeMirror. Rust keeps
//! only validated role sizes and semantic hierarchy ratios for bounded DTO and
//! legacy layout-state validation.

use crate::protocol::{ActiveTypography, FontRole, UiTypographyHierarchy};

pub(crate) const DOCUMENT_LINE_HEIGHT_MULTIPLIER: f64 = 1.4;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UiTextMetrics {
    pub(crate) line_height: f64,
    pub(crate) row_height: f64,
}

impl UiTextMetrics {
    const LINE_HEIGHT_MULTIPLIER: f64 = 1.2;
    const ROW_VERTICAL_PADDING: f64 = 11.6;
    const LIST_VERTICAL_PADDING: f64 = 9.6;

    fn new(font_size: f32) -> Self {
        let line_height = f64::from(font_size) * Self::LINE_HEIGHT_MULTIPLIER;
        Self {
            line_height,
            row_height: line_height + Self::ROW_VERTICAL_PADDING,
        }
    }

    pub(crate) fn list_height(self, detail: Self) -> f64 {
        self.line_height + detail.line_height + Self::LIST_VERTICAL_PADDING
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TypographyRegistry {
    active: ActiveTypography,
}

impl TypographyRegistry {
    pub(crate) fn ui_text_metrics(&self, role: FontRole, variant: UiTextVariant) -> UiTextMetrics {
        let profile = match role {
            FontRole::Monospace => &self.active.monospace,
            FontRole::Proportional => &self.active.proportional,
            FontRole::Ui => &self.active.ui,
        };
        UiTextMetrics::new(profile.size * variant.scale(&self.active.hierarchy))
    }

    #[allow(dead_code)]
    pub(crate) fn document_line_height(&self) -> f64 {
        f64::from(
            self.active
                .monospace
                .size
                .max(self.active.proportional.size),
        ) * DOCUMENT_LINE_HEIGHT_MULTIPLIER
    }
}
