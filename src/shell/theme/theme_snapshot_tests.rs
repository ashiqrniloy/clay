//! Snapshot/DTO/contrast unit tests for the resolved theme token surface
//! (plan 133 task 7: moved verbatim from `src/shell/theme.rs`; module path
//! `shell::theme::theme_snapshot_tests` unchanged).

use super::*;
use crate::protocol::{ActiveTheme, UiDesignTokenOverride, WireDesignTokenValue};

fn empty_theme() -> ActiveTheme {
    ActiveTheme {
        specifier: String::new(),
        overrides: Vec::new(),
        design_tokens: Vec::new(),
    }
}

#[test]
fn core_token_names_are_complete_and_resolvable() {
    assert_eq!(CORE_TOKEN_NAMES.len(), 91);
    for name in CORE_TOKEN_NAMES {
        assert!(
            core_theme_value(name).is_some(),
            "CORE_TOKEN_NAMES entry {name} has no core value"
        );
    }
    // Sorted for deterministic snapshots.
    let mut sorted = CORE_TOKEN_NAMES.to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted.as_slice(), CORE_TOKEN_NAMES);
}

#[test]
fn snapshot_resolves_every_token_from_core_fallbacks() {
    let map = resolve_theme_token_snapshot(&empty_theme()).expect("core defaults pass contrast");
    assert_eq!(map.len(), CORE_TOKEN_NAMES.len());
    // The legacy editor-base projection layers above core fallbacks, so an
    // empty theme resolves surfaces to the editor default palette.
    assert_eq!(
        map.get("surface.main"),
        Some(&ThemeTokenValueDto::Color("#181818".to_string()))
    );
    // Tokens outside the base projection stay core.
    assert_eq!(map.get("radius.xs"), Some(&ThemeTokenValueDto::Scalar(2.0)));
    assert_eq!(
        map.get("density.default"),
        Some(&ThemeTokenValueDto::Level("default".to_string()))
    );
    assert!(matches!(
        map.get("opacity.disabled"),
        Some(ThemeTokenValueDto::Opacity(value)) if (*value - 0.55).abs() < 1e-6
    ));
    assert_eq!(
        map.get("typography.title"),
        Some(&ThemeTokenValueDto::Variant("title".to_string()))
    );
    assert_eq!(
        map.get("motion.fast"),
        Some(&ThemeTokenValueDto::Scalar(100.0))
    );
    assert_eq!(map.get("radius.xs"), Some(&ThemeTokenValueDto::Scalar(2.0)));
}

#[test]
fn core_catalog_meets_every_required_contrast_pair() {
    // The core catalog is the palette Clay paints before any theme snapshot
    // arrives, and what a role falls back to when a theme does not declare
    // it. It therefore has to clear the same floors a theme does — this is
    // also the only path that runs no runtime gate (no activation, no
    // package), so the assertion lives here.
    let core = ResolvedUiTheme::from_active_theme(&[]).expect("empty overrides are valid");
    if let Err(failure) = theme_meets_contrast(&core) {
        panic!(
            "core catalog fails {}/{}: {:.2} < {:.1}",
            failure.foreground, failure.background, failure.ratio, failure.threshold
        );
    }

    // The border ladder is monotonic on every surface a boundary is drawn
    // against (`DESIGN.md` §10.1: 34 % grey, 100 % grey, ink).
    let ratio = |role: &str, surface: &str| {
        crate::editor::theme::composited_contrast_ratio(
            core.color(role).expect(role),
            core.color(surface).expect(surface),
        )
    };
    for surface in ["surface.main", "surface.panel"] {
        let hairline = ratio("border.hairline", surface);
        let subtle = ratio("border.subtle", surface);
        let strong = ratio("border.strong", surface);
        assert!(
            hairline < subtle && subtle < strong,
            "{surface} ladder is not monotonic: hairline {hairline:.2}, subtle {subtle:.2}, strong {strong:.2}"
        );
    }
}

#[test]
fn host_theme_role_block_mirrors_the_core_catalog() {
    // `frontend/src/styles/tokens.css` paints the shell before the theme
    // snapshot lands. Every colour role it declares must equal the core
    // catalog value the snapshot installs, or the window repaints on the
    // first frame (the pre-bootstrap half of the task 16 invariant, for
    // theme roles rather than design-system recipes). Ten roles had drifted
    // from the catalog before this test existed.
    fn css_rgba(value: &str) -> Option<[u8; 4]> {
        let digits = value.trim().strip_prefix('#')?;
        let byte = |from: usize| u8::from_str_radix(digits.get(from..from + 2)?, 16).ok();
        match digits.len() {
            6 => Some([byte(0)?, byte(2)?, byte(4)?, 0xff]),
            8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
            _ => None,
        }
    }

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/frontend/src/styles/tokens.css"
    );
    let css = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path}: {err}"));
    // The design-system recipe block has its own mirror test; the theme-role
    // block is everything before its marker.
    let end = css
        .find("/* Host fallback design-system recipe variables")
        .unwrap_or(css.len());
    let mut host = std::collections::BTreeMap::new();
    for declaration in css[..end].split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let Some(role) = name.trim().strip_prefix("--clay-") else {
            continue;
        };
        if let Some(rgba) = css_rgba(value) {
            host.insert(role.replace('-', "."), rgba);
        }
    }

    let mut checked = 0usize;
    for name in CORE_TOKEN_NAMES {
        let Some(CoreThemeValue {
            value: ResolvedThemeValue::Color(core),
            ..
        }) = core_theme_value(name)
        else {
            continue;
        };
        let Some(rgba) = host.get(*name) else {
            continue; // the host states only the roles its CSS consumes
        };
        assert_eq!(
            *rgba,
            core.components(),
            "`--clay-{}` must mirror the core catalog (`{name}`)",
            name.replace('.', "-")
        );
        checked += 1;
    }
    assert!(
        checked >= 25,
        "expected the host theme-role block to state the core palette (checked={checked})"
    );
}

#[test]
fn override_wins_over_core_fallback() {
    let theme = ActiveTheme {
        design_tokens: vec![UiDesignTokenOverride {
            token: "surface.hover".to_string(),
            value: WireDesignTokenValue::Color([0xff, 0x00, 0x00, 0xff]),
            provenance: "@test/theme".to_string(),
        }],
        ..empty_theme()
    };
    let map = resolve_theme_token_snapshot(&theme).expect("valid");
    assert_eq!(
        map.get("surface.hover"),
        Some(&ThemeTokenValueDto::Color("#ff0000".to_string()))
    );
    // Untouched neighbors keep their resolved (base-projected/core) values.
    assert_eq!(map.get("radius.xs"), Some(&ThemeTokenValueDto::Scalar(2.0)));
}

#[test]
fn density_scale_reads_resolved_level() {
    assert!((density_spacing_scale(&std::collections::BTreeMap::new()) - 1.0).abs() < f64::EPSILON);
    let mut map = std::collections::BTreeMap::new();
    map.insert(
        "density.default".to_string(),
        ThemeTokenValueDto::Level("compact".to_string()),
    );
    assert!((density_spacing_scale(&map) - 0.875).abs() < f64::EPSILON);
}

#[test]
fn dto_round_trips_through_json() {
    let map = resolve_theme_token_snapshot(&empty_theme()).expect("valid");
    let json = serde_json::to_string(&map).expect("serialize");
    let back: std::collections::BTreeMap<String, ThemeTokenValueDto> =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, map);
}
