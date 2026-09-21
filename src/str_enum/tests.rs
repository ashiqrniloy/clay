//! Golden round-trip checks for every [`crate::str_enum::string_enum_impl!`]
//! invocation.
//!
//! The `(variant, string)` pairs below were extracted from the hand-written
//! `parse`/`as_str` tables *before* the macro refactor, so this file pins the
//! wire strings instead of re-deriving them from the macro it is checking. A
//! string that changes without a deliberate edit here is a protocol/config
//! compatibility break.
//!
//! `TextobjectKind` is absent on purpose: it already derives `parse` from its
//! own public `ALL` vocabulary list, so there is no duplicated table to
//! collapse.

use crate::packages::extension_points::{ExtensionContributionKind, RelationOperation};
use crate::packages::self_update::ChannelKind;
use crate::perf::fixtures::FixtureKind;
use crate::protocol::theme::Appearance;
use crate::shell::components::ComponentKind;
use crate::shell::design_system::{
    BorderStyle, OutlineStyle, RecipeState, TransformPreset, TransitionTiming,
};
use crate::shell::theme::{DensityLevel, ElevationLevel, ThemeTokenType, ZLevel};

/// Strings that must never be accepted by any of the enums below.
const UNKNOWN: &[&str] = &["", "nope", "NONE", "None", " not-a-real-value"];

/// Local example: the macro is also its own regression test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Example {
    One,
    TwoWords,
}

string_enum_impl! {
    pub(crate) Example {
        One => "one",
        TwoWords => "two-words",
    }
}

/// `golden` must cover every variant exactly once, `as_str` must return the
/// pinned string, `parse` must round-trip it and reject the unknown samples.
fn roundtrip<E: Copy + PartialEq + std::fmt::Debug>(
    name: &str,
    all: &[E],
    golden: &[(E, &str)],
    as_str: fn(E) -> &'static str,
    parse: Option<fn(&str) -> Option<E>>,
) {
    assert_eq!(
        all.len(),
        golden.len(),
        "{name}: macro variant list and golden pairs disagree"
    );
    let mut seen = std::collections::HashSet::new();
    for (variant, text) in golden {
        assert!(
            all.contains(variant),
            "{name}: {variant:?} is not a variant"
        );
        assert_eq!(as_str(*variant), *text, "{name}: as_str({variant:?})");
        assert!(seen.insert(*text), "{name}: duplicate string {text:?}");
    }
    // `None` for enums whose `parse` stays module-private; the macro builds
    // `parse` from this very list, so the pinned strings cover it.
    let Some(parse) = parse else {
        return;
    };
    for (variant, text) in golden {
        assert_eq!(
            parse(text),
            Some(*variant),
            "{name}: parse({text:?}) must round-trip"
        );
    }
    for unknown in UNKNOWN {
        assert_eq!(parse(unknown), None, "{name}: parse({unknown:?}) leaked");
    }
}

#[test]
fn example_round_trips() {
    roundtrip(
        "Example",
        Example::ALL,
        &[(Example::One, "one"), (Example::TwoWords, "two-words")],
        Example::as_str,
        Some(Example::parse),
    );
}

#[test]
fn string_roundtrip_per_enum() {
    roundtrip(
        "RelationOperation",
        RelationOperation::ALL,
        &[
            (RelationOperation::Append, "append"),
            (RelationOperation::Replace, "replace"),
        ],
        RelationOperation::as_str,
        None, // parse is module-private; generated from this same list
    );

    roundtrip(
        "ExtensionContributionKind",
        ExtensionContributionKind::ALL,
        &[
            (ExtensionContributionKind::ModePattern, "modePattern"),
            (ExtensionContributionKind::Grammar, "grammar"),
            (ExtensionContributionKind::Command, "command"),
            (ExtensionContributionKind::KeyRoute, "keyRoute"),
            (ExtensionContributionKind::TextTransform, "textTransform"),
            (
                ExtensionContributionKind::CompletionProvider,
                "completionProvider",
            ),
            (
                ExtensionContributionKind::DecorationLayer,
                "decorationLayer",
            ),
            (
                ExtensionContributionKind::DiagnosticSource,
                "diagnosticSource",
            ),
            (ExtensionContributionKind::Analyzer, "analyzer"),
            (
                ExtensionContributionKind::IntelligenceProvider,
                "intelligenceProvider",
            ),
            (
                ExtensionContributionKind::PanelContribution,
                "panelContribution",
            ),
            (
                ExtensionContributionKind::ComponentContribution,
                "componentContribution",
            ),
            (
                ExtensionContributionKind::OverlayContribution,
                "overlayContribution",
            ),
            (ExtensionContributionKind::ThemeTokens, "themeTokens"),
            (ExtensionContributionKind::UiDesignSystem, "uiDesignSystem"),
            (ExtensionContributionKind::IconPack, "iconPack"),
            (ExtensionContributionKind::SduiRegion, "sduiRegion"),
            (ExtensionContributionKind::StatusItem, "statusItem"),
        ],
        ExtensionContributionKind::as_str,
        None, // parse is module-private; generated from this same list
    );

    roundtrip(
        "ChannelKind",
        ChannelKind::ALL,
        &[(ChannelKind::Npm, "npm"), (ChannelKind::Curl, "curl")],
        ChannelKind::as_str,
        None, // parse is module-private; generated from this same list
    );

    roundtrip(
        "FixtureKind",
        FixtureKind::ALL,
        &[
            (FixtureKind::LongLines, "long-lines"),
            (FixtureKind::ManyShortLines, "many-short-lines"),
            (FixtureKind::MixedUnicode, "mixed-unicode"),
            (FixtureKind::NewlineHeavy, "newline-heavy"),
        ],
        FixtureKind::as_str,
        Some(FixtureKind::parse),
    );

    roundtrip(
        "Appearance",
        Appearance::ALL,
        &[
            (Appearance::Light, "light"),
            (Appearance::Dark, "dark"),
            (Appearance::System, "system"),
        ],
        Appearance::as_str,
        Some(Appearance::parse),
    );

    roundtrip(
        "ComponentKind",
        ComponentKind::ALL,
        &[
            (ComponentKind::EditorView, "editorView"),
            (ComponentKind::Panel, "panel"),
            (ComponentKind::Label, "label"),
            (ComponentKind::Button, "button"),
            (ComponentKind::List, "list"),
            (ComponentKind::Flex, "flex"),
            (ComponentKind::Stack, "stack"),
            (ComponentKind::Overlay, "overlay"),
            (ComponentKind::Scroll, "scroll"),
            (ComponentKind::Portal, "portal"),
            (ComponentKind::StatusItem, "statusItem"),
            (ComponentKind::Dropdown, "dropdown"),
            (ComponentKind::Collapse, "collapse"),
            (ComponentKind::Modal, "modal"),
            (ComponentKind::TextInput, "textInput"),
            (ComponentKind::TabList, "tabList"),
        ],
        ComponentKind::as_str,
        Some(ComponentKind::parse),
    );

    roundtrip(
        "ThemeTokenType",
        ThemeTokenType::ALL,
        &[
            (ThemeTokenType::ColorRole, "color-role"),
            (ThemeTokenType::Spacing, "spacing"),
            (ThemeTokenType::Radius, "radius"),
            (ThemeTokenType::Typography, "typography"),
            (ThemeTokenType::Opacity, "opacity"),
            (ThemeTokenType::Dimension, "dimension"),
            (ThemeTokenType::Elevation, "elevation"),
            (ThemeTokenType::MotionDuration, "motion-duration"),
            (ThemeTokenType::ZLevel, "z-level"),
            (ThemeTokenType::Density, "density"),
        ],
        ThemeTokenType::as_str,
        Some(ThemeTokenType::parse),
    );

    roundtrip(
        "ElevationLevel",
        ElevationLevel::ALL,
        &[
            (ElevationLevel::None, "none"),
            (ElevationLevel::Raised, "raised"),
            (ElevationLevel::Overlay, "overlay"),
        ],
        ElevationLevel::as_str,
        Some(ElevationLevel::parse),
    );

    roundtrip(
        "ZLevel",
        ZLevel::ALL,
        &[
            (ZLevel::Base, "base"),
            (ZLevel::Panel, "panel"),
            (ZLevel::Overlay, "overlay"),
            (ZLevel::Modal, "modal"),
            (ZLevel::Tooltip, "tooltip"),
        ],
        ZLevel::as_str,
        Some(ZLevel::parse),
    );

    roundtrip(
        "DensityLevel",
        DensityLevel::ALL,
        &[
            (DensityLevel::Compact, "compact"),
            (DensityLevel::Default, "default"),
            (DensityLevel::Spacious, "spacious"),
        ],
        DensityLevel::as_str,
        Some(DensityLevel::parse),
    );

    roundtrip(
        "BorderStyle",
        BorderStyle::ALL,
        &[
            (BorderStyle::None, "none"),
            (BorderStyle::Solid, "solid"),
            (BorderStyle::Dashed, "dashed"),
            (BorderStyle::Dotted, "dotted"),
        ],
        BorderStyle::as_str,
        Some(BorderStyle::parse),
    );

    roundtrip(
        "OutlineStyle",
        OutlineStyle::ALL,
        &[(OutlineStyle::None, "none"), (OutlineStyle::Solid, "solid")],
        OutlineStyle::as_str,
        Some(OutlineStyle::parse),
    );

    roundtrip(
        "TransitionTiming",
        TransitionTiming::ALL,
        &[
            (TransitionTiming::Linear, "linear"),
            (TransitionTiming::EaseOut, "ease-out"),
            (TransitionTiming::SpringSnappy, "spring-snappy"),
            (TransitionTiming::SpringSmooth, "spring-smooth"),
        ],
        TransitionTiming::as_str,
        Some(TransitionTiming::parse),
    );

    roundtrip(
        "TransformPreset",
        TransformPreset::ALL,
        &[
            (TransformPreset::None, "none"),
            (TransformPreset::PressSubtle, "press-subtle"),
            (TransformPreset::PressShiftDown, "press-shift-down"),
            (TransformPreset::HoverLift, "hover-lift"),
        ],
        TransformPreset::as_str,
        Some(TransformPreset::parse),
    );

    roundtrip(
        "RecipeState",
        RecipeState::ALL,
        &[
            (RecipeState::Rest, "rest"),
            (RecipeState::Hover, "hover"),
            (RecipeState::Active, "active"),
            (RecipeState::Focus, "focus"),
            (RecipeState::Disabled, "disabled"),
            (RecipeState::Selected, "selected"),
            (RecipeState::Expanded, "expanded"),
            (RecipeState::Open, "open"),
            (RecipeState::Invalid, "invalid"),
        ],
        RecipeState::as_str,
        Some(RecipeState::parse),
    );
}
