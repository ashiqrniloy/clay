//! Single source of color and text style for the editor and shell paint paths.
//!
//! Plan 046 (Phase 18.15) task 4: the `StyleRegistry` is the only place in the
//! editor/shell paint path that owns color literals. `src/editor/surface.rs`,
//! `src/editor.rs`, and `src/masonry_editor.rs` consult the registry instead of
//! holding their own `Color` constants; a source-guard test forbids
//! `Color::from_rgb8`/`Color::from_rgba8` literals anywhere in that path except
//! this module (the theme-definition module).
//!
//! The registry maps the two-axis vocabulary from Plan 046 task 3
//! (`DecorationKind` + [`crate::protocol::TokenType`] + [`crate::protocol::Modifiers`])
//! to a resolved [`StyleSpec`] (color + text attributes). The default Clay theme
//! reproduces the exact baseline colors locked in task 1 so existing packages
//! render unchanged. Task 5 will layer active-theme overrides over these
//! defaults; the closed-enum default fallback stays here.

use masonry::peniko::Color;

use crate::protocol::{DecorationKind, DiagnosticSeverity, Modifiers, TokenType};

/// Resolved visual style for one decoration span: a background tint `color` plus
/// the text attributes the span's modifiers request (or the theme declares by
/// default). The current paint path consumes `color`; the text attributes are
/// carried for the task-5+ text-formatting work and theme overrides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TextAttributes {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleSpec {
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

impl StyleSpec {
    pub(crate) const fn attributes(self) -> TextAttributes {
        TextAttributes {
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strike: self.strike,
        }
    }
}

/// Base UI (non-decoration) colors consulted by the editor and shell chrome.
/// Names mirror the locked baselines from Plan 046 task 1 so a theme override
/// only has to swap the fields it changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseUiColors {
    /// Root shell background (old `BACKGROUND_COLOR`).
    pub shell_bg: Color,
    /// Editor panel background (old `PANEL_COLOR`).
    pub panel_bg: Color,
    pub text: Color,
    pub placeholder: Color,
    pub selection: Color,
    pub caret: Color,
    pub scrollbar: Color,
    pub scrollbar_track: Color,
    pub status_bg: Color,
    pub status_text: Color,
}

// Bit positions in `StyleRegistry::attr_defaults` for theme-declared
// text-attribute defaults per `TokenType`.
const ATTR_BOLD: u16 = 1 << 0;
const ATTR_ITALIC: u16 = 1 << 1;
const ATTR_UNDERLINE: u16 = 1 << 2;
const ATTR_STRIKE: u16 = 1 << 3;

/// Apply a `Some(bool)` text-attribute override to a per-token attribute bitset:
/// `Some(true)` sets the bit, `Some(false)` clears it, `None` leaves it.
fn set_attr_bit(bits: &mut u16, flag: u16, opt: Option<bool>) {
    match opt {
        Some(true) => *bits |= flag,
        Some(false) => *bits &= !flag,
        None => {}
    }
}

/// Single source of color for the editor/shell paint path. Resolved at
/// load/reload (task 5); immutable and cheap to read during paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleRegistry {
    pub base: BaseUiColors,
    // Decoration-layer fallback colors (kind-first for Diagnostic/SearchMatch;
    // Syntax and Semantic both fall through to the per-`TokenType` table).
    // `semantic` remains the Clay default prose family color shared by headings
    // and other text tokens in the syntax table.
    semantic: Color,
    /// Legacy `DecorationKind::Diagnostic` fill tint (error-severity default).
    diagnostic: Color,
    /// Severity-aware squiggle/underline colors for `DiagnosticSpan` paint.
    diagnostic_error: Color,
    diagnostic_warning: Color,
    diagnostic_info: Color,
    search_match: Color,
    // Per-`TokenType` colors for the `Syntax` layer, indexed by
    // [`TokenType::index`]. The Clay default still reproduces the old family
    // mapping; active themes can override every token independently.
    syntax: [Color; 35],
    // Per-`TokenType` text-attribute defaults declared by the active theme
    // (e.g. make `Keyword` bold, `Quote` italic). Indexed by
    // [`TokenType::index`]; bits per the `ATTR_*` constants above. A span's own
    // `Modifiers` upgrade over these defaults (OR).
    attr_defaults: [u16; 35],
}

impl Default for StyleRegistry {
    fn default() -> Self {
        Self::clay_default()
    }
}

impl StyleRegistry {
    /// Default Clay theme: the exact baseline colors locked in Plan 046 task 1.
    /// These literals are the ONLY color literals permitted in the editor/shell
    /// paint path (this module is the theme-definition module the source-guard
    /// test exempts).
    pub const fn clay_default() -> Self {
        StyleRegistry {
            base: BaseUiColors {
                shell_bg: Color::from_rgb8(0x18, 0x18, 0x18),
                panel_bg: Color::from_rgb8(0x24, 0x24, 0x24),
                text: Color::from_rgb8(0xf4, 0xf1, 0xff),
                placeholder: Color::from_rgb8(0x8d, 0x86, 0xa3),
                selection: Color::from_rgba8(0x8a, 0x6f, 0xff, 0x66),
                caret: Color::from_rgb8(0xff, 0xff, 0xff),
                scrollbar: Color::from_rgba8(0xb9, 0xb2, 0xd6, 0x99),
                scrollbar_track: Color::from_rgba8(0xff, 0xff, 0xff, 0x14),
                status_bg: Color::from_rgb8(0x18, 0x18, 0x1f),
                status_text: Color::from_rgb8(0xd7, 0xd2, 0xe8),
            },
            semantic: Color::from_rgba8(0x4d, 0xc8, 0x8a, 0x2f),
            diagnostic: Color::from_rgba8(0xff, 0x4d, 0x6d, 0x3f),
            diagnostic_error: Color::from_rgb8(0xff, 0x4d, 0x6d),
            diagnostic_warning: Color::from_rgb8(0xff, 0xd1, 0x66),
            diagnostic_info: Color::from_rgb8(0x61, 0xaf, 0xef),
            search_match: Color::from_rgba8(0xff, 0xd1, 0x66, 0x45),
            syntax: [
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Namespace
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Type
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Class
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Enum
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Interface
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Struct
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // TypeParameter
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Parameter
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Variable
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Property
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // EnumMember
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Event
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Function
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Method
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Macro
                Color::from_rgba8(0xc7, 0x92, 0xea, 0x55), // Keyword
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Modifier
                Color::from_rgba8(0x7f, 0x84, 0x8e, 0x55), // Comment
                Color::from_rgba8(0xc3, 0xe8, 0x8d, 0x55), // String
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Number
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Regexp
                Color::from_rgba8(0xab, 0xb2, 0xbf, 0x55), // Operator
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Decorator
                // Prose palette (Plan 059 task 3): differentiated instead of
                // the old uniform muted green. Headings step through hues and
                // are bold by default; links are underlined blue; quotes are
                // italic gray; code keeps the string green in monospace (the
                // font role comes from the span, not the theme).
                Color::from_rgba8(0xff, 0x4d, 0x6d, 0x55), // Heading1
                Color::from_rgba8(0xff, 0xd1, 0x66, 0x55), // Heading2
                Color::from_rgba8(0xc3, 0xe8, 0x8d, 0x55), // Heading3
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Heading4
                Color::from_rgba8(0xc7, 0x92, 0xea, 0x55), // Heading5
                Color::from_rgba8(0x4d, 0xc8, 0x8a, 0x55), // Heading6
                Color::from_rgba8(0xab, 0xb2, 0xbf, 0x55), // ListItem
                Color::from_rgba8(0x7f, 0x84, 0x8e, 0x55), // Quote
                Color::from_rgba8(0xc3, 0xe8, 0x8d, 0x55), // CodeBlock
                Color::from_rgba8(0xff, 0xd1, 0x66, 0x55), // CodeSpan
                Color::from_rgba8(0x61, 0xaf, 0xef, 0x55), // Link
                Color::from_rgb8(0xf4, 0xf1, 0xff),        // Paragraph
            ],
            attr_defaults: [
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // Namespace..Macro
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // Keyword..Decorator
                ATTR_BOLD,
                ATTR_BOLD,
                ATTR_BOLD,
                ATTR_BOLD,
                ATTR_BOLD,
                ATTR_BOLD,      // Heading1..6
                0,              // ListItem
                ATTR_ITALIC,    // Quote
                0,              // CodeBlock
                0,              // CodeSpan
                ATTR_UNDERLINE, // Link
                0,              // Paragraph
            ],
        }
    }

    /// Theme-owned color/style for one diagnostic severity. Paint maps
    /// `DiagnosticSpan.severity` through this helper only — no language/source
    /// branch and no hardcoded paint-path colors.
    pub fn diagnostic_style(&self, severity: DiagnosticSeverity) -> StyleSpec {
        let color = match severity {
            DiagnosticSeverity::Error => self.diagnostic_error,
            DiagnosticSeverity::Warning => self.diagnostic_warning,
            DiagnosticSeverity::Info => self.diagnostic_info,
        };
        StyleSpec {
            color,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
        }
    }

    /// Resolved style for one span. `Diagnostic`/`SearchMatch` color by layer;
    /// `Syntax` and `Semantic` both select the closed `TokenType` family color
    /// so LSP semantic tokens refine vocabulary without a second theme table.
    /// Text attributes come from the span's `modifiers` (OR'd with theme
    /// per-token defaults).
    pub fn style_for(
        &self,
        kind: DecorationKind,
        token_type: TokenType,
        modifiers: Modifiers,
    ) -> StyleSpec {
        let color = match kind {
            DecorationKind::Diagnostic => self.diagnostic,
            DecorationKind::SearchMatch => self.search_match,
            DecorationKind::Syntax | DecorationKind::Semantic => self.syntax_color(token_type),
        };
        // Theme-declared per-token text-attribute defaults upgrade the span
        // modifiers (OR): a theme that makes `Keyword` bold renders keywords
        // bold even when the span carries no BOLD modifier; an explicit span
        // modifier always wins.
        let defaults = self.attr_defaults[token_type.index()];
        StyleSpec {
            color,
            bold: (defaults & ATTR_BOLD) != 0 || modifiers.contains(Modifiers::BOLD),
            italic: (defaults & ATTR_ITALIC) != 0 || modifiers.contains(Modifiers::ITALIC),
            underline: (defaults & ATTR_UNDERLINE) != 0 || modifiers.contains(Modifiers::UNDERLINE),
            strike: (defaults & ATTR_STRIKE) != 0 || modifiers.contains(Modifiers::STRIKETHROUGH),
        }
    }

    /// Vocabulary color for one closed `TokenType`. Shared by `Syntax` and
    /// `Semantic` layers. The Clay default table reproduces the prior
    /// prefix-based family mapping; active themes can override each token
    /// independently.
    fn syntax_color(&self, token_type: TokenType) -> Color {
        self.syntax[token_type.index()]
    }
}

/// Which base-UI color a theme override targets. Mirrors [`BaseUiColors`]'
/// fields so a theme package can restyle chrome (panel background, caret,
/// scrollbar, …) as well as syntax tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseUiColorKey {
    ShellBg,
    PanelBg,
    Text,
    Placeholder,
    Selection,
    Caret,
    Scrollbar,
    ScrollbarTrack,
    StatusBg,
    StatusText,
    DiagnosticError,
    DiagnosticWarning,
    DiagnosticInfo,
}

/// Where a [`TextStyleOverride`] applies: either a base-UI chrome color or a
/// `Syntax`-layer token family. The vocabulary is closed so validation can
/// reject unknown override targets up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideTarget {
    BaseUi(BaseUiColorKey),
    Syntax(TokenType),
}

/// Parse an override `token` name into its [`OverrideTarget`]. Accepts base-UI
/// keys (`panelBg`, `caret`, …) and `TokenType` variant names (`Keyword`,
/// `String`, `Heading1`, …). Returns `None` for unknown names so the caller can
/// reject them deterministically.
pub fn parse_override_token(token: &str) -> Option<OverrideTarget> {
    let base = match token {
        "shellBg" => BaseUiColorKey::ShellBg,
        "panelBg" => BaseUiColorKey::PanelBg,
        "text" => BaseUiColorKey::Text,
        "placeholder" => BaseUiColorKey::Placeholder,
        "selection" => BaseUiColorKey::Selection,
        "caret" => BaseUiColorKey::Caret,
        "scrollbar" => BaseUiColorKey::Scrollbar,
        "scrollbarTrack" => BaseUiColorKey::ScrollbarTrack,
        "statusBg" => BaseUiColorKey::StatusBg,
        "statusText" => BaseUiColorKey::StatusText,
        "diagnosticError" => BaseUiColorKey::DiagnosticError,
        "diagnosticWarning" => BaseUiColorKey::DiagnosticWarning,
        "diagnosticInfo" => BaseUiColorKey::DiagnosticInfo,
        _ => return TokenType::from_name(token).map(OverrideTarget::Syntax),
    };
    Some(OverrideTarget::BaseUi(base))
}

/// Inert text-style override declared by a theme package. `token` names the
/// [`OverrideTarget`]; each field is `Some` only when the theme overrides it
/// (leared over the Clay default, last-wins). `provenance` records the owning
/// theme package id for diagnostics. This is pure style data — no code, ops,
/// widgets, or raw CSS reach the registry from a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyleOverride {
    pub token: String,
    pub color: Option<Color>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub provenance: String,
}

/// Parse a CSS-style hex color (`#rgb`, `#rrggbb`, or `#rrggbbaa`) into
/// `[r, g, b, a]` bytes. Returns `None` for malformed input so callers reject
/// it deterministically. Case-insensitive. The core parser shared by
/// [`parse_hex_color`].
pub fn parse_hex_rgba(hex: &str) -> Option<[u8; 4]> {
    let h = hex.strip_prefix('#')?;
    let parse = |s: &str| u8::from_str_radix(s, 16).ok();
    match h.len() {
        3 => {
            let r = parse(&h[0..1])?;
            let g = parse(&h[1..2])?;
            let b = parse(&h[2..3])?;
            Some([r * 17, g * 17, b * 17, 255])
        }
        6 => Some([parse(&h[0..2])?, parse(&h[2..4])?, parse(&h[4..6])?, 255]),
        8 => Some([
            parse(&h[0..2])?,
            parse(&h[2..4])?,
            parse(&h[4..6])?,
            parse(&h[6..8])?,
        ]),
        _ => None,
    }
}

/// Parse a CSS-style hex color (`#rgb`, `#rrggbb`, or `#rrggbbaa`) into a peniko
/// [`Color`]. Returns `None` for malformed input so callers reject it
/// deterministically. Case-insensitive.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let [r, g, b, a] = parse_hex_rgba(hex)?;
    Some(Color::from_rgba8(r, g, b, a))
}

/// Convert a server-shipped inert [`crate::protocol::TextThemeOverride`] wire
/// record into the editor-side [`TextStyleOverride`] consumed by
/// [`StyleRegistry::with_text_overrides`]. RGBA bytes rebuild the peniko
/// [`Color`]; the rest are carried verbatim. This keeps the `Color` type out of
/// the protocol.
impl From<crate::protocol::TextThemeOverride> for TextStyleOverride {
    fn from(wire: crate::protocol::TextThemeOverride) -> Self {
        Self {
            token: wire.token,
            color: wire.color.map(|[r, g, b, a]| Color::from_rgba8(r, g, b, a)),
            bold: wire.bold,
            italic: wire.italic,
            underline: wire.underline,
            strike: wire.strike,
            provenance: wire.provenance,
        }
    }
}

impl StyleRegistry {
    /// A fresh copy of the active Clay default theme, ready to be mutated by
    /// [`Self::with_text_overrides`].
    pub fn clone_default() -> Self {
        Self::clay_default()
    }

    /// Build the active registry by layering theme-package text-style overrides
    /// over the Clay default. Overrides apply in declaration order (last-wins
    /// for duplicate targets); unknown tokens are skipped here (validation
    /// rejected them at contribution-parse time, so a runtime caller must have
    /// validated first). Resolved once at load/reload (task 7 `setTheme`),
    /// never per paint.
    pub fn with_text_overrides(overrides: &[TextStyleOverride]) -> Self {
        let mut registry = Self::clay_default();
        for o in overrides {
            let Some(target) = parse_override_token(&o.token) else {
                // Invalid tokens are rejected at parse time; this is a no-op.
                continue;
            };
            match target {
                OverrideTarget::BaseUi(base) => {
                    if let Some(color) = o.color {
                        match base {
                            BaseUiColorKey::ShellBg => registry.base.shell_bg = color,
                            BaseUiColorKey::PanelBg => registry.base.panel_bg = color,
                            BaseUiColorKey::Text => registry.base.text = color,
                            BaseUiColorKey::Placeholder => registry.base.placeholder = color,
                            BaseUiColorKey::Selection => registry.base.selection = color,
                            BaseUiColorKey::Caret => registry.base.caret = color,
                            BaseUiColorKey::Scrollbar => registry.base.scrollbar = color,
                            BaseUiColorKey::ScrollbarTrack => registry.base.scrollbar_track = color,
                            BaseUiColorKey::StatusBg => registry.base.status_bg = color,
                            BaseUiColorKey::StatusText => registry.base.status_text = color,
                            BaseUiColorKey::DiagnosticError => registry.diagnostic_error = color,
                            BaseUiColorKey::DiagnosticWarning => {
                                registry.diagnostic_warning = color
                            }
                            BaseUiColorKey::DiagnosticInfo => registry.diagnostic_info = color,
                        }
                    }
                }
                OverrideTarget::Syntax(tt) => {
                    if let Some(color) = o.color {
                        registry.set_syntax_color(tt, color);
                    }
                    // Theme-declared text-attribute defaults upgrade the span
                    // modifiers (OR). `Some(false)` clears a default a prior
                    // override set; `None` leaves it. Base-UI targets carry no
                    // text attributes.
                    let idx = tt.index();
                    let bits = &mut registry.attr_defaults[idx];
                    set_attr_bit(bits, ATTR_BOLD, o.bold);
                    set_attr_bit(bits, ATTR_ITALIC, o.italic);
                    set_attr_bit(bits, ATTR_UNDERLINE, o.underline);
                    set_attr_bit(bits, ATTR_STRIKE, o.strike);
                }
            }
        }
        registry
    }

    /// Reconstruct the active theme from a server-shipped
    /// [`crate::protocol::ActiveTheme`] snapshot (Plan 046 task 7 `setTheme`).
    /// Resolves the Clay default + inert wire overrides into a registry the
    /// client installs before startup paint.
    pub fn from_active_theme(theme: &crate::protocol::ActiveTheme) -> Self {
        let overrides: Vec<TextStyleOverride> =
            theme.overrides.iter().map(|o| o.clone().into()).collect();
        Self::with_text_overrides(&overrides)
    }

    fn set_syntax_color(&mut self, token_type: TokenType, color: Color) {
        self.syntax[token_type.index()] = color;
    }
}

/// Relative luminance (WCAG 2.x) for an sRGB [`Color`], ignoring alpha.
/// Used only for theme-authoring / polish checks — not on the paint hot path.
pub fn relative_luminance(color: Color) -> f64 {
    fn channel(component: f64) -> f64 {
        let c = component / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let rgba = color.to_rgba8();
    0.2126 * channel(f64::from(rgba.r))
        + 0.7152 * channel(f64::from(rgba.g))
        + 0.0722 * channel(f64::from(rgba.b))
}

/// WCAG contrast ratio between two opaque colors. Larger is better; 4.5 is AA
/// for normal text and is the floor Clay uses for status chrome polish.
pub fn contrast_ratio(foreground: Color, background: Color) -> f64 {
    let l1 = relative_luminance(foreground);
    let l2 = relative_luminance(background);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Status-chrome contrast for a resolved registry (`statusText` on `statusBg`).
pub fn status_chrome_contrast_ratio(registry: &StyleRegistry) -> f64 {
    contrast_ratio(registry.base.status_text, registry.base.status_bg)
}

/// Minimum AA contrast Clay expects for themed status chrome.
pub const STATUS_CHROME_MIN_CONTRAST: f64 = 4.5;

/// True when status chrome meets Clay's AA contrast floor.
pub fn status_chrome_meets_contrast(registry: &StyleRegistry) -> bool {
    status_chrome_contrast_ratio(registry) >= STATUS_CHROME_MIN_CONTRAST
}

/// Compact, path-free theme label for status/accessibility observability.
/// `"@clay/theme-gruvbox-material-dark"` → `"theme-gruvbox-material-dark"`.
pub fn theme_display_label(specifier: &str) -> String {
    specifier
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(specifier)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DecorationKind;

    #[test]
    fn default_registry_reproduces_locked_baseline_colors() {
        // Task-1 baselines must round-trip through the single source of color.
        let r = StyleRegistry::default();
        assert_eq!(r.base.panel_bg, Color::from_rgb8(0x24, 0x24, 0x24));
        assert_eq!(r.base.text, Color::from_rgb8(0xf4, 0xf1, 0xff));
        assert_eq!(r.base.selection, Color::from_rgba8(0x8a, 0x6f, 0xff, 0x66));
        assert_eq!(r.semantic, Color::from_rgba8(0x4d, 0xc8, 0x8a, 0x2f));
        assert_eq!(r.diagnostic, Color::from_rgba8(0xff, 0x4d, 0x6d, 0x3f));
        assert_eq!(r.diagnostic_error, Color::from_rgb8(0xff, 0x4d, 0x6d));
        assert_eq!(r.diagnostic_warning, Color::from_rgb8(0xff, 0xd1, 0x66));
        assert_eq!(r.diagnostic_info, Color::from_rgb8(0x61, 0xaf, 0xef));
        assert_eq!(
            r.syntax_color(TokenType::Keyword),
            Color::from_rgba8(0xc7, 0x92, 0xea, 0x55)
        );
    }

    #[test]
    fn diagnostic_severities_resolve_distinct_theme_owned_styles() {
        let r = StyleRegistry::default();
        let error = r.diagnostic_style(DiagnosticSeverity::Error).color;
        let warning = r.diagnostic_style(DiagnosticSeverity::Warning).color;
        let info = r.diagnostic_style(DiagnosticSeverity::Info).color;
        assert_ne!(error, warning);
        assert_ne!(warning, info);
        assert_ne!(error, info);
        assert_eq!(error, r.diagnostic_error);
        assert_eq!(warning, r.diagnostic_warning);
        assert_eq!(info, r.diagnostic_info);

        let overrides = [TextStyleOverride {
            token: "diagnosticError".to_string(),
            color: Some(Color::from_rgb8(0xaa, 0x00, 0x00)),
            bold: None,
            italic: None,
            underline: None,
            strike: None,
            provenance: "test".to_string(),
        }];
        let themed = StyleRegistry::with_text_overrides(&overrides);
        assert_eq!(
            themed.diagnostic_style(DiagnosticSeverity::Error).color,
            Color::from_rgb8(0xaa, 0x00, 0x00)
        );
        assert_eq!(
            themed.diagnostic_style(DiagnosticSeverity::Warning).color,
            r.diagnostic_warning
        );
    }

    #[test]
    fn style_for_drives_color_from_kind_and_token_type() {
        let r = StyleRegistry::default();
        assert_eq!(
            r.style_for(DecorationKind::Syntax, TokenType::Keyword, Modifiers::NONE)
                .color,
            r.syntax_color(TokenType::Keyword)
        );
        assert_eq!(
            r.style_for(DecorationKind::Syntax, TokenType::Heading1, Modifiers::NONE)
                .color,
            r.syntax_color(TokenType::Heading1)
        );
        // Default prose palette (Plan 059 task 3): prose tokens are visually
        // differentiated without any theme package.
        let heading1 = r.style_for(DecorationKind::Syntax, TokenType::Heading1, Modifiers::NONE);
        let heading2 = r.style_for(DecorationKind::Syntax, TokenType::Heading2, Modifiers::NONE);
        assert_ne!(heading1.color, heading2.color);
        assert!(heading1.bold && heading2.bold);
        let paragraph = r.style_for(
            DecorationKind::Syntax,
            TokenType::Paragraph,
            Modifiers::NONE,
        );
        assert_eq!(paragraph.color, r.base.text);
        assert!(!paragraph.bold && !paragraph.underline);
        let link = r.style_for(DecorationKind::Syntax, TokenType::Link, Modifiers::NONE);
        assert!(link.underline);
        assert_ne!(link.color, paragraph.color);
        let quote = r.style_for(DecorationKind::Syntax, TokenType::Quote, Modifiers::NONE);
        assert!(quote.italic);
        let code_span = r.style_for(DecorationKind::Syntax, TokenType::CodeSpan, Modifiers::NONE);
        assert_ne!(code_span.color, paragraph.color);
        assert_eq!(
            r.style_for(
                DecorationKind::Semantic,
                TokenType::Function,
                Modifiers::NONE
            )
            .color,
            r.syntax_color(TokenType::Function)
        );
        assert_eq!(
            r.style_for(
                DecorationKind::Diagnostic,
                TokenType::Variable,
                Modifiers::NONE
            )
            .color,
            r.diagnostic
        );
    }

    #[test]
    fn style_for_reflects_text_attribute_modifiers() {
        let r = StyleRegistry::default();
        let spec = r.style_for(
            DecorationKind::Syntax,
            TokenType::Paragraph,
            Modifiers::BOLD | Modifiers::ITALIC,
        );
        assert!(spec.bold);
        assert!(spec.italic);
        assert!(!spec.underline);
        assert!(!spec.strike);
        assert_eq!(
            spec.color, r.base.text,
            "paragraph and inline emphasis preserve the normal document text color"
        );
    }

    #[test]
    fn parse_hex_color_accepts_rgb_rrggbb_and_rrggbbaa() {
        assert_eq!(
            parse_hex_color("#fff"),
            Some(Color::from_rgb8(0xff, 0xff, 0xff))
        );
        assert_eq!(
            parse_hex_color("#c792ea"),
            Some(Color::from_rgba8(0xc7, 0x92, 0xea, 0xff))
        );
        assert_eq!(
            parse_hex_color("#8a6fff66"),
            Some(Color::from_rgba8(0x8a, 0x6f, 0xff, 0x66))
        );
        assert_eq!(parse_hex_color("not-a-color"), None);
        assert_eq!(parse_hex_color("#12"), None);
    }

    #[test]
    fn parse_override_token_routes_base_ui_and_token_type_names() {
        assert!(matches!(
            parse_override_token("panelBg"),
            Some(OverrideTarget::BaseUi(_))
        ));
        assert!(matches!(
            parse_override_token("caret"),
            Some(OverrideTarget::BaseUi(_))
        ));
        assert!(matches!(
            parse_override_token("Keyword"),
            Some(OverrideTarget::Syntax(TokenType::Keyword))
        ));
        assert!(matches!(
            parse_override_token("Heading1"),
            Some(OverrideTarget::Syntax(TokenType::Heading1))
        ));
        assert!(matches!(
            parse_override_token("diagnosticError"),
            Some(OverrideTarget::BaseUi(BaseUiColorKey::DiagnosticError))
        ));
        assert_eq!(parse_override_token("keyword.control"), None);
        assert_eq!(parse_override_token("notAToken"), None);
    }

    #[test]
    fn with_text_overrides_layers_theme_over_clay_default_last_wins() {
        let overrides = vec![
            TextStyleOverride {
                token: "Keyword".to_string(),
                color: Some(Color::from_rgba8(0x00, 0x00, 0x00, 0xff)),
                bold: Some(true),
                italic: None,
                underline: None,
                strike: None,
                provenance: "@clay/theme-x".to_string(),
            },
            TextStyleOverride {
                token: "panelBg".to_string(),
                color: Some(Color::from_rgb8(0x10, 0x10, 0x10)),
                bold: None,
                italic: None,
                underline: None,
                strike: None,
                provenance: "@clay/theme-x".to_string(),
            },
            // Last-wins for a duplicate target.
            TextStyleOverride {
                token: "Keyword".to_string(),
                color: Some(Color::from_rgb8(0xaa, 0xbb, 0xcc)),
                bold: None,
                italic: None,
                underline: None,
                strike: None,
                provenance: "@clay/theme-x".to_string(),
            },
        ];
        let r = StyleRegistry::with_text_overrides(&overrides);
        assert_eq!(
            r.syntax_color(TokenType::Keyword),
            Color::from_rgb8(0xaa, 0xbb, 0xcc)
        );
        assert_eq!(r.base.panel_bg, Color::from_rgb8(0x10, 0x10, 0x10));
        // Untouched base stays the Clay default.
        assert_eq!(r.base.text, StyleRegistry::default().base.text);
    }

    #[test]
    fn with_text_overrides_skips_unknown_tokens_silently() {
        // Validation rejects unknown tokens at parse time; the merge must not
        // panic if a runtime caller bypasses validation.
        let overrides = vec![TextStyleOverride {
            token: "bogus".to_string(),
            color: Some(Color::from_rgb8(0x00, 0x00, 0x00)),
            bold: None,
            italic: None,
            underline: None,
            strike: None,
            provenance: "@clay/theme-x".to_string(),
        }];
        let r = StyleRegistry::with_text_overrides(&overrides);
        assert_eq!(r, StyleRegistry::default());
    }

    #[test]
    fn theme_text_attribute_defaults_upgrade_span_modifiers() {
        // A theme that makes `Keyword` bold and `Quote` italic must render those
        // family defaults even when a span carries no matching modifier; an
        // explicit span modifier always wins (already-true stays true).
        let overrides = vec![
            TextStyleOverride {
                token: "Keyword".to_string(),
                color: None,
                bold: Some(true),
                italic: None,
                underline: None,
                strike: None,
                provenance: "@clay/theme-x".to_string(),
            },
            TextStyleOverride {
                token: "Quote".to_string(),
                color: None,
                bold: None,
                italic: Some(true),
                underline: None,
                strike: None,
                provenance: "@clay/theme-x".to_string(),
            },
        ];
        let r = StyleRegistry::with_text_overrides(&overrides);
        // No BOLD modifier on the span, but the theme default wins.
        assert!(
            r.style_for(DecorationKind::Syntax, TokenType::Keyword, Modifiers::NONE)
                .bold
        );
        assert!(
            r.style_for(DecorationKind::Syntax, TokenType::Quote, Modifiers::NONE)
                .italic
        );
        // A span that carries no modifier on an unstyled token stays unstyled.
        assert!(
            !r.style_for(DecorationKind::Syntax, TokenType::String, Modifiers::NONE)
                .bold
        );
        // Explicit span modifier upgrades an already-defaulted token (OR).
        assert!(
            r.style_for(
                DecorationKind::Syntax,
                TokenType::Keyword,
                Modifiers::ITALIC
            )
            .italic
        );
    }

    #[test]
    fn clay_default_status_chrome_meets_aa_contrast() {
        let ratio = status_chrome_contrast_ratio(&StyleRegistry::clay_default());
        assert!(
            ratio >= STATUS_CHROME_MIN_CONTRAST,
            "Clay default status chrome contrast {ratio:.2} must be >= {STATUS_CHROME_MIN_CONTRAST}"
        );
    }

    #[test]
    fn theme_display_label_strips_package_prefix() {
        assert_eq!(
            theme_display_label("@clay/theme-gruvbox-material-dark"),
            "theme-gruvbox-material-dark"
        );
        assert_eq!(theme_display_label("@clay/default"), "default");
        assert_eq!(theme_display_label("local"), "local");
    }
}
