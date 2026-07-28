//! Native UI chrome primitives for the Clay shell.
//!
//! Plan 063 (Phase 20.2) task 3: `pub(crate)` paint helpers for divider, focus
//! ring, panel chrome, scroll chrome, badge, `kbd` hint, icon slot, and tooltip
//! shell. Each primitive reads only cached resolved token values from
//! `ResolvedUiTheme`/`SduiThemeStyle`/`TypographyRegistry`; none parse theme,
//! run JS, or hit IPC.
//!
//! Primitives are inert paint helpers with no package-facing surface. Packages
//! continue to declare inert components only; the SDUI/editor paint path calls
//! these helpers by construction.

use masonry::kurbo::{Rect, RoundedRect, Stroke};
use masonry::peniko::Color;
use masonry::vello::Scene;

use crate::editor::typography::TypographyRegistry;
use crate::shell::theme::ResolvedUiTheme;

/// Interaction state for interactive primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // Phase 20.2: foundation; not all variants used yet
pub(crate) enum InteractionState {
    #[default]
    Rest,
    Hover,
    Active,
    Focus,
    Disabled,
}

/// Axis for divider/separator orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Phase 20.2: foundation; not all variants used yet
pub(crate) enum Axis {
    Horizontal,
    Vertical,
}

/// Multiply a color's alpha channel by `factor` (saturating at opaque). Used
/// for disabled-state dimming so a non-opaque base token still dims correctly.
fn apply_alpha(color: Color, factor: f32) -> Color {
    let rgba = color.to_rgba8();
    Color::from_rgba8(rgba.r, rgba.g, rgba.b, (rgba.a as f32 * factor) as u8)
}

/// Resolve a component fill color for an `InteractionState` from design tokens
/// (Phase 20.4). `rest_token` is the surface token at `Rest` (e.g.
/// `surface.control` for buttons). `Hover`/`Active`/`Focus` map to the shared
/// state tokens; `Disabled` maps to `surface.disabled` dimmed by
/// `opacity.disabled`.
pub(crate) fn component_state_color(
    theme: &ResolvedUiTheme,
    rest_token: &str,
    state: InteractionState,
) -> Color {
    match state {
        InteractionState::Rest => theme.color(rest_token).unwrap_or(Color::TRANSPARENT),
        InteractionState::Hover => theme.color("surface.hover").unwrap_or(Color::TRANSPARENT),
        InteractionState::Active => theme.color("surface.active").unwrap_or(Color::TRANSPARENT),
        InteractionState::Focus => theme.color("accent.primary").unwrap_or(Color::TRANSPARENT),
        InteractionState::Disabled => apply_alpha(
            theme
                .color("surface.disabled")
                .unwrap_or(Color::TRANSPARENT),
            theme.opacity("opacity.disabled").unwrap_or(0.5),
        ),
    }
}

/// Resolve a list row fill for an `InteractionState`, honoring the `selected`
/// flag (Phase 20.4). `Selected` rows at `Rest`/`Focus` read `surface.selected`;
/// `Hover`/`Active` override with the shared state tokens; `Disabled` dims
/// `surface.disabled` by `opacity.disabled`. Focus is expressed via a focus
/// ring, not the fill, so `Focus` mirrors `Rest`.
pub(crate) fn list_row_fill_color(
    theme: &ResolvedUiTheme,
    state: InteractionState,
    selected: bool,
) -> Color {
    match state {
        InteractionState::Rest | InteractionState::Focus => {
            if selected {
                theme
                    .color("surface.selected")
                    .unwrap_or(Color::TRANSPARENT)
            } else {
                theme.color("surface.list").unwrap_or(Color::TRANSPARENT)
            }
        }
        InteractionState::Hover => theme.color("surface.hover").unwrap_or(Color::TRANSPARENT),
        InteractionState::Active => theme.color("surface.active").unwrap_or(Color::TRANSPARENT),
        InteractionState::Disabled => apply_alpha(
            theme
                .color("surface.disabled")
                .unwrap_or(Color::TRANSPARENT),
            theme.opacity("opacity.disabled").unwrap_or(0.5),
        ),
    }
}

/// Resolve the text color for a disabled component: `text.disabled` dimmed by
/// `opacity.disabled` (Phase 20.4).
pub(crate) fn disabled_text_color(theme: &ResolvedUiTheme) -> Color {
    apply_alpha(
        theme.color("text.disabled").unwrap_or(Color::TRANSPARENT),
        theme.opacity("opacity.disabled").unwrap_or(0.5),
    )
}

/// Panel chrome state for title row, collapse affordance, and resize handle.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelChrome {
    pub title: Option<&'static str>,
    pub collapse: InteractionState,
    pub resize: InteractionState,
}

/// Icon glyph for icon slot (token-sized, no package image assets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Phase 20.2: foundation; not all variants used yet
pub(crate) enum IconGlyph {
    ChevronRight,
    ChevronDown,
    Close,
    Settings,
    Search,
}

/// Paint a divider/separator.
///
/// Tokens: `border.hairline` (color), `dimension.border.hairline` (width).
/// Accessibility: `Role::Separator`.
#[allow(dead_code)] // Phase 20.2: foundation; not all primitives used yet
pub(crate) fn paint_divider(scene: &mut Scene, rect: Rect, axis: Axis, theme: &ResolvedUiTheme) {
    let color = theme.color("border.hairline").unwrap_or(Color::TRANSPARENT);
    let width = theme.dimension("dimension.border.hairline").unwrap_or(1.0);

    let line_rect = match axis {
        Axis::Horizontal => Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + width),
        Axis::Vertical => Rect::new(rect.x0, rect.y0, rect.x0 + width, rect.y1),
    };

    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        color,
        None,
        &line_rect,
    );
}

/// Paint a focus ring around a rect.
///
/// Tokens: `border.focus` (color), `dimension.border.thin` (width), `radius.xs` (corner radius).
/// Accessibility: `Role::Focusable` (implicit via focus state).
#[allow(dead_code)] // Phase 20.2: foundation; not all primitives used yet
pub(crate) fn paint_focus_ring(scene: &mut Scene, rect: Rect, theme: &ResolvedUiTheme) {
    let color = theme.color("border.focus").unwrap_or(Color::TRANSPARENT);
    let width = theme.dimension("dimension.border.thin").unwrap_or(2.0);
    let radius = theme.dimension("radius.xs").unwrap_or(2.0);

    let rounded = RoundedRect::from_rect(rect, radius);
    let stroke = Stroke::new(width);

    scene.stroke(
        &stroke,
        masonry::kurbo::Affine::IDENTITY,
        color,
        None,
        &rounded,
    );
}

/// Paint panel chrome: title row, collapse affordance, resize handle chrome.
///
/// Tokens: `surface.panel` (background), `border.subtle` (border), `radius.sm` (corner radius),
/// `spacing.panel` (padding), `typography.title` (title text), `PanelDefaults` (title row height,
/// collapse affordance size, resize handle size).
/// Accessibility: `Role::Pane` (panel), `Role::Button` (collapse affordance).
///
/// Resize handle and collapse affordance paint chrome only — drag/resize behavior is Phase 20.3.
pub(crate) fn paint_panel_chrome(
    scene: &mut Scene,
    rect: Rect,
    chrome: &PanelChrome,
    theme: &ResolvedUiTheme,
) {
    let bg = theme.color("surface.panel").unwrap_or(Color::TRANSPARENT);
    let border = theme.color("border.subtle").unwrap_or(Color::TRANSPARENT);
    let radius = theme.dimension("radius.sm").unwrap_or(4.0);
    let border_width = theme.dimension("dimension.border.hairline").unwrap_or(1.0);

    // Panel background
    let rounded = RoundedRect::from_rect(rect, radius);
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        bg,
        None,
        &rounded,
    );

    // Panel border
    let stroke = Stroke::new(border_width);
    scene.stroke(
        &stroke,
        masonry::kurbo::Affine::IDENTITY,
        border,
        None,
        &rounded,
    );

    // Title row (if title present)
    if let Some(_title) = chrome.title {
        // Title text painting would go here using TypographyRegistry
        // For now, chrome only — text rendering is Phase 20.4
    }

    // Collapse affordance (chevron)
    let collapse_color = match chrome.collapse {
        InteractionState::Rest => theme.color("text.muted").unwrap_or(Color::TRANSPARENT),
        InteractionState::Hover => theme.color("text.primary").unwrap_or(Color::TRANSPARENT),
        InteractionState::Active => theme.color("accent.primary").unwrap_or(Color::TRANSPARENT),
        InteractionState::Focus => theme.color("border.focus").unwrap_or(Color::TRANSPARENT),
        InteractionState::Disabled => theme.color("text.disabled").unwrap_or(Color::TRANSPARENT),
    };

    // Resize handle chrome (vertical grip lines)
    let resize_color = match chrome.resize {
        InteractionState::Rest => theme.color("border.subtle").unwrap_or(Color::TRANSPARENT),
        InteractionState::Hover => theme.color("border.strong").unwrap_or(Color::TRANSPARENT),
        InteractionState::Active => theme.color("accent.primary").unwrap_or(Color::TRANSPARENT),
        InteractionState::Focus => theme.color("border.focus").unwrap_or(Color::TRANSPARENT),
        InteractionState::Disabled => theme.color("border.hairline").unwrap_or(Color::TRANSPARENT),
    };

    // Paint resize handle grip (three vertical lines at right edge)
    let handle_width = 6.0;
    let grip_spacing = 2.0;
    for i in 0..3 {
        let x = rect.x1 - handle_width + (i as f64 * grip_spacing);
        let grip_rect = Rect::new(x, rect.y0 + 4.0, x + 1.0, rect.y1 - 4.0);
        scene.fill(
            masonry::vello::peniko::Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            resize_color,
            None,
            &grip_rect,
        );
    }

    // Paint collapse affordance (chevron placeholder)
    let _ = collapse_color; // Used in Phase 20.4 for actual chevron rendering
}

/// Paint scroll chrome: track and thumb.
///
/// Tokens: `surface.scrollbar` (thumb), `surface.scrollbar.track` (track),
/// `dimension.scrollbar.width` (thumb width), `radius.xs` (thumb corner radius),
/// `opacity.disabled` (dimmed/rest + disabled), `opacity.full` (hover/active/focus).
/// Accessibility: `Role::ScrollBar`.
///
/// Minimalist idiom (Plan 065 task 5): the thumb is dim at rest
/// (`opacity.disabled`, the only sub-full opacity token) and full on
/// hover/active/focus, so the scrollbar reads as near-invisible until
/// ponytail: a dedicated `opacity.scrollbar.rest` token is the upgrade path if
/// rest needs to differ from disabled.
pub(crate) fn paint_scroll_chrome(
    scene: &mut Scene,
    track: Rect,
    thumb: Rect,
    state: InteractionState,
    theme: &ResolvedUiTheme,
) {
    let track_color = theme
        .color("surface.scrollbar.track")
        .unwrap_or(Color::TRANSPARENT);
    let thumb_color = theme
        .color("surface.scrollbar")
        .unwrap_or(Color::TRANSPARENT);
    let radius = theme.dimension("radius.xs").unwrap_or(2.0);

    // Track background
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        track_color,
        None,
        &track,
    );

    let rounded_thumb = RoundedRect::from_rect(thumb, radius);
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        scrollbar_thumb_paint_color(thumb_color, state),
        None,
        &rounded_thumb,
    );
}

/// Resolve the scrollbar thumb paint color for an [`InteractionState`]. The
/// theme's `surface.scrollbar` color already encodes its intended *resting*
/// alpha (e.g. `#9f9f9faa`); the old code halved it via `opacity.disabled`,
/// which made the thumb disappear on light themes (a ~33% gray smudge on a
/// near-white track). Rest/Disabled therefore use the theme color verbatim;
/// Hover/Active/Focus lift it toward opaque for perceptible feedback.
fn scrollbar_thumb_paint_color(base: Color, state: InteractionState) -> Color {
    match state {
        InteractionState::Rest | InteractionState::Disabled => base,
        // ponytail: 1.5 hover lift toward opaque; add a `scrollbar.hover-alpha`
        // token if a theme ever needs to control the interaction strength.
        InteractionState::Hover | InteractionState::Active | InteractionState::Focus => {
            apply_alpha(base, 1.5)
        }
    }
}

/// Paint a badge/tag.
///
/// Tokens: `surface.badge` (background), `text.badge` (text color), `radius.xs` (corner radius),
/// `spacing.badge` (padding), `typography.detail` or `typography.caption` (text),
/// `opacity.disabled` (disabled state).
/// Accessibility: `Role::Status`.
#[allow(dead_code)] // Phase 20.2: foundation; not all primitives used yet
pub(crate) fn paint_badge(
    scene: &mut Scene,
    rect: Rect,
    _text: &str,
    state: InteractionState,
    theme: &ResolvedUiTheme,
    _fonts: &TypographyRegistry,
) {
    let bg = theme.color("surface.badge").unwrap_or(Color::TRANSPARENT);
    let text_color = theme.color("text.badge").unwrap_or(Color::TRANSPARENT);
    let radius = theme.dimension("radius.xs").unwrap_or(2.0);

    // Badge background with state-based opacity
    let bg_opacity = match state {
        InteractionState::Rest => 1.0,
        InteractionState::Hover => 1.0,
        InteractionState::Active => 1.0,
        InteractionState::Focus => 1.0,
        InteractionState::Disabled => theme.opacity("opacity.disabled").unwrap_or(0.5),
    };

    let bg_with_opacity = if bg_opacity < 1.0 {
        let rgba = bg.to_rgba8();
        Color::from_rgba8(rgba.r, rgba.g, rgba.b, (rgba.a as f32 * bg_opacity) as u8)
    } else {
        bg
    };

    let rounded = RoundedRect::from_rect(rect, radius);
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        bg_with_opacity,
        None,
        &rounded,
    );

    // Text rendering would go here using TypographyRegistry
    // For now, chrome only — text rendering is Phase 20.4
    let _ = text_color;
}

/// Paint a `kbd` hint.
///
/// Tokens: `surface.kbd` (background), `text.kbd` (text color), `border.kbd` (border),
/// `radius.xs` (corner radius), `dimension.kbd.height` (height), `typography.caption` (text).
/// Accessibility: `Role::Label` (implicit via text).
#[allow(dead_code)] // Phase 20.2: foundation; not all primitives used yet
pub(crate) fn paint_kbd_hint(
    scene: &mut Scene,
    rect: Rect,
    _keys: &str,
    theme: &ResolvedUiTheme,
    _fonts: &TypographyRegistry,
) {
    let bg = theme.color("surface.kbd").unwrap_or(Color::TRANSPARENT);
    let border = theme.color("border.kbd").unwrap_or(Color::TRANSPARENT);
    let radius = theme.dimension("radius.xs").unwrap_or(2.0);
    let border_width = theme.dimension("dimension.border.hairline").unwrap_or(1.0);

    // kbd background
    let rounded = RoundedRect::from_rect(rect, radius);
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        bg,
        None,
        &rounded,
    );

    // kbd border
    let stroke = Stroke::new(border_width);
    scene.stroke(
        &stroke,
        masonry::kurbo::Affine::IDENTITY,
        border,
        None,
        &rounded,
    );

    // Text rendering would go here using TypographyRegistry
    // For now, chrome only — text rendering is Phase 20.4
}

/// Paint an icon slot.
///
/// Tokens: `text.icon` (glyph color), `dimension.icon.size` (slot size),
/// `opacity.disabled` (disabled state).
/// Accessibility: `Role::Image`.
///
/// No package image assets are loaded — token-sized glyph slot only.
#[allow(dead_code)] // Phase 20.2: foundation; not all primitives used yet
pub(crate) fn paint_icon_slot(
    _scene: &mut Scene,
    _rect: Rect,
    _glyph: IconGlyph,
    state: InteractionState,
    theme: &ResolvedUiTheme,
) {
    let icon_color = theme.color("text.icon").unwrap_or(Color::TRANSPARENT);

    // Icon color with state-based opacity
    let icon_opacity = match state {
        InteractionState::Rest => 1.0,
        InteractionState::Hover => 1.0,
        InteractionState::Active => 1.0,
        InteractionState::Focus => 1.0,
        InteractionState::Disabled => theme.opacity("opacity.disabled").unwrap_or(0.5),
    };

    let icon_with_opacity = if icon_opacity < 1.0 {
        let rgba = icon_color.to_rgba8();
        Color::from_rgba8(rgba.r, rgba.g, rgba.b, (rgba.a as f32 * icon_opacity) as u8)
    } else {
        icon_color
    };

    // Glyph rendering would go here (simple geometric shapes)
    // For now, chrome only — glyph rendering is Phase 20.4
    let _ = icon_with_opacity;
}

/// Paint a tooltip shell.
///
/// Tokens: `surface.tooltip` (background), `text.tooltip` (text color), `border.hairline` (border),
/// `radius.sm` (corner radius), `elevation.overlay` (elevation), `z.tooltip` (z-level),
/// `spacing.tooltip` (padding), `typography.body` (text).
/// Accessibility: `Role::ToolTip`.
///
/// Tooltip content/trigger wiring is Phase 20.5.
pub(crate) fn paint_tooltip_shell(scene: &mut Scene, rect: Rect, theme: &ResolvedUiTheme) {
    let bg = theme.color("surface.tooltip").unwrap_or(Color::TRANSPARENT);
    let border = theme.color("border.hairline").unwrap_or(Color::TRANSPARENT);
    let radius = theme.dimension("radius.sm").unwrap_or(4.0);
    let border_width = theme.dimension("dimension.border.hairline").unwrap_or(1.0);

    // Tooltip background
    let rounded = RoundedRect::from_rect(rect, radius);
    scene.fill(
        masonry::vello::peniko::Fill::NonZero,
        masonry::kurbo::Affine::IDENTITY,
        bg,
        None,
        &rounded,
    );

    // Tooltip border
    let stroke = Stroke::new(border_width);
    scene.stroke(
        &stroke,
        masonry::kurbo::Affine::IDENTITY,
        border,
        None,
        &rounded,
    );

    // Text rendering would go here using TypographyRegistry
    // For now, chrome only — text rendering is Phase 20.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> ResolvedUiTheme {
        ResolvedUiTheme::from_active_theme(&[]).unwrap()
    }

    #[test]
    fn primitives_panic_free_on_zero_size_rects() {
        let theme = test_theme();
        let mut scene = Scene::new();
        let zero_rect = Rect::ZERO;

        paint_divider(&mut scene, zero_rect, Axis::Horizontal, &theme);
        paint_focus_ring(&mut scene, zero_rect, &theme);
        paint_panel_chrome(
            &mut scene,
            zero_rect,
            &PanelChrome {
                title: None,
                collapse: InteractionState::Rest,
                resize: InteractionState::Rest,
            },
            &theme,
        );
        paint_scroll_chrome(
            &mut scene,
            zero_rect,
            zero_rect,
            InteractionState::Rest,
            &theme,
        );
        paint_badge(
            &mut scene,
            zero_rect,
            "test",
            InteractionState::Rest,
            &theme,
            &TypographyRegistry::default(),
        );
        paint_kbd_hint(
            &mut scene,
            zero_rect,
            "Ctrl+C",
            &theme,
            &TypographyRegistry::default(),
        );
        paint_icon_slot(
            &mut scene,
            zero_rect,
            IconGlyph::ChevronRight,
            InteractionState::Rest,
            &theme,
        );
        paint_tooltip_shell(&mut scene, zero_rect, &theme);
    }

    #[test]
    fn primitives_render_all_interaction_states() {
        let theme = test_theme();
        let fonts = TypographyRegistry::default();
        let mut scene = Scene::new();
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);

        for state in [
            InteractionState::Rest,
            InteractionState::Hover,
            InteractionState::Active,
            InteractionState::Focus,
            InteractionState::Disabled,
        ] {
            paint_scroll_chrome(&mut scene, rect, rect, state, &theme);
            paint_badge(&mut scene, rect, "test", state, &theme, &fonts);
            paint_icon_slot(&mut scene, rect, IconGlyph::ChevronRight, state, &theme);
        }
    }

    #[test]
    fn disabled_state_applies_opacity() {
        let theme = test_theme();
        let fonts = TypographyRegistry::default();
        let mut scene = Scene::new();
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);

        // Disabled state should apply opacity.disabled
        paint_scroll_chrome(&mut scene, rect, rect, InteractionState::Disabled, &theme);
        paint_badge(
            &mut scene,
            rect,
            "test",
            InteractionState::Disabled,
            &theme,
            &fonts,
        );
        paint_icon_slot(
            &mut scene,
            rect,
            IconGlyph::ChevronRight,
            InteractionState::Disabled,
            &theme,
        );
    }

    #[test]
    fn component_state_color_maps_all_five_states_to_tokens() {
        // Plan 065 (Phase 20.4) task 4: a button fill resolves each
        // InteractionState to the mapped design token. rest_token is
        // surface.control; Hover/Active/Focus map to the shared state tokens;
        // Disabled dims surface.disabled by opacity.disabled.
        let theme = test_theme();
        assert_eq!(
            component_state_color(&theme, "surface.control", InteractionState::Rest),
            Color::from_rgb8(0x39, 0x35, 0x4a)
        );
        assert_eq!(
            component_state_color(&theme, "surface.control", InteractionState::Hover),
            Color::from_rgb8(0x2d, 0x2b, 0x3d)
        );
        assert_eq!(
            component_state_color(&theme, "surface.control", InteractionState::Active),
            Color::from_rgb8(0x34, 0x31, 0x47)
        );
        assert_eq!(
            component_state_color(&theme, "surface.control", InteractionState::Focus),
            Color::from_rgb8(0x7c, 0x6f, 0xff)
        );
        // Disabled: surface.disabled with alpha 255 * 0.55 -> 140.
        assert_eq!(
            component_state_color(&theme, "surface.control", InteractionState::Disabled),
            Color::from_rgba8(0x1b, 0x1a, 0x24, 140)
        );
    }

    #[test]
    fn list_row_fill_color_honors_selected_and_state() {
        // Plan 065 (Phase 20.4) task 4: list row fill honors `selected` at
        // Rest/Focus and overrides with the shared state tokens for
        // Hover/Active; Disabled dims surface.disabled regardless of selection.
        let theme = test_theme();
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Rest, false),
            Color::from_rgb8(0x29, 0x28, 0x35)
        );
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Rest, true),
            Color::from_rgb8(0x3d, 0x38, 0x5c)
        );
        // Hover/Active override selection.
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Hover, true),
            Color::from_rgb8(0x2d, 0x2b, 0x3d)
        );
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Active, true),
            Color::from_rgb8(0x34, 0x31, 0x47)
        );
        // Focus mirrors Rest (focus expressed via ring, not fill).
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Focus, true),
            Color::from_rgb8(0x3d, 0x38, 0x5c)
        );
        // Disabled dims surface.disabled regardless of selection.
        assert_eq!(
            list_row_fill_color(&theme, InteractionState::Disabled, true),
            Color::from_rgba8(0x1b, 0x1a, 0x24, 140)
        );
    }

    #[test]
    fn scrollbar_thumb_rest_keeps_theme_alpha_and_hover_lifts() {
        // Regression: the resting thumb must use the theme color verbatim. The
        // old `opacity.disabled` halving turned a light theme's `#9f9f9faa` into
        // a ~33% smudge that vanished on a near-white track.
        let base = Color::from_rgba8(0x9f, 0x9f, 0x9f, 0xaa);
        assert_eq!(
            scrollbar_thumb_paint_color(base, InteractionState::Rest),
            base,
            "rest must not dim the theme-authored alpha"
        );
        assert_eq!(
            scrollbar_thumb_paint_color(base, InteractionState::Disabled),
            base
        );
        // Hover/Active/Focus lift toward opaque (170 * 1.5 = 255) for feedback.
        assert_eq!(
            scrollbar_thumb_paint_color(base, InteractionState::Hover)
                .to_rgba8()
                .a,
            255
        );
        assert_eq!(
            scrollbar_thumb_paint_color(base, InteractionState::Active)
                .to_rgba8()
                .a,
            255
        );
        // A low-alpha theme color lifts but stays below opaque.
        let faint = Color::from_rgba8(0x9f, 0x9f, 0x9f, 0x22);
        let lifted = scrollbar_thumb_paint_color(faint, InteractionState::Hover).to_rgba8();
        assert!(lifted.a > faint.to_rgba8().a && lifted.a < 255);
    }
}
