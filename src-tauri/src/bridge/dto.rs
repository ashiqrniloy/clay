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
    ActiveTypography, BehaviorManifest, ClientId, DecorationKind, DocumentAccess, DocumentId,
    DocumentVersion, FontProfile, Modifiers, PackageUiProvenance, RuntimeDiagnostic,
    RuntimeStateSnapshot, SduiTree, TabId, TokenType, UiTypographyHierarchy,
};
use clay::shell::theme::{ThemeTokenValueDto, density_spacing_scale, resolve_theme_token_snapshot};
use serde::Serialize;
use std::collections::BTreeMap;

/// Complete session state installed on connect/reconnect. One atomic
/// projection: the webview replaces its previous bootstrap wholesale
/// (reconnect must never merge across sessions).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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
}

/// Resolved theme projection consumed by the frontend theme adapter.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    pub(crate) fn resolve(
        specifier: &str,
        theme: &clay::protocol::ActiveTheme,
    ) -> Result<Self, String> {
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
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TypographySnapshotDto {
    pub revision: u64,
    pub monospace: FontProfile,
    pub proportional: FontProfile,
    pub ui: FontProfile,
    pub hierarchy: UiTypographyHierarchy,
}

impl From<&ActiveTypography> for TypographySnapshotDto {
    fn from(active: &ActiveTypography) -> Self {
        Self {
            revision: active.revision,
            monospace: active.monospace.clone(),
            proportional: active.proportional.clone(),
            ui: active.ui.clone(),
            hierarchy: active.hierarchy,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitialDocumentDto {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub head: clay::protocol::DocumentTextHead,
    pub access: DocumentAccess,
    pub workspace_root: String,
}

impl InitialDocumentDto {
    pub(crate) fn from_initial_state(state: &ClientInitialState) -> Self {
        Self {
            document_id: state.document_id,
            version: state.document_version,
            head: state.head.clone(),
            access: state.access.clone(),
            workspace_root: state.workspace_root.clone(),
        }
    }
}

/// Safe atomic runtime-generation projection. Raw theme overrides and JSON
/// component strings are resolved/parsed in Rust before the webview observes it.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshotDto {
    pub runtime_generation_id: u64,
    pub behavior_manifest: BehaviorManifest,
    pub active_theme: ThemeSnapshotDto,
    pub active_typography: TypographySnapshotDto,
    pub sdui_tree: SduiTree,
    pub package_ui: PackageUiSnapshotDto,
    pub documents: Vec<clay::protocol::DocumentRuntimeRenderState>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageUiSnapshotDto {
    pub version: u64,
    pub empty_tab: Option<PackageSurfaceDto>,
    pub panels: Vec<PackagePanelDto>,
    pub overlays: Vec<PackageOverlayDto>,
    pub components: Vec<PackageSurfaceDto>,
    pub input_routes: Vec<clay::protocol::PackageInputRouteContent>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSurfaceDto {
    pub id: String,
    pub component: serde_json::Value,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePanelDto {
    pub id: String,
    pub slot: String,
    pub visibility: String,
    pub component: serde_json::Value,
    pub action_targets: Vec<String>,
    pub provenance: PackageUiProvenance,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
    pub(crate) fn resolve(snapshot: RuntimeStateSnapshot) -> Result<Self, String> {
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
            sdui_tree: snapshot.sdui_tree,
            package_ui: PackageUiSnapshotDto::parse(snapshot.package_ui)?,
            documents: snapshot.documents,
            diagnostics: snapshot.diagnostics,
        })
    }
}

impl PackageUiSnapshotDto {
    fn parse(snapshot: clay::protocol::PackageUiSnapshot) -> Result<Self, String> {
        let parse = |component: &str| {
            serde_json::from_str(component).map_err(|_| "invalid package component".to_string())
        };
        let empty_tab = snapshot
            .empty_tab
            .map(|entry| -> Result<PackageSurfaceDto, String> {
                Ok(PackageSurfaceDto {
                    id: entry.id,
                    component: parse(&entry.component_json)?,
                    action_targets: entry.action_targets,
                    provenance: entry.provenance,
                })
            })
            .transpose()?;
        let panels = snapshot
            .panels
            .into_iter()
            .map(|entry| {
                Ok(PackagePanelDto {
                    id: entry.id,
                    slot: entry.slot,
                    visibility: entry.visibility,
                    component: parse(&entry.component_json)?,
                    action_targets: entry.action_targets,
                    provenance: entry.provenance,
                })
            })
            .collect::<Result<_, String>>()?;
        let overlays = snapshot
            .overlays
            .into_iter()
            .map(|entry| {
                Ok(PackageOverlayDto {
                    id: entry.id,
                    anchor: entry.anchor,
                    focus_policy: entry.focus_policy,
                    dismissal_policy: entry.dismissal_policy,
                    component: parse(&entry.component_json)?,
                    action_targets: entry.action_targets,
                    provenance: entry.provenance,
                })
            })
            .collect::<Result<_, String>>()?;
        let components = snapshot
            .components
            .into_iter()
            .map(|entry| -> Result<PackageSurfaceDto, String> {
                Ok(PackageSurfaceDto {
                    id: entry.id,
                    component: parse(&entry.component_json)?,
                    action_targets: entry.action_targets,
                    provenance: entry.provenance,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            version: snapshot.version,
            empty_tab,
            panels,
            overlays,
            components,
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
pub enum BridgeEnvelope {
    /// Boxed: the client event union is large and this enum is moved often.
    Event(Box<ClientConnectionEvent>),
    /// Multi-tab event: same payload as `Event`, tagged with the owning client.
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
        #[serde(skip_serializing_if = "Option::is_none")]
        client_id: Option<ClientId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tab_id: Option<TabId>,
    },
}

#[cfg(test)]
mod runtime_projection_tests {
    use super::*;
    use clay::protocol::{
        ActiveTheme, ActiveTypography, BehaviorManifest, PackagePanelContent, PackageUiProvenance,
        PackageUiSnapshot, PackageUiTrustDomain, RuntimeStateSnapshot, SduiNode, SduiNodeId,
        SduiNodeKind,
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
            sdui_tree: SduiTree {
                ui_version: 3,
                root_id: SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: "Ready".into(),
                    },
                )],
            },
            package_ui: PackageUiSnapshot {
                version: 3,
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
        assert!(
            value["data"]["snapshot"]["activeTheme"]
                .get("overrides")
                .is_none()
        );
    }
}
