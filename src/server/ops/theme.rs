//! `setTheme` Clay JS op (Plan 046 task 7, `clay:theme` facade).
//!
//! Selects the one active theme by resolving a first-party `@clay/*` theme
//! package's inert `clay.contributions.textStyles` overrides into a
//! [`crate::protocol::ActiveTheme`] snapshot. The snapshot is carried out in
//! [`crate::server::js_runtime::ClayRuntimeEvaluation`] and applied to the
//! shared server slot so the welcome handshake ships it to the client, which
//! reconstructs the [`crate::editor::theme::StyleRegistry`] before startup
//! paint. This is pure inert style data: no code/widgets/ops/CSS. Deny-by-
//! default for non-`@clay/*` specifiers (decision 2026-07-09-0352).

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use super::ClayOpState;
use super::packages::ensure_first_party_record;
use crate::packages::record::DesignTokenOverrideDescriptor;
use crate::protocol::{ActiveTheme, Appearance, ResolvedAppearance};
use crate::shell::theme::{ContrastFailure, validate_active_theme_contrast};

/// Canonical default theme specifiers for each resolved appearance (Phase
/// 20.6). Light → Modus Operandi, Dark → Modus Vivendi. These are the only
/// appearance-derived defaults; an explicit `setTheme` always wins.
pub(crate) const CANONICAL_LIGHT_THEME: &str = "@clay/theme-modus-operandi";
pub(crate) const CANONICAL_DARK_THEME: &str = "@clay/theme-modus-vivendi";

/// Map a resolved appearance to its canonical default theme specifier.
pub(crate) fn canonical_default_specifier(resolved: ResolvedAppearance) -> &'static str {
    match resolved {
        ResolvedAppearance::Light => CANONICAL_LIGHT_THEME,
        ResolvedAppearance::Dark => CANONICAL_DARK_THEME,
    }
}

/// Build an [`ActiveTheme`] snapshot from a validated first-party package
/// record's inert `textStyles` + `designTokens` contributions. Pure style
/// data: no code/widgets/ops/CSS. Shared by `setTheme` and the appearance
/// canonical-default resolver so both produce identical snapshots.
pub(crate) fn build_active_theme_from_record(
    specifier: &str,
    record: &crate::packages::record::PackageRecord,
) -> ActiveTheme {
    let overrides = record
        .contributions
        .text_styles
        .iter()
        .map(|descriptor| crate::protocol::TextThemeOverride {
            token: descriptor.token.clone(),
            color: descriptor.color,
            background: descriptor.background,
            bold: descriptor.bold,
            italic: descriptor.italic,
            underline: descriptor.underline,
            strike: descriptor.strike,
            scale: descriptor.scale,
            provenance: descriptor.provenance.clone(),
        })
        .collect::<Vec<_>>();
    let design_tokens = record
        .contributions
        .design_tokens
        .iter()
        .map(DesignTokenOverrideDescriptor::to_wire)
        .collect::<Vec<_>>();
    ActiveTheme {
        specifier: specifier.to_string(),
        overrides,
        design_tokens,
    }
}

/// Resolve the canonical default theme for `appearance` by loading the
/// matching first-party Modus package. Returns `None` if the package cannot be
/// resolved/enabled so a missing canonical package never breaks startup — the
/// Clay core default theme applies instead. `os_dark` is the observed OS
/// color-scheme signal (true = dark; `System` with no signal falls back dark).
pub(crate) fn resolve_canonical_default_theme(
    clay_state: &std::sync::Arc<ClayOpState>,
    appearance: Appearance,
    os_dark: bool,
) -> Option<ActiveTheme> {
    let specifier = canonical_default_specifier(appearance.resolve(os_dark));
    let (record, _package_root, _resolved_name) =
        ensure_first_party_record(clay_state, specifier).ok()?;
    let snapshot = build_active_theme_from_record(specifier, &record);
    // A canonical default failing the AA contrast floor is a build invariant
    // violation; record a diagnostic and fall back to the Clay core default
    // theme rather than installing a low-contrast palette at startup.
    if let Err(failure) = validate_active_theme_contrast(&snapshot) {
        clay_state.record(format!(
            "theme.contrast: canonical default {specifier} pair {}/{} ratio {:.2} below {:.1}",
            failure.foreground, failure.background, failure.ratio, failure.threshold
        ));
        return None;
    }
    Some(snapshot)
}

/// Apply an explicit theme selection. Validates the specifier is a
/// first-party `@clay/theme-*` package, resolves its inert `textStyles`
/// overrides, and records it as the active theme. Shared by the `setTheme` op
/// and the persisted-preference apply path so both produce identical state.
/// Returns the installed snapshot. Denies non-first-party specifiers.
pub(crate) fn apply_theme(
    clay_state: &std::sync::Arc<ClayOpState>,
    specifier: &str,
) -> Result<ActiveTheme, JsErrorBox> {
    if specifier.trim().is_empty() {
        return Err(JsErrorBox::generic(
            "theme.invalid_request: setTheme requires a non-empty `specifier`",
        ));
    }
    if !specifier.starts_with("@clay/") {
        return Err(JsErrorBox::generic(format!(
            "theme.unauthorized: setTheme denies non-first-party specifier `{specifier}`"
        )));
    }
    let (record, _package_root, _resolved_name) = ensure_first_party_record(clay_state, specifier)?;
    let snapshot = build_active_theme_from_record(specifier, &record);
    enforce_contrast(clay_state, specifier, &snapshot)?;
    clay_state.set_active_theme(snapshot.clone());
    // An explicit theme selection wins over the appearance-derived default.
    clay_state.set_explicit_theme_active(true);
    Ok(snapshot)
}

/// Enforce the WCAG AA contrast floor for a candidate theme snapshot. On
/// failure, records a `theme.contrast` diagnostic naming the failing pair,
/// ratio, and threshold, and denies the install. Active theme state is left
/// untouched on rejection so a previously valid theme remains installed.
fn enforce_contrast(
    clay_state: &std::sync::Arc<ClayOpState>,
    specifier: &str,
    snapshot: &ActiveTheme,
) -> Result<(), JsErrorBox> {
    if let Err(failure) = validate_active_theme_contrast(snapshot) {
        let message = format_contrast_failure(specifier, &failure);
        clay_state.record(message.clone());
        return Err(JsErrorBox::generic(message));
    }
    Ok(())
}

/// Render a contrast failure as a stable `theme.contrast` diagnostic
/// string naming the specifier, pair, ratio, and threshold.
fn format_contrast_failure(specifier: &str, failure: &ContrastFailure) -> String {
    format!(
        "theme.contrast: {} pair {}/{} ratio {:.2} below {:.1}",
        specifier, failure.foreground, failure.background, failure.ratio, failure.threshold
    )
}

/// Apply an appearance preference. Sets the bounded preference and, when no
/// explicit theme is active, re-resolves the canonical default theme for the
/// resolved appearance. Shared by the `setAppearance` op and the
/// persisted-preference apply path. `os_dark = true` is the no-OS-signal
/// fallback (`System` → dark). Returns the resolved canonical theme specifier,
/// if any.
pub(crate) fn apply_appearance(
    clay_state: &std::sync::Arc<ClayOpState>,
    appearance: Appearance,
    os_dark: bool,
) -> Option<String> {
    clay_state.set_appearance(appearance);
    if clay_state.explicit_theme_active() {
        return None;
    }
    let resolved = resolve_canonical_default_theme(clay_state, appearance, os_dark)?;
    let specifier = resolved.specifier.clone();
    clay_state.set_active_theme(resolved);
    Some(specifier)
}

/// Resolve `specifier` to an enabled package record's inert `textStyles`
/// overrides and record it as the active theme. Returns `{ specifier,
/// overrideCount }`. Denies any specifier that is not a first-party `@clay/*`
/// package so arbitrary theme specifiers grant no filesystem/network/extension
/// authority beyond loading a bundled package.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_theme(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|_| {
        JsErrorBox::generic("theme.invalid_request: setTheme requires { specifier: string }")
    })?;
    let Some(specifier) = request.get("specifier").and_then(Value::as_str) else {
        return Err(JsErrorBox::generic(
            "theme.invalid_request: setTheme requires a `specifier` string",
        ));
    };
    let clay_state = state.borrow::<std::sync::Arc<ClayOpState>>();
    let snapshot = apply_theme(clay_state, specifier)?;
    let override_count = snapshot.overrides.len();
    let design_token_count = snapshot.design_tokens.len();
    serde_json::to_string(&json!({
        "specifier": specifier,
        "overrideCount": override_count,
        "designTokenCount": design_token_count,
    }))
    .map_err(|_| JsErrorBox::generic("theme.invalid_request: serialization failed"))
}

/// `setAppearance` Clay JS op (Phase 20.6). Sets the bounded appearance
/// preference (`light` | `dark` | `system`) and, when no explicit theme is
/// active, re-resolves the canonical default theme for the resolved appearance.
/// An explicit `setTheme` always wins: once a theme is explicitly selected,
/// `setAppearance` no longer re-resolves over it. `system` follows the observed
/// OS color-scheme signal with a dark fallback when no signal is available.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_appearance(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|_| {
        JsErrorBox::generic("theme.invalid_request: setAppearance requires { appearance: string }")
    })?;
    let Some(appearance_str) = request.get("appearance").and_then(Value::as_str) else {
        return Err(JsErrorBox::generic(
            "theme.invalid_request: setAppearance requires an `appearance` string",
        ));
    };
    let appearance = Appearance::parse(appearance_str).ok_or_else(|| {
        JsErrorBox::generic(format!(
            "theme.invalid_request: setAppearance rejects unknown appearance `{appearance_str}`"
        ))
    })?;

    let clay_state = state.borrow::<std::sync::Arc<ClayOpState>>();
    // `os_dark = true` is the no-OS-signal fallback: `System` resolves to dark
    // (Modus Vivendi). A real client OS signal feeds this in a follow-up.
    let resolved_specifier = apply_appearance(clay_state, appearance, true);

    serde_json::to_string(&json!({
        "appearance": appearance.as_str(),
        "resolvedTheme": resolved_specifier,
    }))
    .map_err(|_| JsErrorBox::generic("theme.invalid_request: serialization failed"))
}

/// Apply an explicit UI design-system selection (Plan 102).
///
/// Resolves against exact current package records (`clay.contributions.uiDesignSystem`),
/// resolves full component recipe fallback inheritance against Clay's built-in core baseline,
/// and installs the resolved [`crate::shell::design_system::ActiveDesignSystem`] snapshot on `ClayOpState`.
///
/// Special cases:
/// - `@clay/core`, `clay:core`, `core`: selects and installs the built-in core design-system baseline.
///
/// Package specifiers:
/// - Validates non-empty string.
/// - Resolves and enables first-party (`@clay/*`) or adopted third-party packages through `PackageService`.
/// - Rejects packages that declare no `uiDesignSystem` contribution.
/// - Validates schema version, bounds, and theme color-role authority.
/// - Resolves parent design systems if `extends` is specified.
/// - Leaves previous active design system untouched on any error.
pub(crate) fn apply_design_system(
    clay_state: &std::sync::Arc<ClayOpState>,
    specifier: &str,
) -> Result<crate::shell::design_system::ActiveDesignSystem, JsErrorBox> {
    let trimmed = specifier.trim();
    if trimmed.is_empty() {
        return Err(JsErrorBox::generic(
            "theme.invalid_request: setDesignSystem requires a non-empty `specifier`",
        ));
    }

    if trimmed == "@clay/core" || trimmed == "clay:core" || trimmed == "core" {
        let active = crate::shell::design_system::ActiveDesignSystem::core_fallback(0);
        clay_state.set_active_design_system(active.clone());
        clay_state.set_explicit_design_system_active(true);
        return Ok(active);
    }

    let record = {
        let mut service = clay_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        if trimmed.starts_with("@clay/") {
            // Resolve under the already-held service guard. Calling the
            // public `ensure_first_party_record` here re-locked the same
            // non-reentrant mutex on the current thread and deadlocked the
            // worker for every non-core specifier (plan 110 task 18).
            let (record, _root, _name) =
                super::packages::ensure_first_party_record_locked(&mut service, trimmed)?;
            record
        } else {
            // Adopted third-party package
            if let Some(rec) = service
                .enabled_records()
                .find(|r| r.manifest.name == trimmed)
            {
                rec.clone()
            } else {
                match service.enable(trimmed) {
                    Ok(r) => r.clone(),
                    Err(crate::packages::service::PackageServiceError::AlreadyEnabled {
                        ..
                    }) => service
                        .enabled_records()
                        .find(|r| r.manifest.name == trimmed)
                        .ok_or_else(|| {
                            JsErrorBox::generic(format!(
                                "theme.load_failed: enabled package `{trimmed}` not found"
                            ))
                        })?
                        .clone(),
                    Err(err) => {
                        return Err(JsErrorBox::generic(format!(
                            "theme.load_failed: failed to enable package `{trimmed}`: {err}"
                        )));
                    }
                }
            }
        }
    };

    let descriptor = record
        .contributions
        .ui_design_system
        .as_ref()
        .ok_or_else(|| {
            JsErrorBox::generic(format!(
                "theme.invalid_design_system: package `{trimmed}` does not contribute a uiDesignSystem"
            ))
        })?;

    let declaration: crate::shell::design_system::UiDesignSystemDeclaration = serde_json::from_str(
        &descriptor.declaration_json,
    )
    .map_err(|err| {
        JsErrorBox::generic(format!(
            "theme.invalid_design_system: failed to deserialize uiDesignSystem declaration: {err}"
        ))
    })?;

    let parent_resolved = if let Some(ref parent_id) = declaration.extends {
        if parent_id == "@clay/core" || parent_id == "clay:core" || parent_id == "core" {
            Some(crate::shell::design_system::ResolvedUiDesignSystem::core_fallback())
        } else {
            let service = clay_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            let parent_record = service.enabled_records().find(|r| {
                r.manifest.name == *parent_id
                    || r.contributions
                        .ui_design_system
                        .as_ref()
                        .is_some_and(|ds| ds.id == *parent_id)
            });
            if let Some(parent_rec) = parent_record {
                if let Some(ref parent_ds) = parent_rec.contributions.ui_design_system {
                    let parent_decl: crate::shell::design_system::UiDesignSystemDeclaration =
                        serde_json::from_str(&parent_ds.declaration_json).map_err(|err| {
                            JsErrorBox::generic(format!(
                                "theme.invalid_design_system: parent declaration error: {err}"
                            ))
                        })?;
                    Some(
                        crate::shell::design_system::resolve_design_system(&parent_decl, None)
                            .map_err(|err| {
                                JsErrorBox::generic(format!(
                                    "theme.invalid_design_system: parent resolution failed: {err}"
                                ))
                            })?,
                    )
                } else {
                    None
                }
            } else {
                None
            }
        }
    } else {
        None
    };

    let resolved =
        crate::shell::design_system::resolve_design_system(&declaration, parent_resolved.as_ref())
            .map_err(|err| {
                JsErrorBox::generic(format!(
                    "theme.invalid_design_system: resolution failed: {err}"
                ))
            })?;

    let provenance = crate::shell::design_system::DesignSystemProvenance::from_record(&record);
    let active = crate::shell::design_system::ActiveDesignSystem::from_resolved(
        trimmed, 0, provenance, resolved,
    );

    clay_state.set_active_design_system(active.clone());
    clay_state.set_explicit_design_system_active(true);
    Ok(active)
}

/// `setDesignSystem` Clay JS op (Plan 102). Resolves `specifier` against package
/// records (`clay.contributions.uiDesignSystem`), validates bounds and color authority,
/// computes complete fallback-resolved component recipes, and installs the active design system.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_design_system(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|_| {
        JsErrorBox::generic("theme.invalid_request: setDesignSystem requires { specifier: string }")
    })?;
    let Some(specifier) = request.get("specifier").and_then(Value::as_str) else {
        return Err(JsErrorBox::generic(
            "theme.invalid_request: setDesignSystem requires a `specifier` string",
        ));
    };
    let clay_state = state.borrow::<std::sync::Arc<ClayOpState>>();
    let active = apply_design_system(clay_state, specifier)?;
    let recipe_count = active.recipes.len();
    serde_json::to_string(&json!({
        "specifier": specifier,
        "recipeCount": recipe_count,
        "schemaVersion": active.schema_version,
    }))
    .map_err(|_| JsErrorBox::generic("theme.invalid_request: serialization failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Appearance, ResolvedAppearance};

    #[test]
    fn canonical_mapping_covers_light_and_dark() {
        assert_eq!(
            canonical_default_specifier(ResolvedAppearance::Light),
            "@clay/theme-modus-operandi"
        );
        assert_eq!(
            canonical_default_specifier(ResolvedAppearance::Dark),
            "@clay/theme-modus-vivendi"
        );
    }

    #[test]
    fn appearance_system_falls_back_to_dark_without_os_signal() {
        // No OS signal (os_dark = true as the no-signal fallback): System → dark.
        assert_eq!(Appearance::System.resolve(true), ResolvedAppearance::Dark);
        assert_eq!(Appearance::System.resolve(false), ResolvedAppearance::Light);
        assert_eq!(Appearance::Light.resolve(true), ResolvedAppearance::Light);
        assert_eq!(Appearance::Dark.resolve(false), ResolvedAppearance::Dark);
    }

    #[test]
    fn appearance_parse_rejects_unknown_values() {
        assert_eq!(Appearance::parse("light"), Some(Appearance::Light));
        assert_eq!(Appearance::parse("dark"), Some(Appearance::Dark));
        assert_eq!(Appearance::parse("system"), Some(Appearance::System));
        assert_eq!(Appearance::parse("auto"), None);
        assert_eq!(Appearance::parse(""), None);
    }

    /// Phase 20.7 task 3: a valid (core-palette) snapshot passes the contrast
    /// floor and emits no diagnostic record.
    #[test]
    fn enforce_contrast_accepts_core_palette() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        let snapshot = ActiveTheme {
            specifier: "@clay/core".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        };
        enforce_contrast(&clay_state, "@clay/core", &snapshot).expect("core palette meets AA");
        assert!(
            clay_state.records().is_empty(),
            "no diagnostic on passing theme"
        );
    }

    /// Phase 20.7 task 3: a low-contrast snapshot is rejected, the diagnostic
    /// record names the specifier/pair/ratio/threshold, and the previously
    /// installed active theme is left untouched (no mutation on rejection).
    #[test]
    fn enforce_contrast_rejects_low_contrast_without_mutating_active_theme() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        // Install a valid active theme first.
        let valid = ActiveTheme {
            specifier: "@clay/core".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        };
        clay_state.set_active_theme(valid.clone());
        assert_eq!(clay_state.active_theme().unwrap().specifier, "@clay/core");

        // text.primary overridden to match surface.main core (#100f17): ratio 1.0.
        let low_contrast = ActiveTheme {
            specifier: "@clay/theme-low-contrast".to_string(),
            overrides: Vec::new(),
            design_tokens: vec![crate::protocol::UiDesignTokenOverride {
                token: "text.primary".to_string(),
                value: crate::protocol::WireDesignTokenValue::Color([0x10, 0x0f, 0x17, 0xff]),
                provenance: "theme-low-contrast".to_string(),
            }],
        };
        let err = enforce_contrast(&clay_state, "@clay/theme-low-contrast", &low_contrast)
            .expect_err("low-contrast snapshot must be rejected");
        let message = err.to_string();
        assert!(message.contains("theme.contrast"), "message: {message}");
        assert!(
            message.contains("@clay/theme-low-contrast"),
            "message: {message}"
        );
        assert!(message.contains("text.primary"), "message: {message}");
        assert!(message.contains("surface.main"), "message: {message}");
        assert!(message.contains("4.5"), "message: {message}");

        // Diagnostic record carries the same naming.
        let records = clay_state.records();
        assert_eq!(records.len(), 1, "exactly one contrast diagnostic recorded");
        assert!(
            records[0].contains("text.primary"),
            "record: {}",
            records[0]
        );
        assert!(
            records[0].contains("surface.main"),
            "record: {}",
            records[0]
        );

        // Active theme is unchanged: the rejected snapshot was never installed.
        let still_active = clay_state
            .active_theme()
            .expect("prior valid theme remains");
        assert_eq!(still_active.specifier, "@clay/core");
        assert!(still_active.design_tokens.is_empty());
    }

    #[test]
    fn apply_design_system_core_baseline() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        assert!(clay_state.active_design_system().is_none());
        assert!(!clay_state.explicit_design_system_active());

        let active = apply_design_system(&clay_state, "@clay/core").expect("apply core");
        assert_eq!(active.specifier, "@clay/core");
        assert_eq!(active.provenance.package_name, "core");
        assert_eq!(
            active.provenance.trust_domain,
            crate::protocol::PackageUiTrustDomain::Trusted
        );
        assert!(!active.recipes.is_empty());
        assert!(clay_state.explicit_design_system_active());
        assert_eq!(
            clay_state.active_design_system().unwrap().specifier,
            "@clay/core"
        );

        let active2 = apply_design_system(&clay_state, "core").expect("apply core alias");
        assert_eq!(active2.specifier, "@clay/core");
    }

    #[test]
    fn apply_design_system_rejects_empty_specifier() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        let err = apply_design_system(&clay_state, "   ").expect_err("rejects empty");
        assert!(err.to_string().contains("theme.invalid_request"));
        assert!(clay_state.active_design_system().is_none());
        assert!(!clay_state.explicit_design_system_active());
    }

    #[test]
    fn apply_design_system_rejects_missing_package_and_preserves_state() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        let initial = apply_design_system(&clay_state, "@clay/core").unwrap();
        assert_eq!(initial.specifier, "@clay/core");

        let err = apply_design_system(&clay_state, "@vendor/non-existent-ds")
            .expect_err("rejects non-existent package");
        assert!(err.to_string().contains("theme.load_failed"));

        // Prior active design system is preserved untouched
        let active = clay_state.active_design_system().unwrap();
        assert_eq!(active.specifier, "@clay/core");
        assert_eq!(active.provenance.package_name, "core");
    }

    #[test]
    fn apply_design_system_with_adopted_package_and_inheritance() {
        let clay_state = std::sync::Arc::new(ClayOpState::default());
        let package_json = serde_json::json!({
            "name": "@vendor/custom-nord-ds",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "nord",
                "entry": "./dist/index.js",
                "permissions": [],
                "modes": ["nord"],
                "docs": "./docs/index.md",
                "contributions": {
                    "uiDesignSystem": {
                        "schemaVersion": 1,
                        "id": "@vendor/custom-nord-ds",
                        "displayName": "Nord Design System",
                        "extends": "@clay/core",
                        "values": {
                            "radii.button": {
                                "type": "radius",
                                "value": 6.0
                            }
                        },
                        "recipes": {
                            "button.primary.root.rest": {
                                "borderRadius": 6.0,
                                "backgroundColor": "accent.primary",
                                "textColor": "surface.main"
                            }
                        }
                    }
                }
            }
        });

        let fixture_dir = std::env::temp_dir().join(format!(
            "clay-test-ds-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(fixture_dir.join("dist")).unwrap();
        std::fs::create_dir_all(fixture_dir.join("docs")).unwrap();
        std::fs::write(fixture_dir.join("dist/index.js"), "// noop").unwrap();
        std::fs::write(fixture_dir.join("docs/index.md"), "# Nord").unwrap();
        {
            let mut service = clay_state.package_service().lock().unwrap();
            service
                .install_from_value_at_root_with_spec(
                    package_json,
                    fixture_dir.clone(),
                    "local:nord-ds",
                )
                .unwrap();
            service
                .authorize_package(
                    "@vendor/custom-nord-ds",
                    vec![],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .unwrap();
            service
                .approve_package("@vendor/custom-nord-ds", "cli")
                .unwrap();
        }

        let active = apply_design_system(&clay_state, "@vendor/custom-nord-ds")
            .expect("adopted third-party design system applies");
        assert_eq!(active.specifier, "@vendor/custom-nord-ds");
        assert_eq!(active.provenance.package_name, "@vendor/custom-nord-ds");
        assert_eq!(active.provenance.api_prefix, "nord");
        assert_eq!(
            active.provenance.trust_domain,
            crate::protocol::PackageUiTrustDomain::ThirdParty
        );

        // Check that overridden recipe has the custom border radius
        let btn_key = crate::shell::design_system::RecipeKey::new(
            "button",
            "primary",
            "root",
            crate::shell::design_system::RecipeState::Rest,
        );
        let btn_recipe = active.recipes.get(&btn_key).expect("button recipe present");
        assert_eq!(btn_recipe.border_radius, 6.0);

        // Check that untouched recipes inherit from core baseline
        let input_key = crate::shell::design_system::RecipeKey::new(
            "textInput",
            "default",
            "root",
            crate::shell::design_system::RecipeState::Rest,
        );
        let input_recipe = active
            .recipes
            .get(&input_key)
            .expect("input recipe inherited from core");
        assert_eq!(input_recipe.border_width, 1.0);

        let _ = std::fs::remove_dir_all(fixture_dir);
    }
}
