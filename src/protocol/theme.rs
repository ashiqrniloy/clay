//! Active theme snapshot and wire design-token overrides.

use crate::str_enum::string_enum_impl;

use super::*;

/// Typed override value for a UI design token. The variant present must match
/// the core token's type (validated before install). Levels travel as
/// validated names so the protocol stays independent of shell-side level enums.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum WireDesignTokenValue {
    /// `color-role` override as RGBA bytes.
    Color([u8; 4]),
    /// `spacing`/`radius`/`dimension`/`motion-duration` override as a finite,
    /// non-negative, bounded scalar.
    Scalar(f64),
    /// `opacity` override as a finite `[0, 1]` scalar.
    Opacity(f32),
    /// `elevation`/`z-level`/`density` override as a validated level name.
    Level(String),
}

/// Bounded inert typed UI design-token override shipped within [`ActiveTheme`].
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct UiDesignTokenOverride {
    /// Core Clay token name being overridden (e.g. `surface.hover`).
    pub token: String,
    /// Typed override value; the variant must match the core token's type.
    pub value: WireDesignTokenValue,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// Resolved active theme snapshot shipped from the server (which owns package
/// records) to the client (which owns the editor `StyleRegistry`). Sent once
/// during the welcome handshake when `setTheme("...")` ran in `init.js`; the
/// client reconstructs and installs the registry before/at startup paint.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTheme {
    /// Selected package specifier (e.g. `@clay/theme-gruvbox-material-dark`).
    pub specifier: String,
    /// Inert text-style + base-UI-color overrides for the selected theme.
    pub overrides: Vec<TextThemeOverride>,
    /// Phase 20.1: bounded inert typed UI design-token overrides declared by the
    /// theme package via `clay.contributions.designTokens`. Clay validates each
    /// token name, value type, and bounds against the core fallback catalog
    /// before install. Themes that omit the contribution ship an empty vector
    /// and resolve every UI value from core fallbacks unchanged. Pure data: no
    /// CSS, callbacks, JS execution, or native handles.
    pub design_tokens: Vec<UiDesignTokenOverride>,
}

/// Bounded user appearance preference (Phase 20.6). Selects the canonical
/// default theme only when the user has not explicitly called `setTheme`.
/// `System` follows the observable OS color-scheme signal; when no signal is
/// available it resolves to dark (Modus Vivendi). An explicit `setTheme`
/// specifier always wins over appearance-derived selection.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    System,
}

string_enum_impl! {
    /// Parse a bounded appearance value from its JSON string form. Unknown
    /// values are rejected so a future field never silently round-trips.
    /// Lowercase JSON string form used on the wire and in config.
    pub Appearance {
        Light => "light",
        Dark => "dark",
        System => "system",
    }
}

impl Appearance {
    /// Resolve `System` to a concrete light/dark choice given the observed OS
    /// color-scheme signal. `os_dark = true` means the OS reports a dark
    /// preference; `false` means light or unobservable. `System` with no signal
    /// falls back to dark per the Phase 20.6 pinned semantics.
    pub fn resolve(self, os_dark: bool) -> ResolvedAppearance {
        match self {
            Self::Light => ResolvedAppearance::Light,
            Self::Dark => ResolvedAppearance::Dark,
            Self::System => {
                if os_dark {
                    ResolvedAppearance::Dark
                } else {
                    ResolvedAppearance::Light
                }
            }
        }
    }
}

/// Concrete light/dark choice after resolving `Appearance::System`.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ResolvedAppearance {
    Light,
    Dark,
}
