//! Validation of active-theme design-token overrides and the WCAG contrast
//! floors every installable theme must clear (plan 133 task 7 split of
//! `src/shell/theme.rs`).

use crate::color::Color;

use super::*;

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
/// border, structural boundaries, state fills) and standalone UI chips (`kbd`),
/// which are not prose. WCAG 2.1 non-text contrast (SC 1.4.11) floor is 3.0.
pub(crate) const UI_CONTRAST_MIN: f64 = 3.0;

/// Visibility floor for `border.hairline`, the decorative zone separator
/// (`DESIGN.md` §10.1: the theme's border grey at 34 %). It is deliberately
/// *exempt* from [`UI_CONTRAST_MIN`] — a hairline is required to be quieter
/// than `border.subtle`, which 34 % ink cannot be at 3:1 on either surface —
/// but it must never become invisible, so it keeps a floor of its own: the
/// smallest step that is still a perceivable edge on light or dark chrome.
/// `border.subtle`/`border.strong` carry the structural 3:1 requirement.
///
/// ponytail: a flat visibility floor, not a luminance-difference model; raise
/// it only if a hairline that passes 1.2:1 is reported as unreadable.
pub(crate) const HAIRLINE_VISIBILITY_MIN: f64 = 1.2;

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
///
/// Every pair is measured **composited**: the foreground role is alpha-blended
/// over the background role first (`crate::editor::theme::composited_contrast_ratio`),
/// because alpha is what the user sees — a 34 % hairline or a 75 % accent is not
/// the opaque color its RGB bytes describe. Measured values per theme live in
/// `design-artifacts/approved/quiet-instrument-migration/theme-values.md` and are
/// re-derived by `tests/theme_packages.rs`.
///
/// Groups and their floors:
/// - prose: `text.*` on the surface it is drawn on (4.5, WCAG 2.1 SC 1.4.3).
/// - affordances: accent, focus ring, focus border on the canvas (3.0, SC 1.4.11).
/// - structural boundaries: `border.subtle` and `border.strong` on both surfaces a
///   boundary is drawn against — canvas and panel (3.0). A control outline that
///   cannot be seen is a broken control, not a style choice.
/// - hairline: [`HAIRLINE_VISIBILITY_MIN`], visibility only (see that constant).
///
/// State fills are gated by [`REQUIRED_FILL_PAIRS`] instead: they are painted
/// *under* their text, so the pair needs the surface they sit on.
///
/// ponytail: floors are checked against the raw roles. Host CSS may attenuate a
/// disabled control further (`opacity.disabled` on top of `text.disabled`), which
/// is exempt from WCAG and measured by no pair here.
pub(crate) const REQUIRED_CONTRAST_PAIRS: &[(&str, &str, f64)] = &[
    // Prose: body/label/tooltip text on the surface it is drawn on.
    ("text.primary", "surface.main", TEXT_CONTRAST_MIN),
    ("text.muted", "surface.panel", TEXT_CONTRAST_MIN),
    ("text.primary", "surface.panel", TEXT_CONTRAST_MIN),
    ("text.primary", "surface.control", TEXT_CONTRAST_MIN),
    ("text.badge", "surface.badge", TEXT_CONTRAST_MIN),
    ("text.kbd", "surface.kbd", UI_CONTRAST_MIN),
    ("text.tooltip", "surface.tooltip", TEXT_CONTRAST_MIN),
    ("text.disabled", "surface.main", TEXT_CONTRAST_MIN),
    // Affordances: accent and focus must be visible on the canvas.
    ("accent.primary", "surface.main", UI_CONTRAST_MIN),
    ("accent.muted", "surface.main", UI_CONTRAST_MIN),
    ("focus.ring", "surface.main", UI_CONTRAST_MIN),
    ("border.focus", "surface.main", UI_CONTRAST_MIN),
    // Structural boundaries: a zone edge must be perceivable on both surfaces it
    // separates. Hairlines are decorative and keep only the visibility floor.
    ("border.subtle", "surface.main", UI_CONTRAST_MIN),
    ("border.subtle", "surface.panel", UI_CONTRAST_MIN),
    ("border.strong", "surface.main", UI_CONTRAST_MIN),
    ("border.hairline", "surface.main", HAIRLINE_VISIBILITY_MIN),
    ("border.hairline", "surface.panel", HAIRLINE_VISIBILITY_MIN),
];

/// Required ``(text, fill, surface, threshold)`` triples: the text role must stay
/// readable on a state fill.
///
/// The fill is composited over the surface it is painted on first (a 40 %
/// selection tint is a light wash over the canvas, not the raw bytes), then the
/// text is composited over *that* and measured against it — the layer order the
/// user actually sees: surface, fill, text. The pre-migration uniform 40 %-alpha
/// selection colour scored 1.03–1.43:1 against its own text on dark chrome, which
/// is how a selected row became invisible.
pub(crate) const REQUIRED_FILL_PAIRS: &[(&str, &str, &str, f64)] = &[
    (
        "text.primary",
        "surface.hover",
        "surface.main",
        UI_CONTRAST_MIN,
    ),
    (
        "text.primary",
        "surface.active",
        "surface.main",
        UI_CONTRAST_MIN,
    ),
    (
        "text.primary",
        "surface.selected",
        "surface.main",
        UI_CONTRAST_MIN,
    ),
];

/// Validate that every required contrast pair in `theme` meets its threshold.
/// Returns the first failing pair. Reuses
/// [`crate::editor::theme::composited_contrast_ratio`] as the WCAG engine (alpha
/// blended over the backdrop first); this helper only adds the required-pairs
/// policy. A pair whose foreground or background color role does not resolve
/// (returns `None`) is skipped — the core catalog guarantees all
/// `REQUIRED_CONTRAST_PAIRS` tokens are color roles, so a `None` indicates a
/// non-color override of a color token, which is a separate type-mismatch error
/// surfaced elsewhere, not a bypass: an unresolved role falls back to the core
/// catalog value rather than to no value at all.
pub(crate) fn theme_meets_contrast(theme: &ResolvedUiTheme) -> Result<(), ContrastFailure> {
    for &(foreground, background, threshold) in REQUIRED_CONTRAST_PAIRS {
        let (Some(fg), Some(bg)) = (theme.color(foreground), theme.color(background)) else {
            continue;
        };
        let ratio = crate::editor::theme::composited_contrast_ratio(fg, bg);
        if ratio < threshold {
            return Err(ContrastFailure {
                foreground,
                background,
                ratio,
                threshold,
            });
        }
    }
    for &(text, fill_role, backdrop, threshold) in REQUIRED_FILL_PAIRS {
        let (Some(fg), Some(fill), Some(backdrop)) = (
            theme.color(text),
            theme.color(fill_role),
            theme.color(backdrop),
        ) else {
            continue;
        };
        let painted = crate::editor::theme::composite_over(fill, backdrop);
        let ratio = crate::editor::theme::composited_contrast_ratio(fg, painted);
        if ratio < threshold {
            return Err(ContrastFailure {
                foreground: text,
                background: fill_role,
                ratio,
                threshold,
            });
        }
    }
    Ok(())
}

/// Validate an [`crate::protocol::ActiveTheme`] snapshot's contrast by
/// resolving both typed `design_tokens` and legacy `textStyles` colors through
/// the same cached UI-theme projection used by the client. Shared by the
/// `setTheme` apply path and the canonical-default resolver so both enforce the
/// same AA floor. The snapshot's contributions are package-parse-validated, so
/// resolution only fails if the wire snapshot was malformed (defensive: mapped
/// to a contrast failure so the caller rejects without crashing).
pub fn validate_active_theme_contrast(
    snapshot: &crate::protocol::ActiveTheme,
) -> Result<(), ContrastFailure> {
    let base = crate::editor::theme::StyleRegistry::from_active_theme(snapshot).base;
    let resolved = ResolvedUiTheme::from_active_theme(&snapshot.design_tokens)
        .map_err(|_| ContrastFailure {
            foreground: "<malformed override>",
            background: "<malformed override>",
            ratio: 0.0,
            threshold: TEXT_CONTRAST_MIN,
        })?
        .with_base_ui(&base);
    theme_meets_contrast(&resolved)
}
