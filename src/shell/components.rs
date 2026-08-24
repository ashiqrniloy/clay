//! Clay-owned package UI component catalog and typed style variables.
//!
//! This module defines the schema-level component names and style variables that
//! package UI validators accept. It intentionally has no Masonry widget IDs,
//! native handles, renderer callbacks, CSS parsing, or client JavaScript hooks.

use serde_json::{Map, Value};

use crate::protocol::FontRole;

use super::theme::{ThemeTokenResolver, ThemeTokenType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionState {
    Rest,
    Hover,
    Active,
    Focus,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ComponentKind {
    EditorView,
    Panel,
    Label,
    Button,
    List,
    Flex,
    Stack,
    Overlay,
    Scroll,
    Portal,
    StatusItem,
    /// Phase 20.5: single-select drop-down. Trigger row + list-in-overlay.
    Dropdown,
    /// Phase 20.5: expand/collapse section with chevron and content toggle.
    Collapse,
    /// Phase 20.5: blocking dialog on `z.modal` with focus trap.
    Modal,
    /// Phase 20.5: single-line editable text field with focus, placeholder, and validation states.
    TextInput,
}

impl ComponentKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "editorView" => Some(Self::EditorView),
            "panel" => Some(Self::Panel),
            "label" => Some(Self::Label),
            "button" => Some(Self::Button),
            "list" => Some(Self::List),
            "flex" => Some(Self::Flex),
            "stack" => Some(Self::Stack),
            "overlay" => Some(Self::Overlay),
            "scroll" => Some(Self::Scroll),
            "portal" => Some(Self::Portal),
            "statusItem" => Some(Self::StatusItem),
            "dropdown" => Some(Self::Dropdown),
            "collapse" => Some(Self::Collapse),
            "modal" => Some(Self::Modal),
            "textInput" => Some(Self::TextInput),
            _ => None,
        }
    }

    /// Inverse of `parse`: the catalog string this variant round-trips to.
    /// Used by the package UI conformance matrix (Plan 068 task 5) so a single
    /// `ComponentKind` value drives both `applicable_states` and the
    /// `component_state_palette` paint path, tying the state table to paint.
    #[allow(dead_code)] // conformance primitive; consumed by the package UI conformance suite
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::EditorView => "editorView",
            Self::Panel => "panel",
            Self::Label => "label",
            Self::Button => "button",
            Self::List => "list",
            Self::Flex => "flex",
            Self::Stack => "stack",
            Self::Overlay => "overlay",
            Self::Scroll => "scroll",
            Self::Portal => "portal",
            Self::StatusItem => "statusItem",
            Self::Dropdown => "dropdown",
            Self::Collapse => "collapse",
            Self::Modal => "modal",
            Self::TextInput => "textInput",
        }
    }

    pub(crate) const fn supports_text_font_role(self) -> bool {
        matches!(
            self,
            Self::Panel
                | Self::Label
                | Self::Button
                | Self::List
                | Self::StatusItem
                | Self::Dropdown
                | Self::Collapse
                | Self::Modal
                | Self::TextInput
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeferredComponentKind {
    /// Reserved; no first-party package need identified as of Phase 20.5.
    Table,
}

impl DeferredComponentKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "table" => Some(Self::Table),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentStyleVariable {
    pub(crate) name: String,
    pub(crate) value: ComponentStyleValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComponentStyleValue {
    Token {
        token: String,
        token_type: ThemeTokenType,
    },
    Enum {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentCatalogError {
    pub(crate) field: String,
    pub(crate) message: String,
}

impl ComponentCatalogError {
    /// Stable author-facing rejection shape naming the offending field, the
    /// rejected value, the expected token type/enum/kind, and a short reason:
    /// `"{field} = \`{value}\` rejected: expected {expected}; {reason}"`. The
    /// rejected value is sanitized so an author string cannot break the message
    /// shape or leak unbounded content into the server diagnostics channel.
    pub(crate) fn reject(field: &str, rejected_value: &str, expected: &str, reason: &str) -> Self {
        Self {
            field: field.to_string(),
            message: format!(
                "{field} = `{value}` rejected: expected {expected}; {reason}",
                value = sanitize_rejected(rejected_value)
            ),
        }
    }
}

/// Trim, strip backticks (so an author string cannot break the `` `…` ``
/// diagnostic shape), and bound to 80 chars so a pathological value cannot
/// blow up the message. ponytail: 80 chars covers every realistic token/enum
/// value; raise only if a real author value is truncated.
fn sanitize_rejected(value: &str) -> String {
    let trimmed = value.trim().replace('`', "'");
    if trimmed.chars().count() > 80 {
        let bounded: String = trimmed.chars().take(80).collect();
        format!("{bounded}…")
    } else {
        trimmed
    }
}

/// Compact description of a rejected `serde_json::Value`'s shape for a "got …"
/// diagnostic fragment, so an author sees what they supplied, not only what was
/// expected.
fn json_value_kind(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => format!("boolean {b}"),
        Value::Number(n) => format!("number {n}"),
        Value::String(s) => format!("string `{}`", sanitize_rejected(s)),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

pub(crate) fn validate_component_kind(kind: &str) -> Result<ComponentKind, ComponentCatalogError> {
    if let Some(kind) = ComponentKind::parse(kind) {
        return Ok(kind);
    }
    if DeferredComponentKind::parse(kind).is_some() {
        return Err(ComponentCatalogError {
            field: "kind".to_string(),
            message: format!(
                "component kind `{kind}` is reserved for a later Clay component catalog phase"
            ),
        });
    }
    Err(ComponentCatalogError {
        field: "kind".to_string(),
        message: format!("unsupported component kind `{kind}`"),
    })
}

/// The `InteractionState` values a `ComponentKind` meaningfully renders from
/// theme tokens. Grounded in `references/components.md` per-kind interaction
/// notes (Phase 20.4/20.5): interactive triggers render all five states; chrome
/// containers render only `Rest` (state-independent chrome); text-no-fill kinds
/// render `Rest`/`Focus`/`Disabled` (focus ring + disabled dim, no fill);
/// scrollbar-bearing kinds render `Rest`/`Hover`/`Active`; layout containers
/// render only `Rest`. Consumed by the package UI conformance matrix (Plan 068
/// task 5) so it renders each kind only in the states its paint path supports.
/// ponytail: table is best-effort from components.md; the task-5 render matrix
/// is the ground truth that will correct any mismatch here.
#[allow(dead_code)] // conformance primitive; consumed by the package UI conformance suite (Plan 068 task 5)
pub(crate) fn applicable_states(kind: ComponentKind) -> &'static [InteractionState] {
    use ComponentKind::*;
    use InteractionState::*;
    match kind {
        // Interactive triggers: Rest/Hover/Active/Focus/Disabled (components.md
        // lines 35, 36, 51, 52, 54).
        Button | List | Dropdown | Collapse | TextInput => &[Rest, Hover, Active, Focus, Disabled],
        // Chrome containers: state-independent chrome; currently Rest
        // (components.md lines 38, 53).
        Panel | Overlay | Modal => &[Rest],
        // Text-no-fill: focus ring on Focus, disabled dim, no fill
        // (components.md line 37).
        Label | StatusItem => &[Rest, Focus, Disabled],
        // Scrollbar-bearing: Hover/Active via paint_scroll_chrome, no SDUI
        // state-token fill (components.md lines 38, 39).
        EditorView | Scroll => &[Rest, Hover, Active],
        // Layout containers: non-interactive (no per-kind state notes).
        Flex | Stack | Portal => &[Rest],
    }
}

pub(crate) fn validate_style_variables(
    object: &Map<String, Value>,
    resolver: &ThemeTokenResolver,
) -> Result<Vec<ComponentStyleVariable>, ComponentCatalogError> {
    let Some(style) = object.get("style") else {
        return Ok(Vec::new());
    };
    let style = style.as_object().ok_or_else(|| ComponentCatalogError {
        field: "style".to_string(),
        message: format!(
            "component style must be an object of typed style variables; got {}",
            json_value_kind(style)
        ),
    })?;

    let mut variables = Vec::new();
    for (name, value) in style {
        let variable = validate_style_variable(name, value, resolver)?;
        variables.push(variable);
    }
    Ok(variables)
}

fn validate_style_variable(
    name: &str,
    value: &Value,
    resolver: &ThemeTokenResolver,
) -> Result<ComponentStyleVariable, ComponentCatalogError> {
    let Some(expected) = token_type_for_style_variable(name) else {
        return validate_enum_style_variable(name, value);
    };
    let token = value
        .as_str()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| ComponentCatalogError {
            field: format!("style.{name}"),
            message: format!(
                "style.{name} must be a non-empty token string; got {}",
                json_value_kind(value)
            ),
        })?;
    reject_raw_style_token(name, token, expected)?;
    if !resolver.resolves_as(token, expected) {
        return Err(ComponentCatalogError {
            field: format!("style.{name}"),
            message: format!(
                "style variable `{name}` must reference a known {expected} token; got `{}`",
                sanitize_rejected(token)
            ),
        });
    }
    Ok(ComponentStyleVariable {
        name: name.to_string(),
        value: ComponentStyleValue::Token {
            token: token.to_string(),
            token_type: expected,
        },
    })
}

fn token_type_for_style_variable(name: &str) -> Option<ThemeTokenType> {
    match name {
        "background" | "contentColor" | "borderColor" | "accentColor" => {
            Some(ThemeTokenType::ColorRole)
        }
        "padding" | "gap" | "rowHeight" | "inset" => Some(ThemeTokenType::Spacing),
        "placeholderColor" => Some(ThemeTokenType::ColorRole),
        "radius" => Some(ThemeTokenType::Radius),
        "typography" => Some(ThemeTokenType::Typography),
        "opacity" => Some(ThemeTokenType::Opacity),
        _ => None,
    }
}

fn validate_enum_style_variable(
    name: &str,
    value: &Value,
) -> Result<ComponentStyleVariable, ComponentCatalogError> {
    match name {
        "fontRole" => {
            let role = value
                .as_str()
                .and_then(FontRole::from_name)
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.fontRole".to_string(),
                    message: format!(
                        "style.fontRole must be one of ui, monospace, proportional; got {}",
                        json_value_kind(value)
                    ),
                })?;
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: match role {
                        FontRole::Monospace => "monospace".to_string(),
                        FontRole::Proportional => "proportional".to_string(),
                        FontRole::Ui => "ui".to_string(),
                    },
                },
            })
        }
        "variant" => {
            let raw = value
                .as_str()
                .filter(|raw| !raw.trim().is_empty())
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.variant".to_string(),
                    message: format!(
                        "style.variant must be a non-empty enum string; got {}",
                        json_value_kind(value)
                    ),
                })?;
            if !matches!(raw, "default" | "muted" | "primary" | "danger") {
                return Err(ComponentCatalogError {
                    field: "style.variant".to_string(),
                    message: format!(
                        "style.variant must be one of default, muted, primary, danger; got `{}`",
                        sanitize_rejected(raw)
                    ),
                });
            }
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: raw.to_string(),
                },
            })
        }
        "validationState" => {
            let raw = value
                .as_str()
                .filter(|raw| !raw.trim().is_empty())
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.validationState".to_string(),
                    message: format!(
                        "style.validationState must be a non-empty enum string; got {}",
                        json_value_kind(value)
                    ),
                })?;
            if !matches!(raw, "none" | "error" | "warning" | "success") {
                return Err(ComponentCatalogError {
                    field: "style.validationState".to_string(),
                    message: format!(
                        "style.validationState must be one of none, error, warning, success; got `{}`",
                        sanitize_rejected(raw)
                    ),
                });
            }
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: raw.to_string(),
                },
            })
        }
        _ => Err(ComponentCatalogError {
            field: format!("style.{name}"),
            message: format!("unsupported typed style variable `{name}`"),
        }),
    }
}

fn reject_raw_style_token(
    name: &str,
    token: &str,
    expected: ThemeTokenType,
) -> Result<(), ComponentCatalogError> {
    let lower = token.to_ascii_lowercase();
    if lower.starts_with('#')
        || lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.contains(';')
        || lower.contains('{')
        || lower.contains('}')
        || lower.contains(": ")
        || lower.contains(':')
    {
        return Err(ComponentCatalogError::reject(
            &format!("style.{name}"),
            token,
            &format!("{expected} token"),
            "raw colors or raw CSS are not allowed; reference a Clay token (e.g. surface.main)",
        ));
    }
    Ok(())
}

impl std::fmt::Display for ThemeTokenType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::shell::theme::{PackageThemeToken, ThemeTokenResolver};

    #[test]
    fn component_catalog_accepts_supported_v1_component_kinds() {
        let supported = [
            "editorView",
            "panel",
            "label",
            "button",
            "list",
            "flex",
            "stack",
            "overlay",
            "scroll",
            "portal",
            "statusItem",
            // Phase 20.5: promoted from deferred.
            "dropdown",
            "collapse",
            "modal",
            // Phase 20.5: new kind.
            "textInput",
        ];

        for kind in supported {
            assert!(
                validate_component_kind(kind).is_ok(),
                "{kind} should be supported"
            );
        }

        // table remains reserved; no first-party package need as of Phase 20.5.
        let error = validate_component_kind("table").unwrap_err();
        assert!(error.message.contains("reserved for a later"));
    }

    #[test]
    fn component_catalog_accepts_validation_state_enum() {
        let resolver = ThemeTokenResolver::new();
        for state in ["none", "error", "warning", "success"] {
            let object = json!({ "style": { "validationState": state } });
            assert!(
                validate_style_variables(object.as_object().unwrap(), &resolver).is_ok(),
                "validationState={state} should be accepted"
            );
        }
        let invalid = json!({ "style": { "validationState": "critical" } });
        assert!(validate_style_variables(invalid.as_object().unwrap(), &resolver).is_err());
    }

    #[test]
    fn component_catalog_accepts_placeholder_color_token() {
        let resolver = ThemeTokenResolver::new();
        let object = json!({ "style": { "placeholderColor": "text.muted" } });
        assert!(validate_style_variables(object.as_object().unwrap(), &resolver).is_ok());
        let mismatch = json!({ "style": { "placeholderColor": "spacing.md" } });
        assert!(validate_style_variables(mismatch.as_object().unwrap(), &resolver).is_err());
    }

    #[test]
    fn component_catalog_validates_typed_style_variables_against_tokens() {
        let mut resolver = ThemeTokenResolver::new();
        resolver.insert_package_token(PackageThemeToken {
            token: "markdown.preview.background".to_string(),
            token_type: ThemeTokenType::ColorRole,
            fallback: "surface.panel".to_string(),
            description: "Markdown preview background".to_string(),
        });
        let object = json!({
            "style": {
                "background": "markdown.preview.background",
                "padding": "spacing.panel",
                "typography": "typography.body",
                "variant": "muted"
            }
        });
        let style = validate_style_variables(object.as_object().unwrap(), &resolver).unwrap();

        assert_eq!(style.len(), 4);
        assert!(style.iter().any(|variable| variable.name == "background"));
    }

    #[test]
    fn component_catalog_rejects_unknown_type_incompatible_and_raw_style_variables() {
        let resolver = ThemeTokenResolver::new();
        let unknown = json!({ "style": { "shadow": "surface.panel" } });
        let mismatch = json!({ "style": { "padding": "surface.panel" } });
        let raw_color = json!({ "style": { "background": "#ff00aa" } });
        let concrete_font = json!({ "style": { "fontFamily": "JetBrains Mono" } });
        let concrete_size = json!({ "style": { "fontSize": 16 } });

        assert!(validate_style_variables(unknown.as_object().unwrap(), &resolver).is_err());
        assert!(validate_style_variables(mismatch.as_object().unwrap(), &resolver).is_err());
        assert!(validate_style_variables(raw_color.as_object().unwrap(), &resolver).is_err());
        assert!(validate_style_variables(concrete_font.as_object().unwrap(), &resolver).is_err());
        assert!(validate_style_variables(concrete_size.as_object().unwrap(), &resolver).is_err());
    }

    #[test]
    fn component_catalog_accepts_semantic_font_roles_only() {
        let resolver = ThemeTokenResolver::new();
        for role in ["ui", "monospace", "proportional"] {
            let object = json!({ "style": { "fontRole": role } });
            assert!(validate_style_variables(object.as_object().unwrap(), &resolver).is_ok());
        }
        let invalid = json!({ "style": { "fontRole": "serif" } });
        assert!(validate_style_variables(invalid.as_object().unwrap(), &resolver).is_err());
    }

    /// Phase 20.7 task 4: `applicable_states` is non-empty and contains `Rest`
    /// for every `ComponentKind`, and the documented per-kind sets match the
    /// components.md interaction notes. Anchor cases pin the table so a future
    /// edit cannot silently narrow or widen a kind's applicable states.
    #[test]
    fn applicable_states_table_matches_components_md() {
        use super::InteractionState;
        use ComponentKind::*;
        use InteractionState::*;

        // Every kind is non-empty and includes Rest.
        for kind in [
            EditorView, Panel, Label, Button, List, Flex, Stack, Overlay, Scroll, Portal,
            StatusItem, Dropdown, Collapse, Modal, TextInput,
        ] {
            let states = super::applicable_states(kind);
            assert!(!states.is_empty(), "{kind:?} has no applicable states");
            assert!(states.contains(&Rest), "{kind:?} must include Rest");
        }

        // Anchor: interactive triggers render all five states.
        assert_eq!(
            super::applicable_states(Button),
            &[Rest, Hover, Active, Focus, Disabled]
        );
        assert_eq!(
            super::applicable_states(List),
            &[Rest, Hover, Active, Focus, Disabled]
        );
        assert_eq!(
            super::applicable_states(Dropdown),
            &[Rest, Hover, Active, Focus, Disabled]
        );
        assert_eq!(
            super::applicable_states(Collapse),
            &[Rest, Hover, Active, Focus, Disabled]
        );
        assert_eq!(
            super::applicable_states(TextInput),
            &[Rest, Hover, Active, Focus, Disabled]
        );
        // Anchor: chrome containers are Rest-only (state-independent chrome).
        assert_eq!(super::applicable_states(Panel), &[Rest]);
        assert_eq!(super::applicable_states(Overlay), &[Rest]);
        assert_eq!(super::applicable_states(Modal), &[Rest]);
        // Anchor: text-no-fill kinds render Rest/Focus/Disabled.
        assert_eq!(super::applicable_states(Label), &[Rest, Focus, Disabled]);
        assert_eq!(
            super::applicable_states(StatusItem),
            &[Rest, Focus, Disabled]
        );
        // Anchor: scrollbar-bearing kinds render Rest/Hover/Active.
        assert_eq!(super::applicable_states(EditorView), &[Rest, Hover, Active]);
        assert_eq!(super::applicable_states(Scroll), &[Rest, Hover, Active]);
        // Anchor: layout containers are Rest-only.
        assert_eq!(super::applicable_states(Flex), &[Rest]);
        assert_eq!(super::applicable_states(Stack), &[Rest]);
        assert_eq!(super::applicable_states(Portal), &[Rest]);
    }
}
