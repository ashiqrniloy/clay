//! Clay-owned package UI component catalog and typed style variables.
//!
//! This module defines the schema-level component names and style variables that
//! package UI validators accept. It intentionally has no Masonry widget IDs,
//! native handles, renderer callbacks, CSS parsing, or client JavaScript hooks.

use serde_json::{Map, Value};

use crate::protocol::FontRole;

use super::theme::{ThemeTokenResolver, ThemeTokenType};

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

pub(crate) fn validate_style_variables(
    object: &Map<String, Value>,
    resolver: &ThemeTokenResolver,
) -> Result<Vec<ComponentStyleVariable>, ComponentCatalogError> {
    let Some(style) = object.get("style") else {
        return Ok(Vec::new());
    };
    let style = style.as_object().ok_or_else(|| ComponentCatalogError {
        field: "style".to_string(),
        message: "component style must be an object of typed style variables".to_string(),
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
            message: "style token variables must be non-empty token strings".to_string(),
        })?;
    reject_raw_style_token(name, token)?;
    if !resolver.resolves_as(token, expected) {
        return Err(ComponentCatalogError {
            field: format!("style.{name}"),
            message: format!("style variable `{name}` must reference a known {expected} token"),
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
            let value = value
                .as_str()
                .and_then(FontRole::from_name)
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.fontRole".to_string(),
                    message: "style.fontRole must be ui, monospace, or proportional".to_string(),
                })?;
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: match value {
                        FontRole::Monospace => "monospace".to_string(),
                        FontRole::Proportional => "proportional".to_string(),
                        FontRole::Ui => "ui".to_string(),
                    },
                },
            })
        }
        "variant" => {
            let value = value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.variant".to_string(),
                    message: "style.variant must be a non-empty enum string".to_string(),
                })?;
            if !matches!(value, "default" | "muted" | "primary" | "danger") {
                return Err(ComponentCatalogError {
                    field: "style.variant".to_string(),
                    message: "style.variant must be default, muted, primary, or danger".to_string(),
                });
            }
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: value.to_string(),
                },
            })
        }
        "validationState" => {
            let value = value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ComponentCatalogError {
                    field: "style.validationState".to_string(),
                    message: "style.validationState must be a non-empty enum string".to_string(),
                })?;
            if !matches!(value, "none" | "error" | "warning" | "success") {
                return Err(ComponentCatalogError {
                    field: "style.validationState".to_string(),
                    message: "style.validationState must be none, error, warning, or success"
                        .to_string(),
                });
            }
            Ok(ComponentStyleVariable {
                name: name.to_string(),
                value: ComponentStyleValue::Enum {
                    value: value.to_string(),
                },
            })
        }
        _ => Err(ComponentCatalogError {
            field: format!("style.{name}"),
            message: format!("unsupported typed style variable `{name}`"),
        }),
    }
}

fn reject_raw_style_token(name: &str, token: &str) -> Result<(), ComponentCatalogError> {
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
        return Err(ComponentCatalogError {
            field: format!("style.{name}"),
            message: "style variables must reference typed Clay tokens, not raw CSS or raw colors"
                .to_string(),
        });
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
}
