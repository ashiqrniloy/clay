//! Serde boundary types for the bridge.
//!
//! The protocol types themselves are serde-derivable (see `src/protocol`),
//! so the bridge only defines the *envelope* shapes the webview deals with:
//! the bootstrap snapshot and the event envelope. IDs that can exceed
//! JavaScript's safe-integer range (menu session ids, `1 << 63` partition)
//! cross as strings; counter-based ids stay numbers by construction.

use clay::client::{ClientConnectionEvent, ClientInitialState};
use clay::editor::theme::{StyleRegistry, color_hex};
use clay::protocol::{
    ActiveDesignSystem, ActiveTypography, BehaviorManifest, ClientId, DecorationKind,
    DocumentAccess, DocumentId, DocumentVersion, EmptyTabContent, Modifiers,
    PackageComponentContent, PackageOverlayContent, PackagePanelContent, PackageUiProvenance,
    PackageUiSnapshot, RuntimeDiagnostic, RuntimeStateSnapshot, SduiTree, TabId, TokenType,
};
use clay::shell::design_system::{
    DesignSystemProvenance, InnerHighlight, ResolvedComponentRecipe, ShadowLayer, ThemeColorRef,
};
use clay::shell::icons::{ActiveIconPack, IconGeometry, IconPath};
use clay::shell::theme::{ThemeTokenValueDto, density_spacing_scale, resolve_theme_token_snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Complete session state installed on connect/reconnect. One atomic
/// projection: the webview replaces its previous bootstrap wholesale
/// (reconnect must never merge across sessions).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct BootstrapDto {
    pub client_id: ClientId,
    /// Filled once the server registry binds this connection; `None` until then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<TabId>,
    pub protocol_version: u32,
    pub endpoint: String,
    /// Session generation; increments on every (re)connect. Events from an
    /// older generation are structurally impossible after reconnect (the old
    /// pump is aborted before the new handshake starts), but the number lets
    /// the frontend discard in-flight UI work keyed to a dead session.
    pub generation: u64,
    /// Developer-only switch; it is inherited from `--profile-perf` and never
    /// comes from package or webview input.
    pub performance_profile: bool,
    pub initial_document: InitialDocumentDto,
    pub behavior_manifest: BehaviorManifest,
    /// Fully resolved core-token surface (overrides + legacy base palette
    /// layered over core fallbacks, contrast-validated). The webview never
    /// sees raw override data or performs resolution.
    pub active_theme: ThemeSnapshotDto,
    pub active_typography: TypographySnapshotDto,
    pub active_design_system: DesignSystemSnapshotDto,
    /// Resolved active icon pack at bootstrap; `None` = host fallback subset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_icon_pack: Option<IconPackSnapshotDto>,
}

/// Resolved theme projection consumed by the frontend theme adapter.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct ThemeSnapshotDto {
    pub specifier: String,
    /// Core token name → resolved typed value (e.g. `surface.main` → color).
    pub tokens: BTreeMap<String, ThemeTokenValueDto>,
    /// Closed editor vocabulary resolved by Rust. React never interprets raw
    /// theme-package overrides or invents syntax colors.
    pub editor_styles: BTreeMap<String, EditorStyleDto>,
    /// Spacing rhythm multiplier from the resolved density level
    /// (`0.875`/`1.0`/`1.125`); the adapter pre-scales `spacing.*` with it.
    pub density_scale: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct EditorStyleDto {
    pub color: String,
    pub background: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub scale: f32,
}

fn editor_style_snapshot(theme: &clay::protocol::ActiveTheme) -> BTreeMap<String, EditorStyleDto> {
    use TokenType::*;
    const TYPES: [(&str, TokenType); 35] = [
        ("namespace", Namespace),
        ("type", Type),
        ("class", Class),
        ("enum", Enum),
        ("interface", Interface),
        ("struct", Struct),
        ("typeParameter", TypeParameter),
        ("parameter", Parameter),
        ("variable", Variable),
        ("property", Property),
        ("enumMember", EnumMember),
        ("event", Event),
        ("function", Function),
        ("method", Method),
        ("macro", Macro),
        ("keyword", Keyword),
        ("modifier", Modifier),
        ("comment", Comment),
        ("string", String),
        ("number", Number),
        ("regexp", Regexp),
        ("operator", Operator),
        ("decorator", Decorator),
        ("heading1", Heading1),
        ("heading2", Heading2),
        ("heading3", Heading3),
        ("heading4", Heading4),
        ("heading5", Heading5),
        ("heading6", Heading6),
        ("listItem", ListItem),
        ("quote", Quote),
        ("codeBlock", CodeBlock),
        ("codeSpan", CodeSpan),
        ("link", Link),
        ("paragraph", Paragraph),
    ];
    let registry = StyleRegistry::from_active_theme(theme);
    let dto = |kind, token| {
        let style = registry.style_for(kind, token, Modifiers::NONE);
        EditorStyleDto {
            color: color_hex(style.color),
            background: style.background.map(color_hex),
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            strike: style.strike,
            scale: style.scale,
        }
    };
    let mut styles: BTreeMap<std::string::String, EditorStyleDto> = TYPES
        .into_iter()
        .map(|(name, token)| (name.to_string(), dto(DecorationKind::Syntax, token)))
        .collect();
    styles.insert(
        "searchMatch".to_string(),
        dto(DecorationKind::SearchMatch, TokenType::Variable),
    );
    styles.insert(
        "inlayHint".to_string(),
        dto(DecorationKind::InlayHint, TokenType::Type),
    );
    styles
}

impl ThemeSnapshotDto {
    /// Resolve one snapshot through the Rust authority. Contrast validation
    /// runs first; a below-AA theme is rejected before it reaches the DOM.
    pub fn resolve(specifier: &str, theme: &clay::protocol::ActiveTheme) -> Result<Self, String> {
        let tokens = resolve_theme_token_snapshot(theme)
            .map_err(|failure| format!("theme rejected: {failure:?}"))?;
        let density_scale = density_spacing_scale(&tokens);
        Ok(Self {
            specifier: specifier.to_string(),
            tokens,
            editor_styles: editor_style_snapshot(theme),
            density_scale,
        })
    }
}

/// Frontend-facing typography projection: user-owned profiles plus hierarchy
/// scales; the adapter computes variant sizes once per install.
///
/// Newtype projection of [`ActiveTypography`] (plan 132 U1): the bridge owns no
/// typography field of its own, so the contract cannot drift from the source,
/// and serde's newtype rule keeps the wire JSON identical to the source's.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct TypographySnapshotDto(pub ActiveTypography);

impl From<&ActiveTypography> for TypographySnapshotDto {
    fn from(active: &ActiveTypography) -> Self {
        Self(active.clone())
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct InitialDocumentDto {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub head: clay::protocol::DocumentTextHead,
    pub access: DocumentAccess,
    pub workspace_root: String,
}

impl InitialDocumentDto {
    pub(crate) fn from_initial_state(state: &ClientInitialState) -> Self {
        // Exhaustive destructure (no `..`): a field added to, renamed in, or
        // removed from `ClientInitialState` breaks the build *here*, where the
        // webview projection lives, instead of silently never crossing.
        //
        // The four ignored fields are carried elsewhere in the bootstrap
        // (`client_id`, `behavior_manifest`, `active_theme`,
        // `active_typography`), so they are deliberately not part of this DTO.
        let ClientInitialState {
            client_id: _,
            document_id,
            document_version,
            head,
            access,
            behavior_manifest: _,
            active_theme: _,
            active_typography: _,
            workspace_root,
        } = state;
        Self {
            document_id: *document_id,
            version: *document_version,
            head: head.clone(),
            access: access.clone(),
            workspace_root: workspace_root.clone(),
        }
    }
}

/// Resolved UI design system projection consumed by the frontend design-system adapter.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct DesignSystemSnapshotDto {
    pub specifier: String,
    pub schema_version: u32,
    pub generation: u64,
    pub provenance: DesignSystemProvenance,
    pub recipes: BTreeMap<String, ComponentRecipeDto>,
    pub variables: BTreeMap<String, DesignSystemVariableValueDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct ComponentRecipeDto {
    pub background_color: String,
    pub background_opacity: f64,
    pub text_color: String,
    pub border_color: String,
    pub border_width: f64,
    pub border_style: String,
    pub border_radius: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shadow: Vec<ShadowLayerDto>,
    pub backdrop_blur: f64,
    pub backdrop_saturate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_highlight: Option<InnerHighlightDto>,
    pub opacity: f64,
    pub outline_color: String,
    pub outline_width: f64,
    pub outline_offset: f64,
    pub outline_style: String,
    pub transition_duration: f64,
    pub transition_timing: String,
    pub transform_preset: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct ShadowLayerDto {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub spread: f64,
    pub color: String,
    pub opacity: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct InnerHighlightDto {
    pub color: String,
    pub opacity: f64,
    pub width: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum DesignSystemVariableValueDto {
    ThemeColorRole(String),
    Dimension(f64),
    Radius(f64),
    BorderWidth(f64),
    BorderStyle(String),
    SpacingToken(String),
    Opacity(f64),
    BackdropBlur(f64),
    BackdropSaturate(f64),
    Shadow(Vec<ShadowLayerDto>),
    InnerHighlight(InnerHighlightDto),
    OutlineStyle(String),
    MotionDuration(f64),
    TransitionTiming(String),
    TransformPreset(String),
}

impl From<&ShadowLayer> for ShadowLayerDto {
    fn from(layer: &ShadowLayer) -> Self {
        // Exhaustive destructure (no `..`): a field added to, renamed in, or
        // removed from `ShadowLayer` breaks the build here. `inset` is the one
        // deliberate narrowing — the webview has no inset-shadow rendering, so
        // it never crosses.
        let ShadowLayer {
            x,
            y,
            blur,
            spread,
            color_role,
            opacity,
            inset: _,
        } = layer;
        Self {
            x: *x,
            y: *y,
            blur: *blur,
            spread: *spread,
            color: color_role.0.clone(),
            opacity: *opacity,
        }
    }
}

impl From<&InnerHighlight> for InnerHighlightDto {
    fn from(highlight: &InnerHighlight) -> Self {
        // Exhaustive destructure, as above.
        let InnerHighlight {
            color_role,
            opacity,
            width,
        } = highlight;
        Self {
            color: color_role.0.clone(),
            opacity: *opacity,
            width: *width,
        }
    }
}

impl From<&ResolvedComponentRecipe> for ComponentRecipeDto {
    fn from(recipe: &ResolvedComponentRecipe) -> Self {
        // The single definition of the recipe contract: exhaustive destructure
        // (no `..`) pins all 21 fields to the resolved recipe, so a new recipe
        // field cannot silently miss the webview. Color-role unwrapping and
        // enum → string conversion happen once, here — the variables table and
        // every other consumer read this DTO instead of re-deriving them.
        let ResolvedComponentRecipe {
            background_color,
            background_opacity,
            text_color,
            border_color,
            border_width,
            border_style,
            border_radius,
            padding,
            gap,
            shadow,
            backdrop_blur,
            backdrop_saturate,
            inner_highlight,
            opacity,
            outline_color,
            outline_width,
            outline_offset,
            outline_style,
            transition_duration,
            transition_timing,
            transform_preset,
        } = recipe;
        Self {
            background_color: background_color.0.clone(),
            background_opacity: *background_opacity,
            text_color: text_color.0.clone(),
            border_color: border_color.0.clone(),
            border_width: *border_width,
            border_style: border_style.as_str().to_string(),
            border_radius: *border_radius,
            padding: padding.clone(),
            gap: gap.clone(),
            shadow: shadow.iter().map(ShadowLayerDto::from).collect(),
            backdrop_blur: *backdrop_blur,
            backdrop_saturate: *backdrop_saturate,
            inner_highlight: inner_highlight.as_ref().map(InnerHighlightDto::from),
            opacity: *opacity,
            outline_color: outline_color.0.clone(),
            outline_width: *outline_width,
            outline_offset: *outline_offset,
            outline_style: outline_style.as_str().to_string(),
            transition_duration: *transition_duration,
            transition_timing: transition_timing.as_str().to_string(),
            transform_preset: transform_preset.as_str().to_string(),
        }
    }
}

impl DesignSystemSnapshotDto {
    pub fn resolve(active: &ActiveDesignSystem) -> Result<Self, String> {
        active
            .validate()
            .map_err(|err| format!("design system validation failed: {err}"))?;

        let mut recipes = BTreeMap::new();
        let mut variables = BTreeMap::new();

        for (key, recipe) in &active.recipes {
            let key_str = key.to_key_string();

            // Strict color denial check: reject concrete literal colors and invalid roles
            for (field, color) in [
                ("background_color", &recipe.background_color),
                ("text_color", &recipe.text_color),
                ("border_color", &recipe.border_color),
                ("outline_color", &recipe.outline_color),
            ] {
                if !ThemeColorRef::is_valid_color_role(&color.0) {
                    return Err(format!(
                        "color denial: invalid {field} `{}` in recipe `{key_str}`",
                        color.0
                    ));
                }
            }
            for layer in &recipe.shadow {
                if !ThemeColorRef::is_valid_color_role(&layer.color_role.0) {
                    return Err(format!(
                        "color denial: invalid shadow color `{}` in recipe `{key_str}`",
                        layer.color_role.0
                    ));
                }
            }
            if let Some(highlight) = &recipe.inner_highlight
                && !ThemeColorRef::is_valid_color_role(&highlight.color_role.0)
            {
                return Err(format!(
                    "color denial: invalid inner_highlight color `{}` in recipe `{key_str}`",
                    highlight.color_role.0
                ));
            }

            // Single projection site for the recipe contract; the variables
            // table reads the projected DTO, never the source recipe again.
            let recipe_dto = ComponentRecipeDto::from(recipe);
            variables.extend(design_system_variables(&key_str, &recipe_dto));
            recipes.insert(key_str, recipe_dto);
        }

        Ok(Self {
            specifier: active.specifier.clone(),
            schema_version: active.schema_version,
            generation: active.generation,
            provenance: active.provenance.clone(),
            recipes,
            variables,
        })
    }
}

/// Flat `recipe.field` variables table the frontend's CSS custom properties
/// read (plan 110). Projected from the DTO, so color-role unwrapping and
/// enum → string conversion happen once in `ComponentRecipeDto::from`.
fn design_system_variables(
    key: &str,
    recipe: &ComponentRecipeDto,
) -> Vec<(String, DesignSystemVariableValueDto)> {
    use DesignSystemVariableValueDto as Value;
    let mut variables: Vec<(String, DesignSystemVariableValueDto)> = Vec::new();
    let mut push = |field: &str, value: Value| variables.push((format!("{key}.{field}"), value));
    push(
        "backgroundColor",
        Value::ThemeColorRole(recipe.background_color.clone()),
    );
    push(
        "backgroundOpacity",
        Value::Opacity(recipe.background_opacity),
    );
    push(
        "textColor",
        Value::ThemeColorRole(recipe.text_color.clone()),
    );
    push(
        "borderColor",
        Value::ThemeColorRole(recipe.border_color.clone()),
    );
    push("borderWidth", Value::BorderWidth(recipe.border_width));
    push(
        "borderStyle",
        Value::BorderStyle(recipe.border_style.clone()),
    );
    push("borderRadius", Value::Radius(recipe.border_radius));
    if let Some(padding) = &recipe.padding {
        push("padding", Value::SpacingToken(padding.clone()));
    }
    if let Some(gap) = &recipe.gap {
        push("gap", Value::SpacingToken(gap.clone()));
    }
    if !recipe.shadow.is_empty() {
        push("shadow", Value::Shadow(recipe.shadow.clone()));
    }
    if recipe.backdrop_blur > 0.0 {
        push("backdropBlur", Value::BackdropBlur(recipe.backdrop_blur));
    }
    if (recipe.backdrop_saturate - 1.0).abs() > f64::EPSILON {
        push(
            "backdropSaturate",
            Value::BackdropSaturate(recipe.backdrop_saturate),
        );
    }
    if let Some(highlight) = &recipe.inner_highlight {
        push("innerHighlight", Value::InnerHighlight(highlight.clone()));
    }
    if (recipe.opacity - 1.0).abs() > f64::EPSILON {
        push("opacity", Value::Opacity(recipe.opacity));
    }
    push(
        "outlineColor",
        Value::ThemeColorRole(recipe.outline_color.clone()),
    );
    push("outlineWidth", Value::Dimension(recipe.outline_width));
    push("outlineOffset", Value::Dimension(recipe.outline_offset));
    push(
        "outlineStyle",
        Value::OutlineStyle(recipe.outline_style.clone()),
    );
    push(
        "transitionDuration",
        Value::MotionDuration(recipe.transition_duration),
    );
    push(
        "transitionTiming",
        Value::TransitionTiming(recipe.transition_timing.clone()),
    );
    push(
        "transformPreset",
        Value::TransformPreset(recipe.transform_preset.clone()),
    );
    variables
}

/// Resolved active icon-pack projection consumed by the frontend icon store
/// (Plan 112 task 6). Geometry is the canonical bounded d-string contract
/// shape; the webview never sees parsed command internals.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct IconPackSnapshotDto {
    pub specifier: String,
    pub schema_version: u32,
    pub generation: u64,
    pub provenance: DesignSystemProvenance,
    pub icons: BTreeMap<String, IconGeometryDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct IconGeometryDto {
    pub view_box: [f64; 4],
    pub paths: Vec<IconPathDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct IconPathDto {
    pub d: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

impl From<&IconPath> for IconPathDto {
    fn from(path: &IconPath) -> Self {
        // Exhaustive destructure (no `..`): `commands` is consumed by `to_d()`,
        // so the bounded d-string stays the only geometry that crosses.
        let IconPath {
            commands: _,
            opacity,
        } = path;
        Self {
            d: path.to_d(),
            opacity: opacity.map(f64::from),
        }
    }
}

impl From<&IconGeometry> for IconGeometryDto {
    fn from(geometry: &IconGeometry) -> Self {
        // Exhaustive destructure, as above.
        let IconGeometry { view_box, paths } = geometry;
        Self {
            view_box: view_box.map(f64::from),
            paths: paths.iter().map(IconPathDto::from).collect(),
        }
    }
}

impl IconPackSnapshotDto {
    /// Validate before publication and project defensively; an invalid pack
    /// snapshot is rejected (never silently truncated) so the webview keeps
    /// its last authorized state.
    pub fn resolve(active: &ActiveIconPack) -> Result<Self, String> {
        active
            .validate()
            .map_err(|error| format!("icon pack validation failed: {error}"))?;
        Ok(Self {
            specifier: active.specifier.clone(),
            schema_version: active.schema_version,
            generation: active.generation,
            provenance: active.provenance.clone(),
            icons: active
                .icons
                .iter()
                .map(|(key, geometry)| (key.clone(), IconGeometryDto::from(geometry)))
                .collect(),
        })
    }
}

/// Safe atomic runtime-generation projection. Raw theme overrides and JSON
/// component strings are resolved/parsed in Rust before the webview observes it.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct RuntimeSnapshotDto {
    pub runtime_generation_id: u64,
    pub behavior_manifest: BehaviorManifest,
    pub active_theme: ThemeSnapshotDto,
    pub active_typography: TypographySnapshotDto,
    pub active_design_system: DesignSystemSnapshotDto,
    /// Resolved active icon pack; `None` = host fallback subset active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_icon_pack: Option<IconPackSnapshotDto>,
    /// Server-enumerated Settings selections (plan 110 task 10); passed
    /// through untouched — the bridge owns no package inventory.
    pub ui_choices: clay::protocol::UiChoicesSnapshot,
    pub sdui_tree: SduiTree,
    pub package_ui: PackageUiSnapshotDto,
    pub documents: Vec<clay::protocol::DocumentRuntimeRenderState>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct PackageUiSnapshotDto {
    pub version: u64,
    pub empty_tab: Option<PackageSurfaceDto>,
    /// Named pane surfaces (`activation: "pane"`), e.g. the Coding Agent
    /// split surface. Dropped before plan 108 — the frontend never saw them.
    #[serde(default)]
    pub surfaces: Vec<PackageSurfaceDto>,
    pub panels: Vec<PackagePanelDto>,
    pub overlays: Vec<PackageOverlayDto>,
    pub components: Vec<PackageSurfaceDto>,
    pub input_routes: Vec<clay::protocol::PackageInputRouteContent>,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct PackageSurfaceDto {
    pub id: String,
    pub component: serde_json::Value,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct PackagePanelDto {
    pub id: String,
    pub slot: String,
    pub visibility: String,
    pub component: serde_json::Value,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct PackageOverlayDto {
    pub id: String,
    pub anchor: String,
    pub focus_policy: String,
    pub dismissal_policy: String,
    pub component: serde_json::Value,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

impl RuntimeSnapshotDto {
    pub fn resolve(snapshot: RuntimeStateSnapshot) -> Result<Self, String> {
        snapshot
            .validate()
            .map_err(|error| format!("invalid runtime snapshot: {error:?}"))?;
        Ok(Self {
            runtime_generation_id: snapshot.runtime_generation_id,
            behavior_manifest: snapshot.behavior,
            active_theme: ThemeSnapshotDto::resolve(
                &snapshot.active_theme.specifier,
                &snapshot.active_theme,
            )?,
            active_typography: TypographySnapshotDto::from(&snapshot.active_typography),
            active_design_system: DesignSystemSnapshotDto::resolve(&snapshot.active_design_system)?,
            active_icon_pack: snapshot
                .active_icon_pack
                .as_ref()
                .map(IconPackSnapshotDto::resolve)
                .transpose()?,
            ui_choices: snapshot.ui_choices,
            sdui_tree: snapshot.sdui_tree,
            package_ui: PackageUiSnapshotDto::parse(snapshot.package_ui)?,
            documents: snapshot.documents,
            diagnostics: snapshot.diagnostics,
        })
    }
}

/// Parses one package-supplied component tree. The webview never receives the
/// JSON string; an invalid component fails the whole projection (never a
/// partially parsed UI).
fn parse_component(component_json: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(component_json).map_err(|_| "invalid package component".to_string())
}

impl PackageSurfaceDto {
    /// The one surface projection: empty-tab landings and named component
    /// surfaces share it, so the two source shapes cannot drift apart.
    fn parse(
        id: &str,
        component_json: &str,
        action_targets: &[String],
        provenance: &PackageUiProvenance,
    ) -> Result<Self, String> {
        Ok(Self {
            id: id.to_string(),
            component: parse_component(component_json)?,
            action_targets: action_targets.to_vec(),
            provenance: provenance.clone(),
        })
    }
}

impl TryFrom<&EmptyTabContent> for PackageSurfaceDto {
    type Error = String;

    fn try_from(entry: &EmptyTabContent) -> Result<Self, String> {
        // Exhaustive destructure (no `..`): a field added to, renamed in, or
        // removed from the source breaks the build here. `package_name` is the
        // deliberate narrowing — provenance already carries identity.
        let EmptyTabContent {
            id,
            package_name: _,
            component_json,
            action_targets,
            provenance,
        } = entry;
        Self::parse(id, component_json, action_targets, provenance)
    }
}

impl TryFrom<&PackageComponentContent> for PackageSurfaceDto {
    type Error = String;

    fn try_from(entry: &PackageComponentContent) -> Result<Self, String> {
        // Exhaustive destructure, as above.
        let PackageComponentContent {
            id,
            component_json,
            action_targets,
            provenance,
        } = entry;
        Self::parse(id, component_json, action_targets, provenance)
    }
}

impl TryFrom<&PackagePanelContent> for PackagePanelDto {
    type Error = String;

    fn try_from(entry: &PackagePanelContent) -> Result<Self, String> {
        // Exhaustive destructure, as above.
        let PackagePanelContent {
            id,
            slot,
            visibility,
            component_json,
            action_targets,
            provenance,
        } = entry;
        Ok(Self {
            id: id.clone(),
            slot: slot.clone(),
            visibility: visibility.clone(),
            component: parse_component(component_json)?,
            action_targets: action_targets.clone(),
            provenance: provenance.clone(),
        })
    }
}

impl TryFrom<&PackageOverlayContent> for PackageOverlayDto {
    type Error = String;

    fn try_from(entry: &PackageOverlayContent) -> Result<Self, String> {
        // Exhaustive destructure, as above.
        let PackageOverlayContent {
            id,
            anchor,
            focus_policy,
            dismissal_policy,
            component_json,
            action_targets,
            provenance,
        } = entry;
        Ok(Self {
            id: id.clone(),
            anchor: anchor.clone(),
            focus_policy: focus_policy.clone(),
            dismissal_policy: dismissal_policy.clone(),
            component: parse_component(component_json)?,
            action_targets: action_targets.clone(),
            provenance: provenance.clone(),
        })
    }
}

impl PackageUiSnapshotDto {
    fn parse(snapshot: PackageUiSnapshot) -> Result<Self, String> {
        Ok(Self {
            version: snapshot.version,
            empty_tab: snapshot
                .empty_tab
                .as_ref()
                .map(PackageSurfaceDto::try_from)
                .transpose()?,
            surfaces: snapshot
                .surfaces
                .iter()
                .map(PackageSurfaceDto::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            panels: snapshot
                .panels
                .iter()
                .map(PackagePanelDto::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            overlays: snapshot
                .overlays
                .iter()
                .map(PackageOverlayDto::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            components: snapshot
                .components
                .iter()
                .map(PackageSurfaceDto::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            input_routes: snapshot.input_routes,
        })
    }
}

/// Everything the webview can observe over its subscription channel.
///
/// `event` carries the client layer's validated connection events 1:1
/// (staleness/malformed/unauthorized payloads were dropped before here);
/// `themeSnapshot` is the bridge's resolved projection of server theme
/// changes; the remaining variants are bridge-owned lifecycle notices.
#[derive(Clone, Serialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum BridgeEnvelope {
    /// Boxed: the client event union is large and this enum is moved often.
    ///
    /// `ts(skip)`: the client event union is *not* generated. Generating it
    /// would pull the internal event graph — server messages, agent frames,
    /// viewport patches, completion/diagnostic sets — into the webview
    /// contract's type surface; the shell deliberately narrows it to the
    /// families it consumes (`ShellEvent` in `frontend/src/bridge/types.ts`),
    /// which is a narrowing, not a copy (plan 119 SC-1 decision log).
    #[cfg_attr(feature = "ts-bindings", ts(skip))]
    Event(Box<ClientConnectionEvent>),
    /// Multi-tab event: same payload as `Event`, tagged with the owning client.
    #[cfg_attr(feature = "ts-bindings", ts(skip))]
    Routed {
        client_id: ClientId,
        tab_id: Option<TabId>,
        event: Box<ClientConnectionEvent>,
    },
    /// Rust-resolved replacement for `ClientConnectionEvent::ActiveTheme`:
    /// raw overrides never cross to the webview.
    ThemeSnapshot(super::dto::ThemeSnapshotDto),
    /// Complete runtime generation with Rust-parsed package UI and resolved theme.
    /// Boxed: the DTO is large and this enum moves per event.
    RuntimeSnapshot {
        client_id: ClientId,
        tab_id: Option<TabId>,
        snapshot: Box<super::dto::RuntimeSnapshotDto>,
    },
    /// The server connection dropped. The webview shows a reconnect
    /// affordance; `session_reconnect` re-establishes everything.
    Disconnected {
        reason: String,
        // ts-rs's serde-compat does not carry `skip_serializing_if` into struct
        // variants, so the two optional keys are stated explicitly: the webview
        // receives them *absent*, not as `null`.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts-bindings", ts(optional))]
        client_id: Option<ClientId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts-bindings", ts(optional))]
        tab_id: Option<TabId>,
    },
}

#[cfg(test)]
mod runtime_projection_tests {
    use super::*;
    #[cfg(feature = "ts-bindings")]
    use ts_rs::TS;

    /// Regenerates the webview contract's TypeScript from the DTO layer
    /// (plan 119 SC-1). `scripts/check.sh` runs this and fails on a stale diff,
    /// so the generated file can never drift from what `dto.rs` serializes.
    ///
    /// Run with `--features ts-bindings`; every contract type exports into the
    /// one generated file so the frontend imports a single module.
    #[cfg(feature = "ts-bindings")]
    #[test]
    fn export_webview_contract_bindings() {
        // `number` for 64-bit integers: the wire JSON carries plain numbers and the
        // hand contract already typed them as numbers; ids that can exceed the
        // safe-integer range cross as strings by construction (menu session ids).
        let cfg = ts_rs::Config::new()
            .with_large_int("number")
            .with_out_dir("../frontend/src/bridge/generated");
        for (name, export) in [
            (
                "BootstrapDto",
                BootstrapDto::export_all as fn(&ts_rs::Config) -> _,
            ),
            ("InitialDocumentDto", InitialDocumentDto::export_all),
            ("ThemeSnapshotDto", ThemeSnapshotDto::export_all),
            ("TypographySnapshotDto", TypographySnapshotDto::export_all),
            (
                "DesignSystemSnapshotDto",
                DesignSystemSnapshotDto::export_all,
            ),
            ("IconPackSnapshotDto", IconPackSnapshotDto::export_all),
            ("RuntimeSnapshotDto", RuntimeSnapshotDto::export_all),
            ("BridgeEnvelope", BridgeEnvelope::export_all),
            (
                "BridgeError",
                crate::bridge::errors::BridgeError::export_all,
            ),
        ] {
            export(&cfg).unwrap_or_else(|error| panic!("exporting {name}: {error}"));
        }
    }

    use clay::protocol::{
        ActiveTheme, ActiveTypography, BehaviorManifest, EmptyTabContent, PackagePanelContent,
        PackageUiProvenance, PackageUiSnapshot, PackageUiTrustDomain, RuntimeStateSnapshot,
        SduiNode, SduiNodeId, SduiNodeKind,
    };

    #[test]
    fn runtime_snapshot_parses_package_components_and_hides_raw_theme_overrides() {
        let snapshot = RuntimeStateSnapshot {
            runtime_generation_id: 3,
            client_id: 7,
            behavior: BehaviorManifest::minimal_text_editing(3),
            active_theme: ActiveTheme {
                specifier: "@clay/default".into(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: ActiveTypography::default(),
            active_design_system: ActiveDesignSystem::core_fallback(3),
            active_icon_pack: None,
            ui_choices: clay::protocol::UiChoicesSnapshot::default(),
            sdui_tree: SduiTree {
                ui_version: 3,
                root_id: SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: "Ready".into(),
                        icon: None,
                    },
                )],
            },
            package_ui: PackageUiSnapshot {
                version: 3,
                surfaces: vec![EmptyTabContent {
                    id: "coding-agent.surface".into(),
                    package_name: "@clay/coding-agent".into(),
                    component_json: r#"{"id":"coding-agent.root","kind":"panel","children":[]}"#
                        .into(),
                    action_targets: vec!["coding-agent.profile".into()],
                    provenance: PackageUiProvenance {
                        package_name: "@clay/coding-agent".into(),
                        package_version: "0.1.0".into(),
                        api_prefix: "coding-agent".into(),
                        trust_domain: PackageUiTrustDomain::Trusted,
                    },
                }],
                panels: vec![PackagePanelContent {
                    id: "settings.surface".into(),
                    slot: "right".into(),
                    visibility: "visible".into(),
                    component_json: r#"{"id":"settings.root","kind":"panel","children":[]}"#.into(),
                    action_targets: Vec::new(),
                    provenance: PackageUiProvenance {
                        package_name: "@clay/settings".into(),
                        package_version: "0.1.0".into(),
                        api_prefix: "settings".into(),
                        trust_domain: PackageUiTrustDomain::Trusted,
                    },
                }],
                ..Default::default()
            },
            documents: Vec::new(),
            diagnostics: Vec::new(),
        };
        let dto = RuntimeSnapshotDto::resolve(snapshot).expect("projection");
        let value = serde_json::to_value(BridgeEnvelope::RuntimeSnapshot {
            client_id: 7,
            tab_id: None,
            snapshot: Box::new(dto),
        })
        .expect("json");
        assert_eq!(
            value["data"]["snapshot"]["packageUi"]["panels"][0]["component"]["kind"],
            "panel"
        );
        // Named pane surfaces must survive the bridge projection — the
        // Coding Agent split renders only when `packageUi.surfaces` lands.
        assert_eq!(
            value["data"]["snapshot"]["packageUi"]["surfaces"][0]["id"],
            "coding-agent.surface"
        );
        assert_eq!(
            value["data"]["snapshot"]["packageUi"]["surfaces"][0]["provenance"]["packageName"],
            "@clay/coding-agent"
        );
        assert!(
            value["data"]["snapshot"]["activeTheme"]
                .get("overrides")
                .is_none()
        );
    }
}
