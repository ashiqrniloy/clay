//! Clay-owned typed theme tokens for package UI and SDUI rendering.
//!
//! Package declarations may name semantic, package-prefixed tokens, but they do
//! not provide raw colors, CSS, renderer callbacks, or native style handles.
//! Clay resolves every package token through a same-typed core fallback token
//! before Masonry paint/layout reads cached native values.

use std::collections::BTreeMap;

use masonry::peniko::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ThemeTokenType {
    ColorRole,
    Spacing,
    Radius,
    Typography,
    Opacity,
}

impl ThemeTokenType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "color-role" => Some(Self::ColorRole),
            "spacing" => Some(Self::Spacing),
            "radius" => Some(Self::Radius),
            "typography" => Some(Self::Typography),
            "opacity" => Some(Self::Opacity),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ColorRole => "color-role",
            Self::Spacing => "spacing",
            Self::Radius => "radius",
            Self::Typography => "typography",
            Self::Opacity => "opacity",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedThemeValue {
    Color(Color),
    F64(f64),
    F32(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedThemeToken {
    pub(crate) requested_token: String,
    pub(crate) core_token: String,
    pub(crate) token_type: ThemeTokenType,
    pub(crate) value: ResolvedThemeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageThemeToken {
    pub(crate) token: String,
    pub(crate) token_type: ThemeTokenType,
    pub(crate) fallback: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThemeTokenResolver {
    package_tokens: BTreeMap<String, PackageThemeToken>,
}

impl ThemeTokenResolver {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert_package_token(
        &mut self,
        token: PackageThemeToken,
    ) -> Option<PackageThemeToken> {
        self.package_tokens.insert(token.token.clone(), token)
    }

    pub(crate) fn token_type(&self, token: &str) -> Option<ThemeTokenType> {
        self.package_tokens
            .get(token)
            .map(|declaration| declaration.token_type)
            .or_else(|| core_token_type(token))
    }

    pub(crate) fn resolves_as(&self, token: &str, expected: ThemeTokenType) -> bool {
        self.token_type(token) == Some(expected) && self.resolve(token, expected).is_some()
    }

    pub(crate) fn resolve(
        &self,
        token: &str,
        expected: ThemeTokenType,
    ) -> Option<ResolvedThemeToken> {
        if let Some(package_token) = self.package_tokens.get(token) {
            if package_token.token_type != expected {
                return None;
            }
            let fallback = core_theme_value(&package_token.fallback)?;
            if fallback.token_type != expected {
                return None;
            }
            return Some(ResolvedThemeToken {
                requested_token: token.to_string(),
                core_token: package_token.fallback.clone(),
                token_type: expected,
                value: fallback.value,
            });
        }

        let core = core_theme_value(token)?;
        if core.token_type != expected {
            return None;
        }
        Some(ResolvedThemeToken {
            requested_token: token.to_string(),
            core_token: token.to_string(),
            token_type: expected,
            value: core.value,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CoreThemeValue {
    token_type: ThemeTokenType,
    value: ResolvedThemeValue,
}

pub(crate) fn core_token_type(token: &str) -> Option<ThemeTokenType> {
    core_theme_value(token).map(|value| value.token_type)
}

pub(crate) fn core_fallback_matches_type(fallback: &str, token_type: ThemeTokenType) -> bool {
    core_token_type(fallback) == Some(token_type)
}

fn core_theme_value(token: &str) -> Option<CoreThemeValue> {
    use ResolvedThemeValue::{Color as ColorValue, F32, F64};
    use ThemeTokenType::{ColorRole, Opacity, Radius, Spacing, Typography};

    let value = match token {
        "surface.panel" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x21, 0x20, 0x2b)),
        },
        "surface.overlay" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x18, 0x17, 0x20)),
        },
        "surface.main" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x10, 0x0f, 0x17)),
        },
        "surface.control" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x39, 0x35, 0x4a)),
        },
        "surface.list" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x29, 0x28, 0x35)),
        },
        "surface.selected" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x3d, 0x38, 0x5c)),
        },
        "text.primary" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xee, 0xea, 0xff)),
        },
        "text.muted" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xb9, 0xb2, 0xcf)),
        },
        "accent.primary" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0x7c, 0x6f, 0xff)),
        },
        "diagnostic.error" => CoreThemeValue {
            token_type: ColorRole,
            value: ColorValue(Color::from_rgb8(0xff, 0x6b, 0x6b)),
        },
        "spacing.none" => CoreThemeValue {
            token_type: Spacing,
            value: F64(0.0),
        },
        "spacing.inline" => CoreThemeValue {
            token_type: Spacing,
            value: F64(6.0),
        },
        "spacing.panel" => CoreThemeValue {
            token_type: Spacing,
            value: F64(14.0),
        },
        "spacing.row" => CoreThemeValue {
            token_type: Spacing,
            value: F64(26.0),
        },
        "radius.none" => CoreThemeValue {
            token_type: Radius,
            value: F64(0.0),
        },
        "radius.panel" => CoreThemeValue {
            token_type: Radius,
            value: F64(6.0),
        },
        "typography.body" => CoreThemeValue {
            token_type: Typography,
            value: F32(12.0),
        },
        "typography.title" => CoreThemeValue {
            token_type: Typography,
            value: F32(14.0),
        },
        "typography.status" => CoreThemeValue {
            token_type: Typography,
            value: F32(12.0),
        },
        "opacity.disabled" => CoreThemeValue {
            token_type: Opacity,
            value: F32(0.55),
        },
        "opacity.full" => CoreThemeValue {
            token_type: Opacity,
            value: F32(1.0),
        },
        _ => return None,
    };
    Some(value)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiThemeStyle {
    pub(crate) panel_padding: f64,
    pub(crate) row_height: f64,
    pub(crate) title_text_size: f32,
    pub(crate) body_text_size: f32,
    pub(crate) panel_background: Color,
    pub(crate) button_background: Color,
    pub(crate) list_background: Color,
    pub(crate) selected_background: Color,
    pub(crate) text_color: Color,
    pub(crate) muted_text_color: Color,
}

impl SduiThemeStyle {
    pub(crate) fn from_resolver(resolver: &ThemeTokenResolver) -> Self {
        Self {
            panel_padding: resolve_f64(resolver, "spacing.panel", ThemeTokenType::Spacing),
            row_height: resolve_f64(resolver, "spacing.row", ThemeTokenType::Spacing),
            title_text_size: resolve_f32(resolver, "typography.title", ThemeTokenType::Typography),
            body_text_size: resolve_f32(resolver, "typography.body", ThemeTokenType::Typography),
            panel_background: resolve_color(resolver, "surface.panel"),
            button_background: resolve_color(resolver, "surface.control"),
            list_background: resolve_color(resolver, "surface.list"),
            selected_background: resolve_color(resolver, "surface.selected"),
            text_color: resolve_color(resolver, "text.primary"),
            muted_text_color: resolve_color(resolver, "text.muted"),
        }
    }
}

impl Default for SduiThemeStyle {
    fn default() -> Self {
        Self::from_resolver(&ThemeTokenResolver::new())
    }
}

fn resolve_color(resolver: &ThemeTokenResolver, token: &str) -> Color {
    match resolver.resolve(token, ThemeTokenType::ColorRole) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::Color(color),
            ..
        }) => color,
        _ => Color::from_rgb8(0xff, 0x00, 0xff),
    }
}

fn resolve_f64(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f64 {
    match resolver.resolve(token, token_type) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F64(value),
            ..
        }) => value,
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F32(value),
            ..
        }) => f64::from(value),
        _ => 0.0,
    }
}

fn resolve_f32(resolver: &ThemeTokenResolver, token: &str, token_type: ThemeTokenType) -> f32 {
    match resolver.resolve(token, token_type) {
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F32(value),
            ..
        }) => value,
        Some(ResolvedThemeToken {
            value: ResolvedThemeValue::F64(value),
            ..
        }) => value as f32,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_token_registry_resolves_package_tokens_to_core_fallbacks() {
        let mut resolver = ThemeTokenResolver::new();
        resolver.insert_package_token(PackageThemeToken {
            token: "markdown.preview.background".to_string(),
            token_type: ThemeTokenType::ColorRole,
            fallback: "surface.panel".to_string(),
            description: "Markdown preview background".to_string(),
        });

        let resolved = resolver
            .resolve("markdown.preview.background", ThemeTokenType::ColorRole)
            .expect("package token should resolve through core fallback");

        assert_eq!(resolved.core_token, "surface.panel");
        assert_eq!(resolved.token_type, ThemeTokenType::ColorRole);
        assert_eq!(
            resolved.value,
            ResolvedThemeValue::Color(Color::from_rgb8(0x21, 0x20, 0x2b))
        );
    }

    #[test]
    fn theme_token_registry_rejects_unknown_tokens_and_type_mismatches() {
        let mut resolver = ThemeTokenResolver::new();
        resolver.insert_package_token(PackageThemeToken {
            token: "markdown.preview.background".to_string(),
            token_type: ThemeTokenType::ColorRole,
            fallback: "surface.panel".to_string(),
            description: "Markdown preview background".to_string(),
        });

        assert!(
            resolver
                .resolve("markdown.preview.background", ThemeTokenType::Spacing)
                .is_none()
        );
        assert!(
            resolver
                .resolve("markdown.preview.missing", ThemeTokenType::ColorRole)
                .is_none()
        );
        assert!(!core_fallback_matches_type(
            "surface.panel",
            ThemeTokenType::Spacing
        ));
    }

    #[test]
    fn sdui_theme_style_uses_core_tokens_for_compatibility_renderer() {
        let style = SduiThemeStyle::default();

        assert_eq!(style.panel_padding, 14.0);
        assert_eq!(style.row_height, 26.0);
        assert_eq!(style.title_text_size, 14.0);
        assert_eq!(style.body_text_size, 12.0);
        assert_eq!(style.panel_background, Color::from_rgb8(0x21, 0x20, 0x2b));
    }
}
