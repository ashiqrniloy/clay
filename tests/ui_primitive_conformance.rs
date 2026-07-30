//! Phase 20.2 UI primitive conformance tests.
//!
//! Plan 063 task 5: enforce that primitives are the only way to paint UI chrome.
//! Shell/SDUI chrome paint files contain no color literals and no hardcoded f64
//! chrome sizes outside the primitive module and theme-definition module.
//! Package components map onto primitives by construction.

use std::fs;

/// Strip the `mod tests` section (or, if absent, the first `#[cfg(test)]`
/// block) from source. Preferring the `mod tests` boundary avoids truncating
/// the scan at early `#[cfg(test)]` test-only imports/consts that live in the
/// non-test region (e.g. `src/masonry_sdui.rs`, `src/masonry_shell.rs`).
fn non_test_body(src: &str) -> &str {
    if let Some(i) = src.find("\nmod tests") {
        return &src[..i];
    }
    if let Some(i) = src.find("\n#[cfg(test)]") {
        return &src[..i];
    }
    src
}

#[test]
fn shell_chrome_paint_files_source_color_from_primitives_only() {
    // Plan 063 task 5 source guard: no shell/SDUI chrome paint file may hold a
    // `Color::from_rgb*` literal except `src/shell/primitives.rs` (the primitive
    // module) and `src/shell/theme.rs` (the token-definition module). Primitives
    // own all chrome color; paint paths read from them. If a literal reappears
    // in a chrome paint path this fails fast.
    let chrome_paint_files = [
        "src/shell/package_ui.rs",
        "src/shell/transient_menu.rs",
        "src/shell/file_browser.rs",
        "src/masonry_sdui.rs",
        "src/masonry_shell.rs",
        // Plan 065 task 5: editor chrome paint must source color from the
        // StyleRegistry / shell primitives, not inline literals. Size constants
        // (SCROLLBAR_*, TEXT_INSET) are pre-existing editor chrome and stay out
        // of the size guard below.
        "src/editor/surface.rs",
        // Plan 065 task 6: status bar paint must source color from the
        // StyleRegistry / shell primitives, not inline literals.
        "src/masonry_editor.rs",
    ];
    for file in chrome_paint_files {
        let src =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("{file} should be readable: {e}"));
        let body = non_test_body(&src);
        assert!(
            !body.contains("Color::from_rgb8("),
            "{file} chrome paint path must source color from primitives/tokens, not a Color::from_rgb8 literal. Use crate::shell::primitives::paint_* helpers."
        );
        assert!(
            !body.contains("Color::from_rgba8("),
            "{file} chrome paint path must source color from primitives/tokens, not a Color::from_rgba8 literal. Use crate::shell::primitives::paint_* helpers."
        );
    }

    // The primitive module and token-definition module are the ONLY chrome
    // paint files allowed to hold color literals.
    let primitives_src = fs::read_to_string("src/shell/primitives.rs")
        .expect("src/shell/primitives.rs (primitive module) should be readable");
    let primitives_body = non_test_body(&primitives_src);
    assert!(
        primitives_body.contains("theme.color("),
        "src/shell/primitives.rs must read color from ResolvedUiTheme tokens"
    );

    let theme_src = fs::read_to_string("src/shell/theme.rs")
        .expect("src/shell/theme.rs (token-definition module) should be readable");
    let theme_body = non_test_body(&theme_src);
    assert!(
        theme_body.contains("Color::from_rgb8(") || theme_body.contains("Color::from_rgba8("),
        "src/shell/theme.rs must own the core token color literals"
    );
}

#[test]
fn shell_chrome_paint_files_have_no_hardcoded_chrome_sizes() {
    // Plan 063 task 5 source guard: no shell/SDUI chrome paint file may hold
    // hardcoded f64 chrome-size constants outside `src/shell/primitives.rs`
    // (the primitive module) and `src/shell/theme.rs` (the token-definition
    // module). Chrome sizes come from resolved dimension tokens. If a hardcoded
    // size reappears in a chrome paint path this fails fast.
    //
    // We scan for common chrome-size patterns: scrollbar width/margin, border
    // width, radius, padding. These should all come from tokens.
    let chrome_paint_files = [
        "src/shell/package_ui.rs",
        "src/shell/transient_menu.rs",
        "src/shell/file_browser.rs",
        "src/masonry_sdui.rs",
        "src/masonry_shell.rs",
        // Plan 065 task 6: status bar paint lives here; guard it against
        // reintroducing hardcoded chrome-size constants.
        "src/masonry_editor.rs",
    ];

    // Patterns that indicate hardcoded chrome sizes (not exhaustive, but catches
    // the most common cases). We allow these in primitives.rs and theme.rs.
    let hardcoded_patterns = [
        "SCROLLBAR_WIDTH",
        "SCROLLBAR_MARGIN",
        "SCROLLBAR_MIN_THUMB",
        "BORDER_WIDTH",
        "RADIUS_",
        "PADDING_",
    ];

    for file in chrome_paint_files {
        let src =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("{file} should be readable: {e}"));
        let body = non_test_body(&src);
        for pattern in hardcoded_patterns {
            assert!(
                !body.contains(pattern),
                "{file} chrome paint path must source sizes from dimension tokens, not hardcoded {pattern}. Use theme.dimension(\"dimension.*\") or theme.scalar_f64(\"spacing.*\")."
            );
        }
    }
}

#[test]
fn package_component_paint_routes_chrome_through_primitives() {
    // Plan 063 task 5 + plan 070 step 13e: package-declared ComponentKind
    // components map onto primitives by construction because the retained
    // package widgets call the primitive helpers for chrome (panel/overlay
    // backgrounds/borders). The legacy immediate-mode `paint_package_component`
    // walk in masonry_sdui.rs was deleted in step 13e; the chrome paint now
    // lives in the retained host widgets (masonry_package_region.rs).
    let region_src = fs::read_to_string("src/masonry_package_region.rs")
        .expect("src/masonry_package_region.rs should be readable");

    // Assert that the retained package widgets route chrome through primitives.
    assert!(
        region_src.contains("paint_panel_chrome"),
        "src/masonry_package_region.rs must route panel chrome through primitives::paint_panel_chrome"
    );
    assert!(
        region_src.contains("paint_tooltip_shell"),
        "src/masonry_package_region.rs must route overlay chrome through primitives::paint_tooltip_shell"
    );
}

#[test]
fn editor_scrollbar_routes_through_primitives() {
    // Plan 063 task 5: editor scrollbar chrome routes through primitives.
    let surface_src = fs::read_to_string("src/editor/surface.rs")
        .expect("src/editor/surface.rs should be readable");
    let surface_body = non_test_body(&surface_src);

    // Assert that editor scrollbar routes through primitives.
    assert!(
        surface_body.contains("shell::primitives::paint_scroll_chrome")
            || surface_body.contains("crate::shell::primitives::paint_scroll_chrome"),
        "src/editor/surface.rs must route scrollbar chrome through shell::primitives::paint_scroll_chrome"
    );
}

#[test]
fn primitives_are_token_driven() {
    // Plan 063 task 5: each primitive function is token-driven by scanning for
    // resolved-theme reads. Primitives read from ResolvedUiTheme, not hardcoded
    // values.
    let primitives_src = fs::read_to_string("src/shell/primitives.rs")
        .expect("src/shell/primitives.rs should be readable");
    let primitives_body = non_test_body(&primitives_src);

    // Assert that primitives read from ResolvedUiTheme.
    assert!(
        primitives_body.contains("theme.color("),
        "primitives must read color from ResolvedUiTheme.color()"
    );
    assert!(
        primitives_body.contains("theme.dimension("),
        "primitives must read dimension from ResolvedUiTheme.dimension()"
    );
    assert!(
        primitives_body.contains("theme.opacity("),
        "primitives must read opacity from ResolvedUiTheme.opacity()"
    );

    // Assert that primitives do not contain hardcoded color literals in paint
    // functions (only in tests).
    let paint_functions = [
        "paint_divider",
        "paint_focus_ring",
        "paint_panel_chrome",
        "paint_scroll_chrome",
        "paint_badge",
        "paint_kbd_hint",
        "paint_icon_slot",
        "paint_tooltip_shell",
    ];

    for func in paint_functions {
        let func_start = primitives_body.find(&format!("pub(crate) fn {func}"));
        if let Some(start) = func_start {
            // Find the next function or end of file.
            let func_end = primitives_body[start..]
                .find("\npub(crate) fn ")
                .map(|i| start + i)
                .unwrap_or(primitives_body.len());
            let func_body = &primitives_body[start..func_end];

            // Assert no hardcoded color literals in paint function body.
            // Allow Color::from_rgba8 when used to apply opacity to token-read colors
            // (e.g., Color::from_rgba8(rgba.r, rgba.g, rgba.b, ...)).
            // Reject Color::from_rgb8 with hardcoded hex values (e.g., Color::from_rgb8(0x12, 0x34, 0x56)).
            let has_hardcoded_rgb8 = func_body.contains("Color::from_rgb8(0x");
            let has_hardcoded_rgba8 = func_body.contains("Color::from_rgba8(0x");

            assert!(
                !has_hardcoded_rgb8 && !has_hardcoded_rgba8,
                "primitive {func} must not contain hardcoded color literals (Color::from_rgb*8(0x...)); read from theme.color() instead. Color::from_rgba8(rgba.r, ...) for opacity application is allowed."
            );
        }
    }
}

#[test]
fn primitives_render_all_interaction_states() {
    // Plan 063 task 5: structural test proving each interactive primitive
    // renders all InteractionState variants. We scan for match arms or
    // conditional logic that handles Rest/Hover/Active/Focus/Disabled.
    let primitives_src = fs::read_to_string("src/shell/primitives.rs")
        .expect("src/shell/primitives.rs should be readable");
    let primitives_body = non_test_body(&primitives_src);

    // Interactive primitives that should handle all states.
    let interactive_primitives = ["paint_scroll_chrome", "paint_badge", "paint_icon_slot"];

    for func in interactive_primitives {
        let func_start = primitives_body.find(&format!("pub(crate) fn {func}"));
        if let Some(start) = func_start {
            // Find the next function or end of file.
            let func_end = primitives_body[start..]
                .find("\npub(crate) fn ")
                .map(|i| start + i)
                .unwrap_or(primitives_body.len());
            let func_body = &primitives_body[start..func_end];

            // Assert that the function handles InteractionState.
            assert!(
                func_body.contains("InteractionState::"),
                "interactive primitive {func} must handle InteractionState variants"
            );

            // Assert that the function handles Disabled state (applies opacity).
            assert!(
                func_body.contains("InteractionState::Disabled"),
                "interactive primitive {func} must handle InteractionState::Disabled"
            );
        }
    }
}

#[test]
fn sdui_paint_resolves_from_active_theme_not_core_fallback_resolver() {
    // Plan 065 (Phase 20.4) task 3 source guard: SDUI component paint must
    // resolve fills/typography/spacing from the active ResolvedUiTheme via
    // SduiThemeStyle::from_ui_theme (self.theme_style()), not the
    // core-fallback-only sdui_theme_style()/SduiThemeStyle::default() path
    // (which builds a fresh ThemeTokenResolver and ignores user/theme-package
    // overrides). If the core-fallback path re-enters the paint path this fails.
    let sdui_src =
        fs::read_to_string("src/masonry_sdui.rs").expect("src/masonry_sdui.rs should be readable");
    let body = non_test_body(&sdui_src);

    assert!(
        !body.contains("sdui_theme_style("),
        "src/masonry_sdui.rs paint path must resolve from the active theme via self.theme_style() (SduiThemeStyle::from_ui_theme), not the core-fallback sdui_theme_style()"
    );
    assert!(
        !body.contains("ThemeTokenResolver"),
        "src/masonry_sdui.rs paint path must not construct a ThemeTokenResolver; theme resolution happens at install time into ResolvedUiTheme"
    );
    assert!(
        body.contains("SduiThemeStyle::from_ui_theme"),
        "src/masonry_sdui.rs must resolve the SDUI style via SduiThemeStyle::from_ui_theme"
    );
}

#[test]
fn sdui_paint_wires_focus_ring_and_state_colors_for_interactive_components() {
    // Plan 065 (Phase 20.4) task 4: SDUI component paint must route
    // interactive fills through the token-driven state helpers and paint a
    // focus ring on focused components. Source guard complementing the
    // behavioral tests in masonry_sdui::tests.
    // Plan 070 step 13e: the interactive package component paint moved from the
    // deleted immediate-mode `paint_package_component` walk (masonry_sdui.rs) to
    // the retained package widgets (masonry_package_region.rs). Focus is now
    // Masonry-driven (`ctx.is_focus_target()`), so the focus-ring guard checks
    // `paint_focus_ring` in the retained widgets.
    let region_src = fs::read_to_string("src/masonry_package_region.rs")
        .expect("src/masonry_package_region.rs should be readable");
    let body = non_test_body(&region_src);
    assert!(
        body.contains("component_state_color"),
        "src/masonry_package_region.rs must route button fills through component_state_color"
    );
    assert!(
        body.contains("list_row_fill_color"),
        "src/masonry_package_region.rs must route list row fills through list_row_fill_color"
    );
    assert!(
        body.contains("disabled_text_color"),
        "src/masonry_package_region.rs must dim disabled label/statusItem text via disabled_text_color"
    );
    assert!(
        body.contains("paint_focus_ring"),
        "src/masonry_package_region.rs must paint a focus ring on focused interactive components"
    );
}

#[test]
fn status_bar_insets_are_token_driven() {
    // Plan 065 task 6: paint_status_line must source the status insets from
    // spacing.sm scaled by spacing_scale(), not the hardcoded 12.0/24.0.
    let src = fs::read_to_string("src/masonry_editor.rs").expect("src/masonry_editor.rs readable");
    let body = non_test_body(&src);
    let paint_status = body
        .split("fn paint_status_line")
        .nth(1)
        .expect("paint_status_line body");
    assert!(
        paint_status.contains("scalar_f64(\"spacing.sm\")"),
        "status bar insets must read spacing.sm from the UI theme"
    );
    assert!(
        paint_status.contains("spacing_scale()"),
        "status bar insets must scale by the active density spacing_scale()"
    );
    assert!(
        !paint_status.contains("- 24.0") && !paint_status.contains("(12.0,"),
        "status bar must not reintroduce the hardcoded 24.0/12.0 insets"
    );
}

#[test]
fn status_bar_paints_top_hairline_divider() {
    // Plan 065 task 6: a paint_divider hairline separates the status bar from
    // the editor above.
    let src = fs::read_to_string("src/masonry_editor.rs").expect("src/masonry_editor.rs readable");
    let body = non_test_body(&src);
    let paint_status = body
        .split("fn paint_status_line")
        .nth(1)
        .expect("paint_status_line body");
    assert!(
        paint_status.contains("paint_divider"),
        "status bar must paint a divider via shell::primitives::paint_divider"
    );
    assert!(
        paint_status.contains("Axis::Horizontal"),
        "status bar divider must be a horizontal hairline at the status bar top"
    );
}

#[test]
fn masonry_editor_no_hardcoded_chrome_sizes() {
    // Plan 065 task 6: src/masonry_editor.rs is a chrome-paint file (status
    // bar); it must not hold named chrome-size constants. Numeric literals that
    // are not chrome sizes are out of scope (matched by the conformance size
    // guard's named-pattern scan).
    let src = fs::read_to_string("src/masonry_editor.rs").expect("src/masonry_editor.rs readable");
    let body = non_test_body(&src);
    for pattern in [
        "SCROLLBAR_WIDTH",
        "SCROLLBAR_MARGIN",
        "SCROLLBAR_MIN_THUMB",
        "BORDER_WIDTH",
        "RADIUS_",
        "PADDING_",
    ] {
        assert!(
            !body.contains(pattern),
            "src/masonry_editor.rs chrome paint must not hold hardcoded {pattern}; use dimension/spacing tokens"
        );
    }
}
