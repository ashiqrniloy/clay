/// Package enable/load contract assembler for Phase 17.
///
/// `PackageRecord` is the full typed representation of a package that has passed
/// Clay-owned enable/load validation.  It is built by [`assemble_package_record`]
/// from a raw `package.json`-shaped [`serde_json::Value`], reusing the Phase 16.5
/// validators in [`crate::packages::manifest`], [`crate::packages::permissions`],
/// and [`crate::packages::commands`] rather than duplicating them.
///
/// Enable/load validation runs only at install/enable/reload time and is never
/// called from typing, paint, layout, scroll, or text-event handlers.
use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::packages::manifest::{ClayPackageManifest, PackageDiagnostic, validate_manifest_value};
use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
    COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS, COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
    COMPLETION_RESULT_MAX_ITEMS, LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS,
    LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
    SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
};
use crate::shell::{
    components::validate_component_kind,
    components::validate_style_variables,
    theme::{
        DensityLevel, ElevationLevel, MotionDuration, PackageThemeToken, ThemeTokenResolver,
        ThemeTokenType, ZLevel, core_fallback_matches_type, core_token_type, is_valid_dimension,
    },
};

use crate::editor::theme::{parse_hex_rgba, parse_override_token};
use crate::protocol::{
    CompletionItemTextFormat, DocumentFontRole, LanguageIntelligenceFeature, Modifiers, TokenType,
};

// ── Contribution descriptors ─────────────────────────────────────────────────

/// Inert descriptor for a command contribution declared by a package.
///
/// This is manifest-level metadata only; it does not grant handler authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContributionDescriptor {
    /// Package-prefixed command ID (e.g. `markdown.togglePreview`).
    pub id: String,
    /// Human-readable label for the command palette, help, and AI-agent lookup.
    pub display_name: String,
    /// Routing policy declared by the package for this command.
    pub routing_policy: String,
}

/// Inert descriptor for a configuration key contribution declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationContributionDescriptor {
    /// Package-prefixed configuration key (e.g. `markdown.preview.enabled`).
    pub key: String,
    /// JSON schema type: `"boolean"`, `"string"`, `"number"`, `"integer"`.
    pub value_type: String,
    /// Serialized JSON default value.
    pub default_value: Option<String>,
}

/// Inert descriptor for a key-routing override declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRoutingContributionDescriptor {
    /// Package-prefixed command ID that this key binding targets.
    pub command_id: String,
    /// Optional deterministic key binding token, e.g. `Ctrl+Shift+P`.
    pub key_binding: Option<String>,
    /// Optional routing policy used for ambiguity checks when a key is declared.
    pub routing_policy: Option<String>,
    /// Optional explicit priority. Equal key/routing/priority entries conflict.
    pub priority: Option<i32>,
}

/// Inert descriptor for a text-transform rule declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextTransformContributionDescriptor {
    /// Package-prefixed transform ID (e.g. `markdown.list-continuation`).
    pub transform_id: String,
    /// Known transform kind: `"enter-rule"`, `"tab-rule"`, `"pair-rule"`,
    /// `"comment-continuation"`, or `"autocomplete-trigger"`.
    pub kind: String,
}

/// Inert descriptor for an SDUI/status-bar contribution declared by a package.
///
/// SDUI actions embedded in the contribution must target declared commands;
/// they inherit the command permissions at execution time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SduiContributionDescriptor {
    /// Package-prefixed region/slot identifier.
    pub region_id: String,
    /// Display label for conflict diagnostics and AI-agent lookup.
    pub display_name: String,
    /// Estimated full snapshot payload for this inert package SDUI contribution.
    pub estimated_snapshot_bytes: usize,
    /// Estimated update payload for this inert package SDUI contribution.
    pub estimated_update_bytes: usize,
}

/// Inert descriptor for a decoration/render primitive declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationContributionDescriptor {
    /// Package-prefixed decoration/render primitive ID.
    pub primitive_id: String,
    /// Known decoration kind or style token namespace.
    pub kind: String,
}

/// Inert vocabulary style-map entry for one syntax capture. Maps a
/// Tree-sitter capture name directly to the Phase 18.15 two-axis vocabulary
/// (`TokenType` + `Modifiers`), plus an optional semantic document font role.
/// Packages never select a concrete font family, size, color, or renderer
/// value here — only the closed vocabulary the paint path resolves through the
/// active theme.
/// Default decoration priority for syntax-capture spans. Matches the
/// historical syntax priority so omitted styleMap priorities keep today's
/// rendering order.
pub const DEFAULT_SYNTAX_STYLE_PRIORITY: u16 = 70;

/// Highest priority a styleMap entry may declare. The syntax decoration
/// layer ranks below semantic in overlap resolution regardless of priority,
/// so this bound only orders captures within the syntax layer.
pub const MAX_SYNTAX_STYLE_PRIORITY: u16 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxStyleMapEntry {
    pub token_type: TokenType,
    pub modifiers: Modifiers,
    /// Preserved only for legacy style-token inputs. Native/vocabulary entries
    /// leave this empty and render from the closed two-axis fields above.
    pub scope: Option<String>,
    pub font_role: Option<DocumentFontRole>,
    /// Declarative capture priority: higher wins overlapping ranges within
    /// the syntax layer (narrow captures outrank broad prose). Defaults to
    /// [`DEFAULT_SYNTAX_STYLE_PRIORITY`].
    pub priority: u16,
}

/// Inert descriptor for a package-provided Tree-sitter syntax grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxGrammarContributionDescriptor {
    /// Package-owned contribution ID, defaulting to `<apiPrefix>.<languageId>`.
    pub id: String,
    /// Language identifier selected independently from the active major mode.
    pub language_id: String,
    /// Supported file extensions without leading dots.
    pub extensions: Vec<String>,
    /// Supported exact file names.
    pub file_names: Vec<String>,
    /// Server-owned grammar artifact kind; Phase 18.10 only accepts `tree-sitter-wasm`.
    pub grammar_kind: String,
    /// Package-root-confined grammar artifact path.
    pub grammar_path: String,
    /// Optional source/provenance label for the bundled artifact.
    pub grammar_source: Option<String>,
    /// Required Tree-sitter highlights query path.
    pub highlights_query_path: String,
    /// Optional locals query path.
    pub locals_query_path: Option<String>,
    /// Optional injections query path.
    pub injections_query_path: Option<String>,
    /// Tree-sitter capture name to known Clay style token and optional
    /// document-role override.
    pub style_map: BTreeMap<String, SyntaxStyleMapEntry>,
    /// Optional parser timeout budget in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional parse-window byte budget override.
    pub max_window_bytes: Option<usize>,
    /// Estimated bounded metadata payload for the contribution.
    pub estimated_payload_bytes: usize,
}

/// One bounded inert item in a package completion-provider contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItemContributionDescriptor {
    /// Picker label.
    pub label: String,
    /// Plain text or inert snippet syntax inserted on accept.
    pub insert_text: String,
    /// Optional picker detail; empty when omitted.
    pub detail: String,
    /// Client-local interpretation of `insert_text`.
    pub text_format: CompletionItemTextFormat,
}

/// Inert descriptor for a package-provided completion provider.
///
/// Completion provider contributions are inert metadata: provider ID, priority,
/// trigger characters, word-boundary rule, bounded static text/snippet items,
/// timeout, and item budgets. No callbacks, executable snippet transforms,
/// command side effects, or executable code are represented here. The package's
/// `completion-provider` permission is the authority gate; the descriptor
/// carries no extra authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionProviderContributionDescriptor {
    /// Package-prefixed provider ID (e.g. `<apiPrefix>.words`). Must not claim
    /// the reserved `clay.*` namespace.
    pub id: String,
    /// Higher priority providers run first when multiple match a trigger.
    pub priority: i32,
    /// Suppress strictly lower-priority providers when this provider is among
    /// the highest-priority matches. Defaults to false and grants no authority.
    pub exclusive: bool,
    /// Inert trigger characters that should request completion from this
    /// provider. Never executed.
    pub trigger_characters: Vec<String>,
    /// Inert word-boundary characters used by the provider to split tokens.
    pub word_boundary_chars: Vec<String>,
    /// Bounded static plain-text or client-expanded snippet items.
    pub items: Vec<CompletionItemContributionDescriptor>,
    /// Per-provider timeout in milliseconds. Must be within `1..=5000`.
    pub timeout_ms: u64,
    /// Per-provider cap on result item count. Must be within `1..=COMPLETION_RESULT_MAX_ITEMS`.
    pub max_items: usize,
    /// Estimated bounded metadata payload for the contribution.
    pub estimated_payload_bytes: usize,
}

/// Fixed launch metadata for one separately authorized language-server process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageServerContributionDescriptor {
    pub id: String,
    pub executable: String,
    pub args: Vec<String>,
    pub inherit_environment: Vec<String>,
}

/// Inert descriptor for a language-intelligence provider contribution.
///
/// Declares feature/mode/timeout metadata only. Handler binding happens at
/// registration time through a resolver-validated module/export token; no
/// callback, process, or language-server authority is granted here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageIntelligenceProviderContributionDescriptor {
    /// Package-prefixed provider ID.
    pub id: String,
    /// Modes this provider serves. Empty means all modes.
    pub modes: Vec<String>,
    /// Feature kinds this provider serves. Must be non-empty.
    pub features: Vec<LanguageIntelligenceFeature>,
    /// Higher priority providers are preferred.
    pub priority: i32,
    /// Optional package-root-relative module path for documentation/diagnostics.
    pub module: Option<String>,
    /// Export name expected when the package binds a handler token.
    pub export_name: String,
    /// Per-provider timeout in milliseconds.
    pub timeout_ms: u64,
    /// Estimated bounded metadata payload for the contribution.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a fixed slot-aware package UI panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPanelContributionDescriptor {
    /// Package-prefixed panel contribution ID.
    pub id: String,
    /// Fixed shell slot requested by the package (`left`, `right`, `top`, or `bottom`).
    pub slot: String,
    /// Package-prefixed root component ID for diagnostics and conflict handling.
    pub component_id: String,
    /// Estimated bounded snapshot payload for the inert panel declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a reusable package UI component root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiComponentContributionDescriptor {
    /// Package-prefixed component root ID.
    pub id: String,
    /// Validated Clay component catalog kind for the root.
    pub root_kind: String,
    /// Number of nodes in the validated component tree.
    pub component_count: usize,
    /// Number of typed style variables in the validated component tree.
    pub style_variable_count: usize,
    /// Estimated bounded snapshot payload for the inert component declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a transient package overlay contribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiOverlayContributionDescriptor {
    /// Package-prefixed overlay contribution ID.
    pub id: String,
    /// Overlay anchor (`working-area`, `active-pane`, `main`, or `pointer`).
    pub anchor: String,
    /// Focus policy (`none`, `restore`, or `trap`).
    pub focus_policy: String,
    /// Dismissal policy (`manual`, `escape`, `outside`, or `escape-or-outside`).
    pub dismissal_policy: String,
    /// Package-prefixed root component ID for diagnostics and conflict handling.
    pub component_id: String,
    /// Estimated bounded update payload for the inert overlay declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a package semantic theme token declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeTokenContributionDescriptor {
    /// Package-prefixed token name.
    pub token: String,
    /// Typed Clay token category.
    pub token_type: String,
    /// Same-type Clay core fallback token.
    pub fallback: String,
    /// Estimated bounded update payload for the inert token declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a Plan 046 task-5 text-style override declared by a
/// theme package. Stored as an `Eq` representation (RGBA bytes + bool flags)
/// so it composes with the `Eq`-deriving contribution inventory; converted to
/// the editor-side [`crate::editor::theme::TextStyleOverride`] at the point the
/// active theme is resolved into the `StyleRegistry`. This is pure style data:
/// no code/widgets/ops/CSS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextStyleOverrideDescriptor {
    /// Override target: a `TokenType` variant name or a base-UI color key.
    pub token: String,
    /// RGBA override, present only when the entry declares a `color`.
    pub color: Option<[u8; 4]>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

impl TextStyleOverrideDescriptor {
    /// Convert to the editor-side override consumed by
    /// [`crate::editor::theme::StyleRegistry::with_text_overrides`].
    #[allow(dead_code)] // wired by Plan 046 task 7 (setTheme) — keep live.
    pub(crate) fn to_override(&self) -> crate::editor::theme::TextStyleOverride {
        crate::editor::theme::TextStyleOverride {
            token: self.token.clone(),
            color: self
                .color
                .map(|[r, g, b, a]| masonry::peniko::Color::from_rgba8(r, g, b, a)),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strike: self.strike,
            provenance: self.provenance.clone(),
        }
    }
}

/// Inert descriptor for a Phase 20.1 typed UI design-token override declared by
/// a theme package via `clay.contributions.designTokens`. Stored as an `Eq`
/// representation (RGBA bytes, `f64`/`f32` bits, or a validated level name) so it
/// composes with the `Eq`-deriving contribution inventory; converted to the
/// protocol [`crate::protocol::WireDesignTokenValue`] when the active theme is
/// resolved (`setTheme`). Pure style data: no code/widgets/ops/CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignTokenOverrideDescriptor {
    /// Core Clay token name being overridden (e.g. `surface.hover`).
    pub token: String,
    /// Validated override value.
    pub value: DesignTokenValueDescriptor,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// Eq-friendly validated design-token value. Floats travel as `to_bits()` so the
/// descriptor stays `Eq`; the protocol layer reconstructs `f64`/`f32` from bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesignTokenValueDescriptor {
    /// `color-role` override as RGBA bytes.
    Color([u8; 4]),
    /// `spacing`/`radius`/`dimension`/`motion-duration` override as `f64::to_bits`.
    Scalar(u64),
    /// `opacity` override as `f32::to_bits`, validated to a finite `[0, 1]` value.
    Opacity(u32),
    /// `elevation`/`z-level`/`density` override as a validated level name.
    Level(String),
}

impl DesignTokenOverrideDescriptor {
    /// Convert to the protocol wire override shipped in
    /// [`crate::protocol::ActiveTheme::design_tokens`].
    pub(crate) fn to_wire(&self) -> crate::protocol::UiDesignTokenOverride {
        let value = match &self.value {
            DesignTokenValueDescriptor::Color(b) => {
                crate::protocol::WireDesignTokenValue::Color(*b)
            }
            DesignTokenValueDescriptor::Scalar(bits) => {
                crate::protocol::WireDesignTokenValue::Scalar(f64::from_bits(*bits))
            }
            DesignTokenValueDescriptor::Opacity(bits) => {
                crate::protocol::WireDesignTokenValue::Opacity(f32::from_bits(*bits))
            }
            DesignTokenValueDescriptor::Level(s) => {
                crate::protocol::WireDesignTokenValue::Level(s.clone())
            }
        };
        crate::protocol::UiDesignTokenOverride {
            token: self.token.clone(),
            value,
            provenance: self.provenance.clone(),
        }
    }
}

/// Inert descriptor for package-owned pointer/focus/action input metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputContributionDescriptor {
    /// Package-prefixed input contribution ID.
    pub id: String,
    /// Input scope (`component`, `panel`, or `overlay`).
    pub scope: String,
    /// Package-prefixed target component/panel/overlay component ID.
    pub component_id: String,
    /// Registered package command IDs this input metadata can emit.
    pub action_targets: Vec<String>,
    /// Estimated bounded update payload for the inert input declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for package UI state scope schema/lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStateScopeContributionDescriptor {
    /// Package-prefixed state scope ID.
    pub id: String,
    /// State scope (`package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, or `transient-overlay`).
    pub scope: String,
    /// State owner (`package`, `shell`, or `server`).
    pub owner: String,
    /// State lifetime (`session`, `workspace`, `document`, or `transient`).
    pub lifetime: String,
    /// Persistence contract (`none`, `client-local`, `server-canonical`, or `deferred`).
    pub persistence: String,
    /// Implementation status (`implemented` or `deferred`).
    pub implementation_status: String,
    /// Bounded schema kind (`boolean`, `number`, `string`, `enum`, or `object`).
    pub value_schema_kind: String,
    /// Optional package-prefixed target ID for pane/component/overlay scopes.
    pub target_id: Option<String>,
    /// Estimated bounded update payload for the inert state-scope declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for package layout/configuration default metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutOverrideContributionDescriptor {
    /// Package-prefixed target panel/component/input/token ID.
    pub target_id: String,
    /// Override property (`slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, or `fallback`).
    pub property: String,
    /// Precedence source for diagnostics.
    pub source: String,
    /// Estimated bounded update payload for the inert override declaration.
    pub estimated_payload_bytes: usize,
}

/// Inert descriptor for a package-owned typed option schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageOptionContributionDescriptor {
    /// Package-prefixed option name.
    pub option: String,
    /// Declared value type for the option schema.
    pub value_type: String,
    /// Serialized JSON default value when provided.
    pub default_value: Option<String>,
    /// Estimated bounded update payload for the option schema.
    pub estimated_payload_bytes: usize,
}

/// All inert primitive contribution descriptors declared by a package.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageContributions {
    pub commands: Vec<CommandContributionDescriptor>,
    pub configuration: Vec<ConfigurationContributionDescriptor>,
    pub key_routing: Vec<KeyRoutingContributionDescriptor>,
    pub text_transforms: Vec<TextTransformContributionDescriptor>,
    pub sdui: Vec<SduiContributionDescriptor>,
    pub decorations: Vec<DecorationContributionDescriptor>,
    pub syntax_grammars: Vec<SyntaxGrammarContributionDescriptor>,
    pub completion_providers: Vec<CompletionProviderContributionDescriptor>,
    pub language_servers: Vec<LanguageServerContributionDescriptor>,
    pub language_intelligence_providers: Vec<LanguageIntelligenceProviderContributionDescriptor>,
    pub ui_panels: Vec<UiPanelContributionDescriptor>,
    pub ui_components: Vec<UiComponentContributionDescriptor>,
    pub ui_overlays: Vec<UiOverlayContributionDescriptor>,
    pub theme_tokens: Vec<ThemeTokenContributionDescriptor>,
    /// Inert text-style overrides declared by theme packages (Plan 046 task 5).
    /// Converted to the editor `StyleRegistry` override shape when the active
    /// theme is resolved (task 7 `setTheme`).
    pub text_styles: Vec<TextStyleOverrideDescriptor>,
    /// Inert typed UI design-token overrides declared by theme packages
    /// (Phase 20.1). Converted to [`crate::protocol::UiDesignTokenOverride`] when
    /// the active theme is resolved (`setTheme`).
    pub design_tokens: Vec<DesignTokenOverrideDescriptor>,
    pub input_contributions: Vec<InputContributionDescriptor>,
    pub ui_state_scopes: Vec<UiStateScopeContributionDescriptor>,
    pub layout_overrides: Vec<LayoutOverrideContributionDescriptor>,
    pub package_options: Vec<PackageOptionContributionDescriptor>,
}

// ── Documentation and performance metadata ───────────────────────────────────

/// Path to the package's Clay JS API documentation index, declared in the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDocsMetadata {
    /// Relative path to the docs entry point (e.g. `./docs/index.md`).
    pub docs_path: String,
}

/// Performance metadata for the package's contribution to static payload budgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePerformanceMetadata {
    /// Estimated static manifest payload size in bytes, checked against
    /// [`crate::perf::budgets::BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`].
    pub estimated_manifest_bytes: usize,
}

/// A declared Clay JS API dependency of the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageApiDependency {
    /// Stable Clay JS API ID (e.g. `clay.modes.serverRegisterModePattern`).
    pub api_id: String,
}

// ── Package record ───────────────────────────────────────────────────────────

/// Full enable/load record for a validated Clay package.
///
/// A `PackageRecord` is produced by [`assemble_package_record`] only after all
/// Clay-owned validation rules pass.  It retains provenance on every accepted
/// contribution descriptor so later conflict handling, diagnostics, generated
/// documentation, and AI-agent discovery can identify the owning package.
#[derive(Debug, Clone, Eq)]
pub struct PackageRecord {
    /// The validated Phase 16.5 manifest (identity, prefix, permissions, modes, entries).
    pub manifest: ClayPackageManifest,
    /// Declared inert primitive contributions.
    pub contributions: PackageContributions,
    /// Documentation metadata path, required for every enabled package.
    pub docs: PackageDocsMetadata,
    /// Static performance metadata, checked against payload budgets at enable time.
    pub performance: PackagePerformanceMetadata,
    /// Declared Clay JS API dependencies.
    pub api_dependencies: Vec<PackageApiDependency>,
    /// Host-owned runtime trust domain. Defaults to `ThirdParty` at assembly;
    /// `PackageService::enable` upgrades it to `Trusted` only after the
    /// bundled inventory verifies exact name, version, canonical root, and
    /// manifest integrity. Crate-internal: never exposed to JavaScript.
    pub(crate) runtime_domain: crate::packages::bundled::RuntimeDomain,
}

impl PartialEq for PackageRecord {
    /// Record identity is the validated manifest content. `runtime_domain` is
    /// a host stamp derived from install provenance, so it is excluded:
    /// comparing a caller-assembled record against an enabled host record must
    /// not depend on who assembled it.
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.contributions == other.contributions
            && self.docs == other.docs
            && self.performance == other.performance
            && self.api_dependencies == other.api_dependencies
    }
}

// ── Error types ──────────────────────────────────────────────────────────────

/// Structured enable/load diagnostic produced when a package fails contract
/// validation.  Every field is optional because some errors occur before the
/// package identity is fully parsed.
///
/// String fields use `Box<str>` (not `String`) to keep the `Err`-variant under
/// clippy's `result_large_err` 128-byte threshold: `Box<str>` is a 16-byte fat
/// pointer vs `String`'s 24-byte (ptr+len+cap), and `Option<Box<str>>` is 24
/// bytes vs `Option<String>`'s 32. These diagnostics are constructed once and
/// read/displayed, never mutated in place, so the loss of growability is free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRecordError {
    pub package_name: Option<Box<str>>,
    pub package_version: Option<Box<str>>,
    pub api_prefix: Option<Box<str>>,
    /// The contribution ID that failed (command, config key, transform, …) when applicable.
    pub contribution_id: Option<Box<str>>,
    pub rule: PackageRecordRule,
    pub message: Box<str>,
}

/// Validation rule that caused a [`PackageRecordError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRecordRule {
    /// A required field (entry, docs, performance, contributions block) is absent.
    MissingRequiredField,
    /// The static payload estimate exceeds an advisory budget constant.
    PayloadBudgetExceeded,
    /// A contribution descriptor is malformed or references an undeclared ID.
    InvalidContributionDescriptor,
    /// A contribution requires a permission not declared in `clay.permissions`.
    UndeclaredPermissionForContribution,
    /// A contribution ID claims the reserved `clay.*` namespace.
    ReservedClayIdInContribution,
    /// A duplicate contribution ID (command, config key, region …) within the package.
    DuplicateContributionId,
    /// A declared API dependency ID is empty or malformed.
    InvalidApiDependency,
    /// Re-exported from the manifest layer.
    ManifestValidationFailed,
}

impl PackageRecordError {
    fn from_manifest_diagnostic(d: PackageDiagnostic) -> Self {
        Self {
            package_name: d.package_name.map(String::into_boxed_str),
            package_version: d.package_version.map(String::into_boxed_str),
            api_prefix: d.api_prefix.map(String::into_boxed_str),
            contribution_id: None,
            rule: PackageRecordRule::ManifestValidationFailed,
            message: d.message.into_boxed_str(),
        }
    }
}

// ── Assembler ────────────────────────────────────────────────────────────────

/// Assemble a [`PackageRecord`] from a raw `package.json`-shaped JSON value.
///
/// This is the Clay-owned enable/load contract validator.  It runs at
/// install/enable/reload time and must never be called from typing, paint,
/// layout, scroll, or text-event handlers.
///
/// Steps:
/// 1. Validate the Phase 16.5 manifest (identity, prefix, permissions, modes).
/// 2. Parse and validate contribution descriptors from `clay.contributions`.
/// 3. Validate the required `clay.docs` path.
/// 4. Validate `clay.performance` metadata against advisory budgets.
/// 5. Parse `clay.apiDependencies` stubs.
pub fn assemble_package_record(value: &Value) -> Result<PackageRecord, PackageRecordError> {
    // Step 1: reuse Phase 16.5 manifest validator.
    let manifest =
        validate_manifest_value(value).map_err(PackageRecordError::from_manifest_diagnostic)?;

    let clay = value
        .get("clay")
        .and_then(Value::as_object)
        .expect("clay block already validated by validate_manifest_value");

    let ctx = ErrorContext {
        package_name: Some(manifest.name.clone()),
        package_version: Some(manifest.version.clone()),
        api_prefix: Some(manifest.clay.api_prefix.clone()),
    };

    // Step 2: contribution descriptors (optional block; empty = no contributions).
    let contributions = match clay.get("contributions") {
        Some(contrib_value) => parse_contributions(
            contrib_value,
            &manifest.clay.api_prefix,
            &manifest.clay.permissions,
            &manifest.clay.modes,
            &ctx,
        )?,
        None => PackageContributions::default(),
    };
    if manifest
        .clay
        .permissions
        .contains(&PackagePermission::LanguageServer)
        && contributions.language_servers.is_empty()
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "packages requesting `language-server` must declare at least one clay.contributions.languageServers entry",
        ));
    }

    // Step 3: required docs path.
    let docs = parse_docs_metadata(clay.get("docs"), &ctx)?;

    // Step 4: performance metadata.
    let performance = parse_performance_metadata(clay.get("performance"), value, &ctx)?;

    // Step 5: API dependencies (optional).
    let api_dependencies = parse_api_dependencies(clay.get("apiDependencies"), &ctx)?;
    validate_api_dependency_permissions(&api_dependencies, &manifest.clay.permissions, &ctx)?;

    Ok(PackageRecord {
        manifest,
        contributions,
        docs,
        performance,
        api_dependencies,
        runtime_domain: crate::packages::bundled::RuntimeDomain::ThirdParty,
    })
}

// ── Internal parsers ─────────────────────────────────────────────────────────

fn parse_docs_metadata(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<PackageDocsMetadata, PackageRecordError> {
    match value {
        Some(Value::String(path)) if !path.trim().is_empty() => Ok(PackageDocsMetadata {
            docs_path: path.clone(),
        }),
        _ => Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.docs must be a non-empty path to the package documentation index",
        )),
    }
}

fn parse_performance_metadata(
    value: Option<&Value>,
    raw_manifest: &Value,
    ctx: &ErrorContext,
) -> Result<PackagePerformanceMetadata, PackageRecordError> {
    let estimated_manifest_bytes = match value {
        Some(Value::Object(perf)) => {
            match perf.get("estimatedManifestBytes") {
                Some(Value::Number(n)) => n
                    .as_u64()
                    .map(|v| v as usize)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::MissingRequiredField,
                            None,
                            "clay.performance.estimatedManifestBytes must be a non-negative integer",
                        )
                    })?,
                _ => return Err(ctx.error(
                    PackageRecordRule::MissingRequiredField,
                    None,
                    "clay.performance.estimatedManifestBytes must be declared as a non-negative integer",
                )),
            }
        }
        // When no performance block is present, derive the estimate from the raw payload size.
        None => {
            serde_json::to_vec(raw_manifest)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX)
        }
        _ => return Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.performance must be an object when present",
        )),
    };

    if estimated_manifest_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
        return Err(ctx.error(
            PackageRecordRule::PayloadBudgetExceeded,
            None,
            format!(
                "package estimated manifest payload ({estimated_manifest_bytes} bytes) exceeds \
                 BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
            ),
        ));
    }

    Ok(PackagePerformanceMetadata {
        estimated_manifest_bytes,
    })
}

fn parse_api_dependencies(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<Vec<PackageApiDependency>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidApiDependency,
            None,
            "clay.apiDependencies must be an array of Clay JS API ID strings when present",
        ));
    };
    let mut deps = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(api_id) = entry.as_str() else {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be strings",
            ));
        };
        if api_id.trim().is_empty() {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be non-empty strings",
            ));
        }
        deps.push(PackageApiDependency {
            api_id: api_id.to_string(),
        });
    }
    Ok(deps)
}

fn validate_api_dependency_permissions(
    dependencies: &[PackageApiDependency],
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    for dependency in dependencies {
        let required = match dependency.api_id.as_str() {
            "clay.packages.serverLoadPackage" => None,
            "clay.behavior.buildCodeEditingManifest" => None,
            "clay.modes.serverRegisterModePattern" => Some(PackagePermission::ModeRegistration),
            "clay.modes.serverActivateMajorMode" => Some(PackagePermission::ModeActivation),
            "clay.commands.serverRegisterCommand" => Some(PackagePermission::CommandRegistration),
            "clay.completion.serverRegisterCompletionProvider" => {
                Some(PackagePermission::CompletionProvider)
            }
            "clay.completion.completionTriggerCharactersFromEditorRules" => None,
            "clay.parse.serverRegisterParseHandler"
            | "clay.language.serverRegisterDocumentAnalyzer" => {
                Some(PackagePermission::ParseDocument)
            }
            "clay.language-server.startLanguageServerSession" => {
                Some(PackagePermission::LanguageServer)
            }
            "clay.decorations.serverPublishDecorations"
            | "clay.diagnostics.serverPublishDiagnostics" => {
                Some(PackagePermission::RenderDecorations)
            }
            "clay.syntax.serverRegisterSyntaxGrammar" => Some(PackagePermission::ParseDocument),
            "clay.ui.serverRegisterPanelContribution"
            | "clay.ui.serverRegisterComponentContribution"
            | "clay.ui.serverRegisterTransientOverlayContribution"
            | "clay.ui.serverRegisterThemeToken"
            | "clay.ui.serverRegisterInputContribution"
            | "clay.ui.serverRegisterUiStateScope" => None,
            "clay.ui.serverSetLayoutOverride" | "clay.configuration.setPackageOption" => {
                Some(PackagePermission::PackageConfiguration)
            }
            // Read-only Git discovery and inert SDUI publication are
            // server-owned: they require no package permission because Git
            // authority never reaches package code.
            "clay.git.serverListGitStatuses"
            | "clay.git.serverRefreshGitStatus"
            | "clay.sdui.publishTree" => None,
            _ => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidApiDependency,
                    Some(&dependency.api_id),
                    format!(
                        "unknown Clay JS API dependency `{}`; packages must list documented Clay API IDs",
                        dependency.api_id
                    ),
                ));
            }
        };

        if let Some(required) = required
            && !permissions.contains(&required)
        {
            return Err(ctx.error(
                    PackageRecordRule::UndeclaredPermissionForContribution,
                    Some(&dependency.api_id),
                    format!(
                        "API dependency `{}` requires the `{}` permission to be declared in clay.permissions",
                        dependency.api_id,
                        required.as_str()
                    ),
                ));
        }
    }

    Ok(())
}

fn parse_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    package_modes: &[String],
    ctx: &ErrorContext,
) -> Result<PackageContributions, PackageRecordError> {
    let Value::Object(map) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions must be an object when present",
        ));
    };

    let commands = match map.get("commands") {
        Some(v) => parse_command_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let configuration = match map.get("configuration") {
        Some(v) => parse_configuration_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let key_routing = match map.get("keyRouting") {
        Some(v) => parse_key_routing_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let text_transforms = match map.get("textTransforms") {
        Some(v) => parse_text_transform_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let sdui = match map.get("sdui") {
        Some(v) => parse_sdui_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let decorations = match map.get("decorations") {
        Some(v) => parse_decoration_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let syntax_grammars = match map.get("syntaxGrammars") {
        Some(v) => parse_syntax_grammar_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let completion_providers = match map.get("completionProviders") {
        Some(v) => parse_completion_provider_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let language_servers = match map.get("languageServers") {
        Some(v) => parse_language_server_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let language_intelligence_providers = match map.get("languageIntelligenceProviders") {
        Some(v) => {
            parse_language_intelligence_provider_contributions(v, api_prefix, permissions, ctx)?
        }
        None => Vec::new(),
    };
    let theme_tokens = match map.get("themeTokens") {
        Some(v) => parse_theme_token_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let text_styles = match map.get("textStyles") {
        Some(v) => parse_text_style_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let design_tokens = match map.get("designTokens") {
        Some(v) => parse_design_token_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let registered_command_ids: Vec<String> =
        commands.iter().map(|command| command.id.clone()).collect();
    let theme_resolver = theme_resolver_for_package_tokens(&theme_tokens);
    let (ui_panels, ui_components, ui_overlays) = match map.get("ui") {
        Some(v) => {
            parse_ui_contributions(v, api_prefix, &registered_command_ids, &theme_resolver, ctx)?
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let input_contributions = match map.get("input") {
        Some(v) => {
            parse_input_contributions(v, api_prefix, package_modes, &registered_command_ids, ctx)?
        }
        None => Vec::new(),
    };
    let ui_state_scopes = match map.get("uiStateScopes") {
        Some(v) => parse_ui_state_scope_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let package_options = match map.get("packageOptions") {
        Some(v) => parse_package_option_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let layout_overrides = match map.get("layoutOverrides") {
        Some(v) => parse_layout_override_contributions(
            v,
            api_prefix,
            &registered_command_ids,
            &theme_tokens,
            &input_contributions,
            permissions,
            ctx,
        )?,
        None => Vec::new(),
    };

    Ok(PackageContributions {
        commands,
        configuration,
        key_routing,
        text_transforms,
        sdui,
        decorations,
        syntax_grammars,
        completion_providers,
        language_servers,
        language_intelligence_providers,
        ui_panels,
        ui_components,
        ui_overlays,
        theme_tokens,
        text_styles,
        design_tokens,
        input_contributions,
        ui_state_scopes,
        layout_overrides,
        package_options,
    })
}

fn parse_command_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<CommandContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.commands must be an array",
        ));
    };

    // Commands require `command-registration` permission.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::CommandRegistration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "command contributions require the `command-registration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "command contribution entries must be objects",
            )
        })?;

        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "command contribution must include a non-empty `id` field",
                )
            })?;

        if id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(id),
                "command contribution IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "command contribution IDs must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "command contribution IDs must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `displayName` field",
                )
            })?;

        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `routingPolicy` field",
                )
            })?;

        // Reject routing policies that would grant built-in client-edit authority.
        if matches!(
            routing_policy,
            "client-first-predictable" | "client-first-requires-ack"
        ) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "package command contributions cannot declare built-in client-edit routing policies",
            ));
        }

        descriptors.push(CommandContributionDescriptor {
            id: id.to_string(),
            display_name: display_name.to_string(),
            routing_policy: routing_policy.to_string(),
        });
    }

    Ok(descriptors)
}

fn parse_configuration_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<ConfigurationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.configuration must be an array",
        ));
    };

    // Behavior-changing configuration requires `package-configuration`.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "configuration contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_keys = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "configuration contribution entries must be objects",
            )
        })?;

        let key = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "configuration contribution must include a non-empty `key` field",
                )
            })?;

        if key.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(key),
                "configuration contribution keys cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(key, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration keys must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_keys.insert(key.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(key),
                "configuration contribution keys must be unique within a package",
            ));
        }

        let value_type = obj
            .get("type")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(key),
                    "configuration contribution must include a `type` field",
                )
            })?;

        if !matches!(value_type, "boolean" | "string" | "number" | "integer") {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration contribution `type` must be one of: boolean, string, number, integer",
            ));
        }

        let default_value = obj
            .get("default")
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        descriptors.push(ConfigurationContributionDescriptor {
            key: key.to_string(),
            value_type: value_type.to_string(),
            default_value,
        });
    }

    Ok(descriptors)
}

fn parse_key_routing_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<KeyRoutingContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.keyRouting must be an array",
        ));
    };

    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "key routing contribution entries must be objects",
            )
        })?;

        let command_id = obj
            .get("commandId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "key routing contribution must include a non-empty `commandId` field",
                )
            })?;

        if command_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(command_id),
                "key routing contributions cannot target reserved clay.* command IDs",
            ));
        }
        if !is_package_owned_id(command_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(command_id),
                "key routing contribution commandId must use the package apiPrefix namespace",
            ));
        }

        let key_binding = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let priority = match obj.get("priority") {
            Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(command_id),
                    "key routing contribution priority must be an integer when present",
                ));
            }
            None => None,
        };

        descriptors.push(KeyRoutingContributionDescriptor {
            command_id: command_id.to_string(),
            key_binding,
            routing_policy,
            priority,
        });
    }

    Ok(descriptors)
}

fn parse_text_transform_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<TextTransformContributionDescriptor>, PackageRecordError> {
    const VALID_KINDS: &[&str] = &[
        "enter-rule",
        "tab-rule",
        "pair-rule",
        "comment-continuation",
        "autocomplete-trigger",
    ];

    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.textTransforms must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text transform contribution entries must be objects",
            )
        })?;

        // Reject any executable fields.
        for forbidden in &["javascriptCallback", "code", "clientHook", "drawCallback"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "text transform contributions are inert manifest data and must not include `{forbidden}`"
                    ),
                ));
            }
        }

        let transform_id = obj
            .get("transformId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "text transform contribution must include a non-empty `transformId` field",
                )
            })?;

        if transform_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(transform_id),
                "text transform IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(transform_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                "text transform IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(transform_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(transform_id),
                "text transform IDs must be unique within a package",
            ));
        }

        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(transform_id),
                    "text transform contribution must include a `kind` field",
                )
            })?;

        if !VALID_KINDS.contains(&kind) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                format!(
                    "text transform `kind` must be one of: {}",
                    VALID_KINDS.join(", ")
                ),
            ));
        }

        descriptors.push(TextTransformContributionDescriptor {
            transform_id: transform_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

fn parse_sdui_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<SduiContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.sdui must be an array",
        ));
    };

    let mut seen_regions = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "SDUI contribution entries must be objects",
            )
        })?;

        // Reject executable widget fields.
        for forbidden in &[
            "widgetCallback",
            "clientJavaScript",
            "drawCallback",
            "nativeHandle",
        ] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "SDUI contributions must not include client-side or native-widget fields (`{forbidden}`)"
                    ),
                ));
            }
        }

        let region_id = obj
            .get("regionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "SDUI contribution must include a non-empty `regionId` field",
                )
            })?;

        if region_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(region_id),
                "SDUI contribution regionIds cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(region_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(region_id),
                "SDUI contribution regionIds must use the package apiPrefix namespace",
            ));
        }
        if !seen_regions.insert(region_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(region_id),
                "SDUI contribution regionIds must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(region_id),
                    "SDUI contribution must include a non-empty `displayName` field",
                )
            })?;

        let estimated_snapshot_bytes = obj
            .get("estimatedSnapshotBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                serde_json::to_vec(entry)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            });
        if estimated_snapshot_bytes > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI snapshot payload estimate ({estimated_snapshot_bytes} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        let estimated_update_bytes = obj
            .get("estimatedUpdateBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(estimated_snapshot_bytes);
        if estimated_update_bytes > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI update payload estimate ({estimated_update_bytes} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        descriptors.push(SduiContributionDescriptor {
            region_id: region_id.to_string(),
            display_name: display_name.to_string(),
            estimated_snapshot_bytes,
            estimated_update_bytes,
        });
    }

    Ok(descriptors)
}

fn parse_decoration_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<DecorationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.decorations must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "decoration contribution entries must be objects",
            )
        })?;
        for forbidden in &["drawCallback", "clientJavaScript", "nativeHandle"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "decoration contributions are inert and must not include `{forbidden}`"
                    ),
                ));
            }
        }
        let primitive_id = obj
            .get("primitiveId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "decoration contribution must include a non-empty `primitiveId` field",
                )
            })?;
        if primitive_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(primitive_id),
                "decoration primitive IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(primitive_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(primitive_id),
                "decoration primitive IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(primitive_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(primitive_id),
                "decoration primitive IDs must be unique within a package",
            ));
        }
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(primitive_id),
                    "decoration contribution must include a non-empty `kind` field",
                )
            })?;
        descriptors.push(DecorationContributionDescriptor {
            primitive_id: primitive_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

fn parse_syntax_grammar_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<SyntaxGrammarContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.syntaxGrammars", ctx)?;
    if !entries.is_empty()
        && (!permissions.contains(&PackagePermission::ParseDocument)
            || !permissions.contains(&PackagePermission::RenderDecorations))
    {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "syntax grammar contributions require `parse-document` and `render-decorations` permissions",
        ));
    }
    if !entries.is_empty()
        && !ctx
            .package_name
            .as_deref()
            .is_some_and(|package_name| package_name.starts_with("@clay/"))
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "Phase 18.10 syntax grammar contributions are first-party-only; arbitrary third-party grammar packages are not accepted",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut seen_languages = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "syntax grammar metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_syntax_grammar_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "syntax grammar contribution", ctx)?;
        let language_id = required_str_field(obj, "languageId", ctx)?;
        if !is_valid_language_id(language_id) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(language_id),
                "syntax grammar languageId must use lowercase letters, digits, hyphen, underscore, plus, or dot",
            ));
        }
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("{api_prefix}.{language_id}"));
        if id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(&id),
                "syntax grammar IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(&id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "syntax grammar IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(id.clone()) || !seen_languages.insert(language_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "syntax grammar IDs and languageIds must be unique within a package",
            ));
        }

        let patterns = obj
            .get("filePatterns")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include filePatterns object",
                )
            })?;
        let extensions =
            optional_string_vec(patterns.get("extensions"), "filePatterns.extensions", ctx)?;
        for extension in &extensions {
            if extension.starts_with('.')
                || extension.contains('/')
                || extension.contains('\\')
                || extension.trim().is_empty()
            {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(extension),
                    "syntax grammar extensions must be bare extension names without path separators or leading dots",
                ));
            }
        }
        let file_names =
            optional_string_vec(patterns.get("fileNames"), "filePatterns.fileNames", ctx)?;
        if extensions.is_empty() && file_names.is_empty() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "syntax grammar filePatterns must declare extensions or fileNames",
            ));
        }

        let grammar = obj
            .get("grammar")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include grammar object",
                )
            })?;
        let grammar_kind = required_str_field(grammar, "kind", ctx)?;
        let (grammar_path, grammar_source) = if grammar_kind == "native" {
            // Tier 1: the grammar is compiled into the server binary; packages
            // declare only the native crate identifier. No wasm asset path is
            // accepted, and the first-party registry is the sole source of
            // truth for which native crates exist.
            if grammar.get("path").is_some() {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "native (Tier 1) syntax grammars are compiled into the server; a `path` is not accepted",
                ));
            }
            let source = required_str_field(grammar, "source", ctx)?;
            (String::new(), source.to_string())
        } else if grammar_kind == "tree-sitter-wasm" {
            let grammar_path = required_str_field(grammar, "path", ctx)?;
            validate_package_asset_path(grammar_path, "grammar.path", Some(".wasm"), ctx)?;
            let source = grammar
                .get("source")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned);
            (grammar_path.to_string(), source.unwrap_or_default())
        } else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(grammar_kind),
                "Phase 18.18 syntax grammars support kind `native` (Tier 1 compiled-in) or `tree-sitter-wasm` (Tier 2 host-side); other kinds are not accepted",
            ));
        };
        let grammar_source = if grammar_source.is_empty() {
            None
        } else {
            Some(grammar_source)
        };

        let queries = obj
            .get("queries")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include queries object",
                )
            })?;
        let highlights_query_path = required_str_field(queries, "highlights", ctx)?;
        validate_package_asset_path(
            highlights_query_path,
            "queries.highlights",
            Some(".scm"),
            ctx,
        )?;
        let locals_query_path = optional_asset_path(queries.get("locals"), "queries.locals", ctx)?;
        let injections_query_path =
            optional_asset_path(queries.get("injections"), "queries.injections", ctx)?;
        let style_map = parse_syntax_style_map(obj.get("styleMap"), &id, ctx)?;

        let budgets = obj.get("budgets").and_then(Value::as_object);
        let timeout_ms = optional_u64_budget(budgets, "timeoutMs", &id, ctx)?;
        let max_window_bytes = optional_usize_budget(budgets, "maxWindowBytes", &id, ctx)?;

        descriptors.push(SyntaxGrammarContributionDescriptor {
            id,
            language_id: language_id.to_string(),
            extensions,
            file_names,
            grammar_kind: grammar_kind.to_string(),
            grammar_path: grammar_path.to_string(),
            grammar_source,
            highlights_query_path: highlights_query_path.to_string(),
            locals_query_path,
            injections_query_path,
            style_map,
            timeout_ms,
            max_window_bytes,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

fn parse_completion_provider_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<CompletionProviderContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.completionProviders", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::CompletionProvider) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "completion provider contributions require `completion-provider` permission",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "completion provider metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_completion_provider_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "completion provider contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "completion provider IDs must be unique within a package",
            ));
        }
        let priority = obj.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
        let exclusive = match obj.get("exclusive") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "completion provider exclusive must be a boolean",
                ));
            }
        };
        let trigger_characters =
            optional_string_vec(obj.get("triggerCharacters"), "triggerCharacters", ctx)?;
        let word_boundary_chars =
            optional_string_vec(obj.get("wordBoundaryChars"), "wordBoundaryChars", ctx)?;
        let items = parse_completion_items(obj.get("items"), &id, ctx)?;
        if items.len() > COMPLETION_RESULT_MAX_ITEMS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider items must contain at most {COMPLETION_RESULT_MAX_ITEMS} entries"
                ),
            ));
        }
        let mut unique_labels = HashSet::new();
        if let Some(item) = items
            .iter()
            .find(|item| !unique_labels.insert(item.label.as_str()))
        {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                format!("completion provider item `{}` is duplicated", item.label),
            ));
        }
        let has_plain_text = items
            .iter()
            .any(|item| item.text_format == CompletionItemTextFormat::PlainText);
        let has_snippets = items
            .iter()
            .any(|item| item.text_format == CompletionItemTextFormat::Snippet);
        if has_plain_text && has_snippets {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "completion provider items cannot mix plainText and snippet formats; use separate providers",
            ));
        }
        let timeout_ms = optional_u64_budget(
            obj.get("budgets").and_then(Value::as_object),
            "timeoutMs",
            &id,
            ctx,
        )?
        .unwrap_or(500);
        let max_items = optional_usize_budget(
            obj.get("budgets").and_then(Value::as_object),
            "maxItems",
            &id,
            ctx,
        )?
        .unwrap_or(64);
        if timeout_ms == 0 || timeout_ms > 5_000 {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "completion provider timeoutMs must be within 1..=5000",
            ));
        }
        if max_items == 0 || max_items > COMPLETION_RESULT_MAX_ITEMS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider maxItems must be within 1..={COMPLETION_RESULT_MAX_ITEMS}"
                ),
            ));
        }
        if items.len() > max_items {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider item count ({}) exceeds its maxItems budget ({max_items})",
                    items.len()
                ),
            ));
        }
        descriptors.push(CompletionProviderContributionDescriptor {
            id,
            priority,
            exclusive,
            trigger_characters,
            word_boundary_chars,
            items,
            timeout_ms,
            max_items,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

fn parse_language_server_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LanguageServerContributionDescriptor>, PackageRecordError> {
    const MAX_SERVERS: usize = 8;
    const MAX_ARGS: usize = 32;
    const MAX_ENVIRONMENT_NAMES: usize = 32;
    const MAX_VALUE_BYTES: usize = 4096;

    let entries = array_field(value, "clay.contributions.languageServers", ctx)?;
    if entries.len() > MAX_SERVERS {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("languageServers must contain at most {MAX_SERVERS} entries"),
        ));
    }
    if !entries.is_empty() && !permissions.contains(&PackagePermission::LanguageServer) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "language-server contributions require the `language-server` capability",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = object_field(entry, "language-server contribution", ctx)?;
        for field in obj.keys() {
            if !matches!(
                field.as_str(),
                "id" | "executable" | "args" | "inheritEnvironment"
            ) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!("language-server contribution field `{field}` is not allowed"),
                ));
            }
        }
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if id.len() > 128 || !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                if id.len() > 128 {
                    PackageRecordRule::InvalidContributionDescriptor
                } else {
                    PackageRecordRule::DuplicateContributionId
                },
                Some(&id),
                "language-server contribution IDs must be unique and at most 128 bytes",
            ));
        }
        let executable = obj
            .get("executable")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-server executable must be a non-empty string",
                )
            })?;
        if executable.len() > MAX_VALUE_BYTES || executable.chars().any(char::is_control) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "language-server executable must be at most 4096 bytes without control characters",
            ));
        }
        let args =
            bounded_string_array(obj.get("args"), "args", MAX_ARGS, MAX_VALUE_BYTES, &id, ctx)?;
        let inherit_environment = bounded_string_array(
            obj.get("inheritEnvironment"),
            "inheritEnvironment",
            MAX_ENVIRONMENT_NAMES,
            128,
            &id,
            ctx,
        )?;
        let mut seen_environment = HashSet::new();
        for name in &inherit_environment {
            let valid = name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            });
            if !valid || !seen_environment.insert(name.as_str()) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "inheritEnvironment names must be unique ASCII environment variable names",
                ));
            }
        }
        descriptors.push(LanguageServerContributionDescriptor {
            id,
            executable: executable.to_string(),
            args,
            inherit_environment,
        });
    }
    Ok(descriptors)
}

fn parse_language_intelligence_provider_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LanguageIntelligenceProviderContributionDescriptor>, PackageRecordError> {
    const MAX_PROVIDERS: usize = 32;
    const MAX_MODES: usize = 32;
    const MAX_FEATURES: usize = 4;

    let entries = array_field(
        value,
        "clay.contributions.languageIntelligenceProviders",
        ctx,
    )?;
    if entries.len() > MAX_PROVIDERS {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("languageIntelligenceProviders must contain at most {MAX_PROVIDERS} entries"),
        ));
    }
    if !entries.is_empty() && !permissions.contains(&PackagePermission::ParseDocument) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "language-intelligence provider contributions require `parse-document` permission",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "language-intelligence provider metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_language_intelligence_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "language-intelligence provider contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "language-intelligence provider IDs must be unique within a package",
            ));
        }
        let modes = optional_string_vec(obj.get("modes"), "modes", ctx)?;
        if modes.len() > MAX_MODES {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider modes must contain at most {MAX_MODES} entries"
                ),
            ));
        }
        let features = parse_language_intelligence_features(obj.get("features"), &id, ctx)?;
        if features.is_empty() || features.len() > MAX_FEATURES {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider features must contain 1..={MAX_FEATURES} entries"
                ),
            ));
        }
        let priority = obj.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
        let module = match obj.get("module") {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) => {
                if path.is_empty()
                    || path.len() > 512
                    || path.chars().any(char::is_control)
                    || path.contains("..")
                    || path.starts_with('/')
                {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(&id),
                        "language-intelligence provider module must be a bounded package-relative path",
                    ));
                }
                Some(path.clone())
            }
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-intelligence provider module must be a string path",
                ));
            }
        };
        let export_name = match obj.get("exportName") {
            None | Some(Value::Null) => "provideLanguageIntelligence".to_string(),
            Some(Value::String(name))
                if !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control) =>
            {
                name.clone()
            }
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-intelligence provider exportName must be a non-empty bounded string",
                ));
            }
        };
        let timeout_ms = optional_u64_budget(
            obj.get("budgets").and_then(Value::as_object),
            "timeoutMs",
            &id,
            ctx,
        )?
        .or_else(|| obj.get("timeoutMs").and_then(Value::as_u64))
        .unwrap_or(LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS);
        if timeout_ms == 0 || timeout_ms > LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider timeoutMs must be within 1..={LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS}"
                ),
            ));
        }
        descriptors.push(LanguageIntelligenceProviderContributionDescriptor {
            id,
            modes,
            features,
            priority,
            module,
            export_name,
            timeout_ms,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

fn parse_language_intelligence_features(
    value: Option<&Value>,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<LanguageIntelligenceFeature>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "language-intelligence provider features must be an array of strings",
        ));
    };
    let mut features = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let Some(name) = value.as_str() else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(contribution_id),
                "language-intelligence provider features must be strings",
            ));
        };
        let feature = match name {
            "hover" | "Hover" => LanguageIntelligenceFeature::Hover,
            "definition" | "goToDefinition" | "GoToDefinition" => {
                LanguageIntelligenceFeature::GoToDefinition
            }
            "codeAction" | "CodeAction" => LanguageIntelligenceFeature::CodeAction,
            "signatureHelp" | "SignatureHelp" => LanguageIntelligenceFeature::SignatureHelp,
            other => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(contribution_id),
                    format!("unsupported language-intelligence feature `{other}`"),
                ));
            }
        };
        if seen.insert(feature) {
            features.push(feature);
        }
    }
    Ok(features)
}

fn reject_language_intelligence_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "handler"
                        | "callback"
                        | "function"
                        | "clientJavaScript"
                        | "nativeHandle"
                        | "rawOps"
                        | "shellCommand"
                        | "executable"
                        | "languageServer"
                        | "process"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "language-intelligence provider metadata must not include executable or process authority field `{key}`"
                        ),
                    ));
                }
                reject_language_intelligence_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_language_intelligence_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn bounded_string_array(
    value: Option<&Value>,
    field: &str,
    max_items: usize,
    max_bytes: usize,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("language-server {field} must be an array of strings"),
        ));
    };
    if values.len() > max_items {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("language-server {field} must contain at most {max_items} entries"),
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty() && text.len() <= max_bytes && !text.chars().any(char::is_control))
                .map(str::to_string)
                .ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(contribution_id),
                        format!("language-server {field} entries must be non-empty bounded strings without control characters"),
                    )
                })
        })
        .collect()
}

fn reject_completion_provider_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::String(text)
            if text.contains("://")
                || text.contains("Deno.core.ops")
                || text.contains("nativeHandle")
                || text.contains("drawCallback")
                || text.contains("clientJavaScript")
                || text.contains("rawOps")
                || text.contains("css")
                || text.contains("rawColor") =>
        {
            Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "completion provider metadata must not contain URLs, raw ops, native handles, client JavaScript, CSS, or raw colors",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "nativeHandle"
                        | "nativeLibrary"
                        | "dynamicLibrary"
                        | "downloadUrl"
                        | "packageManager"
                        | "shellCommand"
                        | "clientJavaScript"
                        | "drawCallback"
                        | "rawOps"
                        | "css"
                        | "rawColor"
                        | "snippet"
                        | "command"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "completion provider metadata must not include executable or external authority field `{key}`"
                        ),
                    ));
                }
                reject_completion_provider_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_completion_provider_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn parse_theme_token_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<ThemeTokenContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.themeTokens must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "theme token declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "theme token contribution entries must be objects",
            )
        })?;
        if obj.contains_key("value") || obj.contains_key("rawColor") || obj.contains_key("css") {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "theme token declarations must use typed fallback contracts, not raw values, raw colors, or CSS",
            ));
        }
        let token = package_owned_field(obj, "token", api_prefix, ctx)?;
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "theme token IDs must be unique within a package",
            ));
        }
        let token_type_text = required_str_field(obj, "type", ctx)?;
        let Some(token_type) = ThemeTokenType::parse(token_type_text) else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                format!(
                    "theme token type must be one of: {}",
                    ThemeTokenType::all_as_str().join(", ")
                ),
            ));
        };
        let fallback = required_str_field(obj, "fallback", ctx)?;
        if !core_fallback_matches_type(fallback, token_type) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "theme token fallback must reference a known Clay core token with the same type",
            ));
        }
        required_str_field(obj, "description", ctx)?;
        descriptors.push(ThemeTokenContributionDescriptor {
            token: token.to_string(),
            token_type: token_type.as_str().to_string(),
            fallback: fallback.to_string(),
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

/// Parse the Plan 046 task-5 `clay.contributions.textStyles` array into inert
/// [`TextStyleOverride`]s. This is the theme-package declaration path for
/// text-rendering overrides (the contract `setTheme` in task 7 selects). The
/// SDUI typed-scalar `ThemeTokenResolver` is intentionally untouched: text
/// styles are resolved into the editor `StyleRegistry`, separate from SDUI
/// component theming. Validation mirrors `parse_theme_token_contributions`:—
/// bounded payload, deny-by-default for executable/raw-CSS/raw-color fields,
/// closed override-target vocabulary (TokenType variant names + base-UI keys),
/// valid hex color, deterministic duplicate-token diagnostics, provenance
/// recorded as the declaring package's api prefix.
fn parse_text_style_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<TextStyleOverrideDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.textStyles must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "text style declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text style contribution entries must be objects",
            )
        })?;
        // The override carries an actual color value via the `color` hex
        // string; reject the raw-injection spelling (`rawColor`/`value`/`css`)
        // so only the validated `color` field is honored.
        if obj.contains_key("value")
            || obj.contains_key("rawColor")
            || obj.contains_key("css")
            || obj.contains_key("rawCss")
            || obj.contains_key("cssText")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text style declarations must use the validated `color` hex field, not raw values, raw colors, or CSS",
            ));
        }
        let token = required_str_field(obj, "token", ctx)?;
        if parse_override_token(token).is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "text style token must name a TokenType variant (e.g. `Keyword`, `Heading1`) or a base UI color key (e.g. `panelBg`, `caret`)",
            ));
        }
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "text style tokens must be unique within a package",
            ));
        }
        let color = match obj.get("color").and_then(Value::as_str) {
            Some(hex) => Some(parse_hex_rgba(hex).ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "text style `color` must be a #rgb, #rrggbb, or #rrggbbaa hex string",
                )
            })?),
            None => None,
        };
        let bold = obj.get("bold").and_then(Value::as_bool);
        let italic = obj.get("italic").and_then(Value::as_bool);
        let underline = obj.get("underline").and_then(Value::as_bool);
        let strike = obj.get("strike").and_then(Value::as_bool);
        // At least one override field must be present, otherwise the entry is
        // a no-op declaration.
        if color.is_none()
            && bold.is_none()
            && italic.is_none()
            && underline.is_none()
            && strike.is_none()
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "text style declaration must override at least one of color, bold, italic, underline, strike",
            ));
        }
        descriptors.push(TextStyleOverrideDescriptor {
            token: token.to_string(),
            color,
            bold,
            italic,
            underline,
            strike,
            provenance: api_prefix.to_string(),
        });
    }
    Ok(descriptors)
}

// ponytail: duplicated from src/shell/components.rs (json_value_kind +
// sanitize_rejected) rather than promoted to a shared module — one tiny fn
// pair, two validation modules; sharing would add a module + import churn for
// no callers beyond these two. Fold into a shared diagnostic module if a third
// author-JSON validator appears.
/// Compact description of a rejected `serde_json::Value`'s shape for a "got …"
/// diagnostic fragment.
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

/// Trim, strip backticks, and bound to 80 chars so an author-supplied value
/// cannot break the diagnostic shape or leak unbounded content.
fn sanitize_rejected(value: &str) -> String {
    let trimmed = value.trim().replace('`', "'");
    if trimmed.chars().count() > 80 {
        let bounded: String = trimmed.chars().take(80).collect();
        format!("{bounded}…")
    } else {
        trimmed
    }
}

/// Parse `clay.contributions.designTokens` (Phase 20.1) into validated inert
/// [`DesignTokenOverrideDescriptor`]s. Each entry names a core Clay token and
/// supplies a typed value whose variant must match the core token type and pass
/// domain-specific bounds. Unknown tokens, type mismatches, NaN/infinite/
/// out-of-range scalars, unparseable levels, duplicate tokens, raw CSS/raw color
/// injection fields, and oversize payloads are rejected before the descriptor
/// is stored. Design tokens never override typography variants (that is the
/// separate hierarchy path in task 5).
fn parse_design_token_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<DesignTokenOverrideDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.designTokens must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "design token declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "design token contribution entries must be objects",
            )
        })?;
        // Reject the raw-injection spellings; the validated `value` field below is
        // the only honored value path (it is typed, not raw CSS).
        if obj.contains_key("rawColor")
            || obj.contains_key("css")
            || obj.contains_key("rawCss")
            || obj.contains_key("cssText")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "design token declarations must use the validated `value` field, not raw colors or CSS",
            ));
        }
        let token = required_str_field(obj, "token", ctx)?;
        let Some(core_type) = core_token_type(token) else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design token must name a known Clay core token",
            ));
        };
        // Design tokens carry UI scalar/color/level overrides only; typography
        // variant overrides belong to the separate typography hierarchy path.
        if core_type == ThemeTokenType::Typography {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design tokens cannot override typography variants; use the typography hierarchy",
            ));
        }
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "design token names must be unique within a package",
            ));
        }
        let value_field = obj.get("value").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design token declaration must include a `value` field",
            )
        })?;
        let descriptor_value = match core_type {
            ThemeTokenType::ColorRole => {
                let hex = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                let [r, g, b, a] = parse_hex_rgba(hex).ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got `{}`",
                            sanitize_rejected(hex)
                        ),
                    )
                })?;
                DesignTokenValueDescriptor::Color([r, g, b, a])
            }
            ThemeTokenType::Spacing | ThemeTokenType::Radius | ThemeTokenType::Dimension => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "scalar design token `value` must be a finite number; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if !is_valid_dimension(v) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "scalar design token `value` must be finite, non-negative, and bounded; got {v}"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Scalar(v.to_bits())
            }
            ThemeTokenType::Opacity => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "opacity design token `value` must be a finite number; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })? as f32;
                if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "opacity design token `value` must be a finite number in [0, 1]; got {v}"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Opacity(v.to_bits())
            }
            ThemeTokenType::MotionDuration => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "motion-duration design token `value` must be a finite number of milliseconds; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if MotionDuration::from_millis(v).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "motion-duration design token `value` must be finite, non-negative, and at most 1000 ms; got {v} ms"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Scalar(v.to_bits())
            }
            ThemeTokenType::Elevation => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "elevation design token `value` must be a level name (none, raised, overlay); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if ElevationLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "elevation design token `value` must be one of: none, raised, overlay; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            ThemeTokenType::ZLevel => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "z-level design token `value` must be a level name (base, panel, overlay, modal, tooltip); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if ZLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "z-level design token `value` must be one of: base, panel, overlay, modal, tooltip; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            ThemeTokenType::Density => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "density design token `value` must be a level name (compact, default, spacious); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if DensityLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "density design token `value` must be one of: compact, default, spacious; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            // `Typography` was rejected above; this arm is unreachable but keeps
            // the match exhaustive without a wildcard.
            ThemeTokenType::Typography => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "design tokens cannot override typography variants; use the typography hierarchy",
                ));
            }
        };
        descriptors.push(DesignTokenOverrideDescriptor {
            token: token.to_string(),
            value: descriptor_value,
            provenance: api_prefix.to_string(),
        });
    }
    Ok(descriptors)
}

type ParsedUiContributions = (
    Vec<UiPanelContributionDescriptor>,
    Vec<UiComponentContributionDescriptor>,
    Vec<UiOverlayContributionDescriptor>,
);

fn parse_ui_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<ParsedUiContributions, PackageRecordError> {
    let Value::Object(map) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.ui must be an object with panels, components, and overlays arrays",
        ));
    };

    let components = match map.get("components") {
        Some(v) => parse_ui_component_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    let panels = match map.get("panels") {
        Some(v) => parse_ui_panel_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    let overlays = match map.get("overlays") {
        Some(v) => parse_ui_overlay_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    Ok((panels, components, overlays))
}

fn parse_ui_panel_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiPanelContributionDescriptor>, PackageRecordError> {
    const VALID_SLOTS: &[&str] = &["left", "right", "top", "bottom"];
    const VALID_VISIBILITY: &[&str] = &["visible", "hidden", "collapsed"];
    let entries = array_field(value, "clay.contributions.ui.panels", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut seen_slots = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "panel contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "panel contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "panel contribution IDs must be unique within a package",
            ));
        }
        let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("fixed");
        if kind != "fixed" {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "Phase 18.3 panel contributions support kind `fixed`; transient UI must use overlays",
            ));
        }
        let slot = required_str_field(obj, "slot", ctx)?;
        if !VALID_SLOTS.contains(&slot) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel slot must be one of left, right, top, or bottom",
            ));
        }
        if !seen_slots.insert(slot.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "fixed panel contributions cannot claim the same shell slot within one package",
            ));
        }
        let default_visibility = obj
            .get("defaultVisibility")
            .and_then(Value::as_str)
            .unwrap_or("hidden");
        if !VALID_VISIBILITY.contains(&default_visibility) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel defaultVisibility must be visible, hidden, or collapsed",
            ));
        }
        let actions = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        validate_registered_action_targets(&actions, registered_command_ids, ctx)?;
        let component = obj.get("component").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel contribution must include a component object",
            )
        })?;
        let summary = validate_ui_component_tree(
            component,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        descriptors.push(UiPanelContributionDescriptor {
            id: id.to_string(),
            slot: slot.to_string(),
            component_id: summary.root_id,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

fn parse_ui_component_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiComponentContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.ui.components", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "component contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let summary = validate_ui_component_tree(
            entry,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        if !seen_ids.insert(summary.root_id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&summary.root_id),
                "component contribution root IDs must be unique within a package",
            ));
        }
        descriptors.push(UiComponentContributionDescriptor {
            id: summary.root_id,
            root_kind: summary.root_kind,
            component_count: summary.component_count,
            style_variable_count: summary.style_variable_count,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

fn parse_ui_overlay_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiOverlayContributionDescriptor>, PackageRecordError> {
    const VALID_ANCHORS: &[&str] = &["working-area", "active-pane", "main", "pointer"];
    const VALID_FOCUS: &[&str] = &["none", "restore", "trap"];
    const VALID_DISMISSAL: &[&str] = &["manual", "escape", "outside", "escape-or-outside"];
    let entries = array_field(value, "clay.contributions.ui.overlays", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "overlay contribution payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "overlay contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "overlay contribution IDs must be unique within a package",
            ));
        }
        let anchor = obj
            .get("anchor")
            .and_then(Value::as_str)
            .unwrap_or("working-area");
        if !VALID_ANCHORS.contains(&anchor) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay anchor must be one of working-area, active-pane, main, or pointer",
            ));
        }
        let focus_policy = obj
            .get("focusPolicy")
            .and_then(Value::as_str)
            .unwrap_or("restore");
        if !VALID_FOCUS.contains(&focus_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay focusPolicy must be none, restore, or trap",
            ));
        }
        let dismissal_policy = obj
            .get("dismissalPolicy")
            .and_then(Value::as_str)
            .unwrap_or("escape");
        if !VALID_DISMISSAL.contains(&dismissal_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay dismissalPolicy must be manual, escape, outside, or escape-or-outside",
            ));
        }
        let actions = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        validate_registered_action_targets(&actions, registered_command_ids, ctx)?;
        let component = obj.get("component").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay contribution must include a component object",
            )
        })?;
        let summary = validate_ui_component_tree(
            component,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        descriptors.push(UiOverlayContributionDescriptor {
            id: id.to_string(),
            anchor: anchor.to_string(),
            focus_policy: focus_policy.to_string(),
            dismissal_policy: dismissal_policy.to_string(),
            component_id: summary.root_id,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

fn parse_input_contributions(
    value: &Value,
    api_prefix: &str,
    package_modes: &[String],
    registered_command_ids: &[String],
    ctx: &ErrorContext,
) -> Result<Vec<InputContributionDescriptor>, PackageRecordError> {
    const VALID_SCOPES: &[&str] = &["component", "panel", "overlay"];
    const VALID_POINTER_CLICK: &[&str] = &["none", "focus", "action", "select"];
    const VALID_POINTER_DRAG: &[&str] = &["none", "select", "pan"];
    const VALID_FOCUS: &[&str] = &["none", "restore-editor", "focus-component", "trap"];
    const VALID_SELECTION: &[&str] = &["preserve-editor", "component-local", "disabled"];

    let entries = array_field(value, "clay.contributions.input", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "input contribution payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "input contribution", ctx)?;
        if obj.contains_key("keys") || obj.contains_key("keybindings") || obj.contains_key("onKey")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "package input contributions must not declare key routing; use behavior manifests and clay:keybindings",
            ));
        }
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "input contribution IDs must be unique within a package",
            ));
        }
        let scope = required_str_field(obj, "scope", ctx)?;
        if !VALID_SCOPES.contains(&scope) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "input scope must be component, panel, or overlay",
            ));
        }
        let component_id = package_owned_field(obj, "componentId", api_prefix, ctx)?;
        let pointer = obj.get("pointer").and_then(Value::as_object);
        let pointer_click = pointer
            .and_then(|p| p.get("click"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        if !VALID_POINTER_CLICK.contains(&pointer_click) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.click must be none, focus, action, or select",
            ));
        }
        let pointer_drag = pointer
            .and_then(|p| p.get("drag"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        if !VALID_POINTER_DRAG.contains(&pointer_drag) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.drag must be none, select, or pan",
            ));
        }
        let pointer_action = pointer
            .and_then(|p| p.get("action"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        if pointer_click == "action" && pointer_action.is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.click=action requires a registered pointer.action command ID",
            ));
        }
        let focus = obj.get("focus").and_then(Value::as_object);
        let focus_policy = focus
            .and_then(|f| f.get("policy"))
            .and_then(Value::as_str)
            .unwrap_or("restore-editor");
        if !VALID_FOCUS.contains(&focus_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "focus.policy must be none, restore-editor, focus-component, or trap",
            ));
        }
        let selection_policy = obj
            .get("selectionPolicy")
            .and_then(Value::as_str)
            .unwrap_or("preserve-editor");
        if !VALID_SELECTION.contains(&selection_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "selectionPolicy must be preserve-editor, component-local, or disabled",
            ));
        }
        if let Some(context) = obj.get("context") {
            let context = object_field(context, "input context", ctx)?;
            for mode in string_vec_field(context.get("modes"), "context.modes", ctx)? {
                if !package_modes.iter().any(|declared| declared == &mode) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(&mode),
                        "input context modes must be declared by the package manifest",
                    ));
                }
            }
        }
        let mut action_targets = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        if let Some(action) = pointer_action {
            action_targets.push(action);
        }
        validate_registered_action_targets(&action_targets, registered_command_ids, ctx)?;
        action_targets.sort();
        action_targets.dedup();

        descriptors.push(InputContributionDescriptor {
            id: id.to_string(),
            scope: scope.to_string(),
            component_id: component_id.to_string(),
            action_targets,
            estimated_payload_bytes: size,
        });
    }

    Ok(descriptors)
}

fn parse_ui_state_scope_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<UiStateScopeContributionDescriptor>, PackageRecordError> {
    const VALID_SCOPES: &[&str] = &[
        "package-global",
        "user-config",
        "workspace",
        "document",
        "pane",
        "component",
        "transient-overlay",
    ];
    const VALID_OWNERS: &[&str] = &["package", "shell", "server"];
    const VALID_LIFETIMES: &[&str] = &["session", "workspace", "document", "transient"];
    const VALID_PERSISTENCE: &[&str] = &["none", "client-local", "server-canonical", "deferred"];
    const VALID_STATUS: &[&str] = &["implemented", "deferred"];
    const VALID_SCHEMA_KINDS: &[&str] = &["boolean", "number", "string", "enum", "object"];

    let entries = array_field(value, "clay.contributions.uiStateScopes", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "UI state scope declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "UI state scope declaration", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if id
            .split('.')
            .any(|segment| segment.is_empty() || segment.starts_with('_'))
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope IDs must not use hidden or empty path segments",
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "UI state scope IDs must be unique within a package",
            ));
        }
        let scope = required_str_field(obj, "scope", ctx)?;
        if !VALID_SCOPES.contains(&scope) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope must be package-global, user-config, workspace, document, pane, component, or transient-overlay",
            ));
        }
        let owner = required_str_field(obj, "owner", ctx)?;
        if !VALID_OWNERS.contains(&owner) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state owner must be package, shell, or server",
            ));
        }
        let lifetime = required_str_field(obj, "lifetime", ctx)?;
        if !VALID_LIFETIMES.contains(&lifetime) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state lifetime must be session, workspace, document, or transient",
            ));
        }
        let persistence = required_str_field(obj, "persistence", ctx)?;
        if !VALID_PERSISTENCE.contains(&persistence) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state persistence must be none, client-local, server-canonical, or deferred",
            ));
        }
        let implementation_status = obj
            .get("implementationStatus")
            .and_then(Value::as_str)
            .unwrap_or("deferred");
        if !VALID_STATUS.contains(&implementation_status) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "implementationStatus must be implemented or deferred",
            ));
        }
        let target_id = obj
            .get("targetId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        if matches!(scope, "pane" | "component" | "transient-overlay") && target_id.is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pane, component, and transient-overlay state scopes require a package-prefixed targetId",
            ));
        }
        if let Some(target) = &target_id
            && !is_package_owned_id(target, api_prefix)
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(target),
                "state scope targetId must use the package apiPrefix",
            ));
        }
        if implementation_status == "implemented"
            && matches!(scope, "workspace" | "document" | "user-config")
            && persistence != "client-local"
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "workspace, document, and user-config UI state persistence remains deferred unless explicitly declared client-local",
            ));
        }
        let value_schema = obj.get("valueSchema").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scopes require a bounded valueSchema object",
            )
        })?;
        reject_ui_prohibited_authority(value_schema, ctx)?;
        let schema = object_field(value_schema, "valueSchema", ctx)?;
        if schema.contains_key("defaultValue")
            || schema.contains_key("initialValue")
            || schema.contains_key("rawValue")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope declarations define schemas only; state values are not accepted during registration",
            ));
        }
        let value_schema_kind = required_str_field(schema, "kind", ctx)?;
        if !VALID_SCHEMA_KINDS.contains(&value_schema_kind) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "valueSchema.kind must be boolean, number, string, enum, or object",
            ));
        }
        if value_schema_kind == "enum" {
            let values = string_vec_field(schema.get("values"), "valueSchema.values", ctx)?;
            if values.is_empty() || values.len() > 32 {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "enum valueSchema.values must include 1 to 32 string values",
                ));
            }
        }
        descriptors.push(UiStateScopeContributionDescriptor {
            id: id.to_string(),
            scope: scope.to_string(),
            owner: owner.to_string(),
            lifetime: lifetime.to_string(),
            persistence: persistence.to_string(),
            implementation_status: implementation_status.to_string(),
            value_schema_kind: value_schema_kind.to_string(),
            target_id,
            estimated_payload_bytes: size,
        });
    }

    Ok(descriptors)
}

fn parse_layout_override_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_tokens: &[ThemeTokenContributionDescriptor],
    input_contributions: &[InputContributionDescriptor],
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LayoutOverrideContributionDescriptor>, PackageRecordError> {
    const VALID_PROPERTIES: &[&str] = &[
        "slot",
        "visibility",
        "splitRatio",
        "themeToken",
        "inputDefault",
        "actionDefault",
        "fallback",
    ];
    const VALID_SOURCES: &[&str] = &["global-package", "package-default"];

    let entries = array_field(value, "clay.contributions.layoutOverrides", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "layout override contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }
    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "layout override payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "layout override declaration", ctx)?;
        let target_id = package_owned_field(obj, "targetId", api_prefix, ctx)?;
        let property = required_str_field(obj, "property", ctx)?;
        if !VALID_PROPERTIES.contains(&property) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(property),
                "layout override property must be slot, visibility, splitRatio, themeToken, inputDefault, actionDefault, or fallback",
            ));
        }
        let source = obj
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("package-default");
        if !VALID_SOURCES.contains(&source) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(source),
                "manifest layout override source must be global-package or package-default; user and mode overrides flow through documented configuration APIs",
            ));
        }
        let value = obj.get("value").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(target_id),
                "layout override requires a typed value",
            )
        })?;
        validate_layout_override_contribution_value(
            property,
            target_id,
            value,
            registered_command_ids,
            theme_tokens,
            input_contributions,
            ctx,
        )?;
        let key = format!("{target_id}:{property}");
        if !seen.insert(key) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(target_id),
                "layout override targets and properties must be unique within a package",
            ));
        }
        descriptors.push(LayoutOverrideContributionDescriptor {
            target_id: target_id.to_string(),
            property: property.to_string(),
            source: source.to_string(),
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

fn parse_package_option_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<PackageOptionContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.packageOptions", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "package option contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }
    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "package option schema payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "package option declaration", ctx)?;
        let option = package_owned_field(obj, "option", api_prefix, ctx)?;
        validate_package_option_suffix(api_prefix, option, ctx)?;
        if !seen.insert(option.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(option),
                "package option names must be unique within a package",
            ));
        }
        let value_type = required_str_field(obj, "type", ctx)?;
        if !matches!(
            value_type,
            "boolean" | "string" | "number" | "integer" | "object"
        ) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(option),
                "package option type must be boolean, string, number, integer, or object",
            ));
        }
        validate_package_option_type(api_prefix, option, value_type, ctx)?;
        let default_value = obj
            .get("default")
            .map(|value| serde_json::to_string(value).unwrap_or_default());
        if let Some(default) = obj.get("default") {
            validate_package_option_default(api_prefix, option, default, ctx)?;
        }
        descriptors.push(PackageOptionContributionDescriptor {
            option: option.to_string(),
            value_type: value_type.to_string(),
            default_value,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

struct UiComponentSummary {
    root_id: String,
    root_kind: String,
    component_count: usize,
    style_variable_count: usize,
}

fn validate_ui_component_tree(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<UiComponentSummary, PackageRecordError> {
    let mut state = UiComponentValidationState {
        api_prefix,
        registered_command_ids,
        theme_resolver,
        ctx,
        seen_ids: HashSet::new(),
        component_count: 0,
        style_variable_count: 0,
    };
    let (root_id, root_kind) = state.validate_node(value)?;
    Ok(UiComponentSummary {
        root_id,
        root_kind,
        component_count: state.component_count,
        style_variable_count: state.style_variable_count,
    })
}

struct UiComponentValidationState<'a> {
    api_prefix: &'a str,
    registered_command_ids: &'a [String],
    theme_resolver: &'a ThemeTokenResolver,
    ctx: &'a ErrorContext,
    seen_ids: HashSet<String>,
    component_count: usize,
    style_variable_count: usize,
}

impl UiComponentValidationState<'_> {
    fn validate_node(&mut self, value: &Value) -> Result<(String, String), PackageRecordError> {
        const MAX_COMPONENT_NODES: usize = 128;
        self.component_count += 1;
        if self.component_count > MAX_COMPONENT_NODES {
            return Err(self.ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!("component tree exceeds {MAX_COMPONENT_NODES} nodes"),
            ));
        }
        reject_ui_prohibited_authority(value, self.ctx)?;
        let obj = object_field(value, "component", self.ctx)?;
        let kind = required_str_field(obj, "kind", self.ctx)?;
        let component_kind = validate_component_kind(kind).map_err(|error| {
            self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&error.field),
                error.message,
            )
        })?;
        let id = package_owned_field(obj, "id", self.api_prefix, self.ctx)?;
        if !self.seen_ids.insert(id.to_string()) {
            return Err(self.ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "component IDs must be unique within a package UI contribution tree",
            ));
        }
        if obj.contains_key("styleString") || obj.contains_key("className") {
            return Err(self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "component declarations must use typed style variables, not raw CSS/style strings or class names",
            ));
        }
        let style_variables =
            validate_style_variables(obj, self.theme_resolver).map_err(|error| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&error.field),
                    error.message,
                )
            })?;
        if style_variables
            .iter()
            .any(|variable| variable.name == "fontRole")
            && !component_kind.supports_text_font_role()
        {
            return Err(self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "style.fontRole is only supported by text-bearing panel, label, button, list, and statusItem components",
            ));
        }
        self.style_variable_count += style_variables.len();
        if let Some(action) = obj.get("action").and_then(Value::as_object) {
            let command_id = required_str_field(action, "commandId", self.ctx)?;
            validate_registered_action_targets(
                &[command_id.to_string()],
                self.registered_command_ids,
                self.ctx,
            )?;
        }
        if let Some(items) = obj.get("items") {
            let items = items.as_array().ok_or_else(|| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "component items must be an array",
                )
            })?;
            for item in items {
                let item_object = item.as_object().ok_or_else(|| {
                    self.ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(id),
                        "component list items must be objects",
                    )
                })?;
                if let Some(action) = item_object.get("action").and_then(Value::as_object) {
                    let command_id = required_str_field(action, "commandId", self.ctx)?;
                    validate_registered_action_targets(
                        &[command_id.to_string()],
                        self.registered_command_ids,
                        self.ctx,
                    )?;
                }
            }
        }
        if let Some(children) = obj.get("children") {
            let children = children.as_array().ok_or_else(|| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "component children must be an array",
                )
            })?;
            for child in children {
                self.validate_node(child)?;
            }
        }
        Ok((id.to_string(), kind.to_string()))
    }
}

fn validate_layout_override_contribution_value(
    property: &str,
    target_id: &str,
    value: &Value,
    registered_command_ids: &[String],
    theme_tokens: &[ThemeTokenContributionDescriptor],
    input_contributions: &[InputContributionDescriptor],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match property {
        "slot" => {
            let Some(slot) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "slot override value must be a string",
                ));
            };
            if !matches!(slot, "left" | "right" | "top" | "bottom") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(slot),
                    "slot override value must be left, right, top, or bottom",
                ));
            }
        }
        "visibility" => {
            let Some(visibility) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "visibility override value must be a string",
                ));
            };
            if !matches!(visibility, "visible" | "hidden" | "collapsed") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(visibility),
                    "visibility override value must be visible, hidden, or collapsed",
                ));
            }
        }
        "splitRatio" => {
            let Some(ratio) = value.as_f64() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "splitRatio override value must be a number",
                ));
            };
            if !(0.1..=0.9).contains(&ratio) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "splitRatio override value must be between 0.1 and 0.9",
                ));
            }
        }
        "themeToken" => {
            let obj = object_field(value, "themeToken override value", ctx)?;
            let token = required_str_field(obj, "token", ctx)?;
            if !is_package_owned_id(token, target_package_prefix(target_id)) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "themeToken override token must use the target package prefix",
                ));
            }
            let fallback = required_str_field(obj, "fallback", ctx)?;
            let declared = theme_tokens
                .iter()
                .find(|declared| declared.token == token)
                .ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        "themeToken override token must be declared in clay.contributions.themeTokens",
                    )
                })?;
            let Some(token_type) = ThemeTokenType::parse(&declared.token_type) else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "theme token declaration has an invalid type",
                ));
            };
            if !core_fallback_matches_type(fallback, token_type) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(fallback),
                    "themeToken fallback must reference a known Clay core token with the same type",
                ));
            }
        }
        "inputDefault" => {
            let obj = object_field(value, "inputDefault override value", ctx)?;
            let input_id = required_str_field(obj, "inputId", ctx)?;
            if !input_contributions.iter().any(|input| input.id == input_id) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(input_id),
                    "inputDefault.inputId must reference a declared package input contribution",
                ));
            }
        }
        "actionDefault" => {
            let Some(action_id) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "actionDefault override value must be a registered action ID string",
                ));
            };
            if !registered_command_ids
                .iter()
                .any(|command| command == action_id)
            {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(action_id),
                    "actionDefault must reference a command declared in clay.contributions.commands",
                ));
            }
        }
        "fallback" => {
            let Some(fallback) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "fallback override value must be a string",
                ));
            };
            if !matches!(fallback, "package-default" | "hide" | "ignore") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(fallback),
                    "fallback override value must be package-default, hide, or ignore",
                ));
            }
        }
        _ => unreachable!("layout override property validated before value validation"),
    }
    Ok(())
}

fn validate_package_option_suffix(
    api_prefix: &str,
    option: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    if option
        .split('.')
        .any(|segment| segment.is_empty() || segment.starts_with('_'))
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            "package option names must not use hidden or empty path segments",
        ));
    }
    if !matches!(
        suffix,
        "layout.defaultVisibility"
            | "layout.defaultSlot"
            | "layout.splitRatio"
            | "input.default"
            | "action.default"
            | "themeTokenRemap"
            | "fallback"
    ) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            "unsupported package option; use documented layout.defaultVisibility, layout.defaultSlot, layout.splitRatio, input.default, action.default, themeTokenRemap, or fallback options",
        ));
    }
    Ok(())
}

fn validate_package_option_type(
    api_prefix: &str,
    option: &str,
    value_type: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    let expected = match suffix {
        "layout.defaultVisibility"
        | "layout.defaultSlot"
        | "input.default"
        | "action.default"
        | "fallback" => "string",
        "layout.splitRatio" => "number",
        "themeTokenRemap" => "object",
        _ => value_type,
    };
    if value_type != expected {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            format!("package option `{option}` must declare type `{expected}`"),
        ));
    }
    Ok(())
}

fn validate_package_option_default(
    api_prefix: &str,
    option: &str,
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    match suffix {
        "layout.defaultVisibility" => {
            validate_string_choice(value, &["visible", "hidden", "collapsed"], option, ctx)
        }
        "layout.defaultSlot" => {
            validate_string_choice(value, &["left", "right", "top", "bottom"], option, ctx)
        }
        "layout.splitRatio" => {
            let Some(ratio) = value.as_f64() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "layout.splitRatio default must be a number",
                ));
            };
            if !(0.1..=0.9).contains(&ratio) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "layout.splitRatio default must be between 0.1 and 0.9",
                ));
            }
            Ok(())
        }
        "input.default" | "action.default" => {
            let Some(id) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "input.default and action.default defaults must be package-prefixed strings",
                ));
            };
            if !is_package_owned_id(id, api_prefix) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "input.default and action.default defaults must use package-prefixed public IDs",
                ));
            }
            Ok(())
        }
        "themeTokenRemap" => {
            let object = object_field(value, "themeTokenRemap default", ctx)?;
            let token = required_str_field(object, "token", ctx)?;
            if !is_package_owned_id(token, api_prefix) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "themeTokenRemap token must use the package apiPrefix",
                ));
            }
            required_str_field(object, "fallback", ctx)?;
            Ok(())
        }
        "fallback" => {
            validate_string_choice(value, &["package-default", "hide", "ignore"], option, ctx)
        }
        _ => Ok(()),
    }
}

fn validate_string_choice(
    value: &Value,
    allowed: &[&str],
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let Some(text) = value.as_str() else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "package option default must be a string for this option",
        ));
    };
    if !allowed.contains(&text) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!(
                "package option default must be one of: {}",
                allowed.join(", ")
            ),
        ));
    }
    Ok(())
}

fn target_package_prefix(target_id: &str) -> &str {
    target_id.split('.').next().unwrap_or(target_id)
}

fn reject_syntax_grammar_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::String(text)
            if text.contains("://")
                || text.contains("Deno.core.ops")
                || text.contains("nativeHandle")
                || text.contains("drawCallback")
                || text.contains("clientJavaScript")
                || text.contains("rawOps")
                || text.contains("css")
                || text.contains("rawColor") =>
        {
            Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "syntax grammar metadata must not contain URLs, raw ops, native handles, client JavaScript, CSS, raw colors, or concrete typography",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "nativeHandle"
                        | "nativeLibrary"
                        | "dynamicLibrary"
                        | "downloadUrl"
                        | "packageManager"
                        | "shellCommand"
                        | "clientJavaScript"
                        | "drawCallback"
                        | "rawOps"
                        | "css"
                        | "rawColor"
                        | "fontFamily"
                        | "fontFamilies"
                        | "fontSize"
                        | "fontStack"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "syntax grammar metadata must not include executable, external, or concrete typography field `{key}`"
                        ),
                    ));
                }
                reject_syntax_grammar_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_syntax_grammar_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_valid_language_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_' | '+' | '.')
        })
}

fn validate_package_asset_path(
    path: &str,
    field: &str,
    required_suffix: Option<&str>,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let valid = path.starts_with("./")
        && !path.contains('\\')
        && !path.contains("://")
        && !path.contains("Deno.core.ops")
        && path[2..]
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !valid {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(path),
            format!("{field} must be a package-root-confined relative ./ path without traversal, URLs, or raw ops"),
        ));
    }
    if let Some(suffix) = required_suffix
        && !path.ends_with(suffix)
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(path),
            format!("{field} must end with {suffix}"),
        ));
    }
    Ok(())
}

fn optional_asset_path(
    value: Option<&Value>,
    field: &str,
    ctx: &ErrorContext,
) -> Result<Option<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) if !path.trim().is_empty() => {
            validate_package_asset_path(path, field, Some(".scm"), ctx)?;
            Ok(Some(path.clone()))
        }
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{field} must be a non-empty string path when present"),
        )),
    }
}

fn parse_syntax_style_map(
    value: Option<&Value>,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<BTreeMap<String, SyntaxStyleMapEntry>, PackageRecordError> {
    let Some(Value::Object(map)) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "syntax grammar styleMap must map captures to vocabulary objects or known legacy Clay style tokens",
        ));
    };
    if map.is_empty() {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "syntax grammar styleMap must not be empty",
        ));
    }

    let mut style_map = BTreeMap::new();
    for (capture, value) in map {
        if capture.trim().is_empty()
            || capture.starts_with('@')
            || capture.contains('{')
            || capture.contains('}')
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(capture),
                "syntax grammar capture names must be non-empty names without @, braces, CSS, or query payloads",
            ));
        }

        let priority = match value {
            Value::Object(entry) => match entry.get("priority") {
                None => DEFAULT_SYNTAX_STYLE_PRIORITY,
                Some(Value::Number(number)) => match number
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                    .filter(|value| *value <= MAX_SYNTAX_STYLE_PRIORITY)
                {
                    Some(priority) => priority,
                    None => {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap `priority` must be an integer between 0 and 100",
                        ));
                    }
                },
                Some(_) => {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(capture),
                        "syntax grammar styleMap `priority` must be an integer between 0 and 100",
                    ));
                }
            },
            _ => DEFAULT_SYNTAX_STYLE_PRIORITY,
        };

        let (token_type, modifiers, scope, font_role) = match value {
            Value::String(style_token) => {
                if !is_known_syntax_style_token(style_token) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(style_token),
                        "syntax grammar styleMap legacy values must be known Clay style tokens, not raw CSS or colors",
                    ));
                }
                let (token_type, modifiers) = TokenType::classify_style_token(style_token);
                (token_type, modifiers, Some(style_token.clone()), None)
            }
            Value::Object(entry) => {
                if entry.contains_key("fontFamily")
                    || entry.contains_key("fontFamilies")
                    || entry.contains_key("fontSize")
                    || entry.contains_key("fontStack")
                    || entry.contains_key("color")
                {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(capture),
                        "syntax grammar styleMap entries may select vocabulary and a semantic fontRole, never font families, sizes, stacks, or colors",
                    ));
                }
                let font_role = match entry.get("fontRole") {
                    None => None,
                    Some(Value::String(role)) => match DocumentFontRole::from_name(role) {
                        Some(
                            role @ (DocumentFontRole::Monospace | DocumentFontRole::Proportional),
                        ) => Some(role),
                        _ => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(role),
                                "syntax grammar styleMap fontRole must be `monospace` or `proportional`",
                            ));
                        }
                    },
                    Some(_) => {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap fontRole must be a semantic role string",
                        ));
                    }
                };

                if let Some(style_token) = entry.get("styleToken").and_then(Value::as_str) {
                    if entry.contains_key("type") || entry.contains_key("modifiers") {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap entries must use either legacy `styleToken` or vocabulary `type` + `modifiers`, never both",
                        ));
                    }
                    if !is_known_syntax_style_token(style_token) {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(style_token),
                            "syntax grammar styleMap legacy values must be known Clay style tokens, not raw CSS or colors",
                        ));
                    }
                    let (token_type, modifiers) = TokenType::classify_style_token(style_token);
                    (
                        token_type,
                        modifiers,
                        Some(style_token.to_string()),
                        font_role,
                    )
                } else {
                    let type_name = entry
                        .get("type")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(capture),
                                "syntax grammar styleMap entry requires a known `type` naming a TokenType variant (e.g. `Keyword`, `String`, `Heading1`)",
                            )
                        })?;
                    let token_type = TokenType::from_name(type_name).ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(type_name),
                            "syntax grammar styleMap `type` must name a known TokenType variant, not raw CSS, a color, or a free-form string",
                        )
                    })?;
                    let modifiers = match entry.get("modifiers") {
                        None => Modifiers::NONE,
                        Some(Value::Array(names)) => {
                            let parsed: Vec<&str> = names
                                .iter()
                                .map(|value| value.as_str().unwrap_or(""))
                                .collect();
                            Modifiers::from_names(&parsed).ok_or_else(|| {
                                ctx.error(
                                    PackageRecordRule::InvalidContributionDescriptor,
                                    Some(capture),
                                    "syntax grammar styleMap `modifiers` must be an array of known Modifiers variant names (e.g. `Declaration`, `Bold`)",
                                )
                            })?
                        }
                        Some(_) => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(capture),
                                "syntax grammar styleMap `modifiers` must be an array of known Modifiers variant names",
                            ));
                        }
                    };
                    (token_type, modifiers, None, font_role)
                }
            }
            _ => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(capture),
                    "syntax grammar styleMap values must be vocabulary objects or known legacy Clay style-token strings",
                ));
            }
        };

        style_map.insert(
            capture.clone(),
            SyntaxStyleMapEntry {
                token_type,
                modifiers,
                scope,
                font_role,
                priority,
            },
        );
    }
    Ok(style_map)
}

fn is_known_syntax_style_token(token: &str) -> bool {
    matches!(
        token,
        "markup.heading.1"
            | "markup.heading.2"
            | "markup.heading.3"
            | "markup.heading.4"
            | "markup.heading.5"
            | "markup.heading.6"
            | "markup.strong"
            | "markup.emphasis"
            | "markup.inline-code"
            | "markup.code-block"
            | "markup.list-marker"
            | "keyword.control"
            | "string.quoted"
            | "comment.line"
            | "punctuation.definition"
            | "diagnostic.error"
            | "diagnostic.warning"
            | "diagnostic.info"
            | "search.match"
            | "text"
    )
}

fn optional_u64_budget(
    budgets: Option<&serde_json::Map<String, Value>>,
    field: &str,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Option<u64>, PackageRecordError> {
    match budgets.and_then(|budgets| budgets.get(field)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(contribution_id),
                format!("budgets.{field} must be a non-negative integer"),
            )
        }),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("budgets.{field} must be a non-negative integer"),
        )),
    }
}

fn optional_usize_budget(
    budgets: Option<&serde_json::Map<String, Value>>,
    field: &str,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Option<usize>, PackageRecordError> {
    optional_u64_budget(budgets, field, contribution_id, ctx).map(|value| value.map(|n| n as usize))
}

fn theme_resolver_for_package_tokens(
    tokens: &[ThemeTokenContributionDescriptor],
) -> ThemeTokenResolver {
    let mut resolver = ThemeTokenResolver::new();
    for token in tokens {
        let Some(token_type) = ThemeTokenType::parse(&token.token_type) else {
            continue;
        };
        resolver.insert_package_token(PackageThemeToken {
            token: token.token.clone(),
            token_type,
            fallback: token.fallback.clone(),
            description: String::new(),
        });
    }
    resolver
}

// ── Utility ──────────────────────────────────────────────────────────────────

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn payload_size(value: &Value) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

fn contribution_payload_size(value: &Value) -> usize {
    value
        .as_object()
        .and_then(|object| object.get("estimatedPayloadBytes"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or_else(|| payload_size(value))
}

fn array_field<'a>(
    value: &'a Value,
    label: &str,
    ctx: &ErrorContext,
) -> Result<&'a Vec<Value>, PackageRecordError> {
    value.as_array().ok_or_else(|| {
        ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{label} must be an array"),
        )
    })
}

fn object_field<'a>(
    value: &'a Value,
    label: &str,
    ctx: &ErrorContext,
) -> Result<&'a serde_json::Map<String, Value>, PackageRecordError> {
    value.as_object().ok_or_else(|| {
        ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{label} must be an object"),
        )
    })
}

fn required_str_field<'a>(
    obj: &'a serde_json::Map<String, Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<&'a str, PackageRecordError> {
    obj.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                format!("{key} must be a non-empty string"),
            )
        })
}

fn package_owned_field<'a>(
    obj: &'a serde_json::Map<String, Value>,
    key: &str,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<&'a str, PackageRecordError> {
    let value = required_str_field(obj, key, ctx)?;
    if value.starts_with("clay.") {
        return Err(ctx.error(
            PackageRecordRule::ReservedClayIdInContribution,
            Some(value),
            format!("{key} cannot claim the reserved clay.* namespace"),
        ));
    }
    if !is_package_owned_id(value, api_prefix) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(value),
            format!("{key} must use the package apiPrefix or apiPrefix.* namespace"),
        ));
    }
    Ok(value)
}

fn parse_completion_items(
    value: Option<&Value>,
    provider_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<CompletionItemContributionDescriptor>, PackageRecordError> {
    let values = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(values)) => values,
        Some(_) => {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(provider_id),
                "completion provider items must be an array",
            ));
        }
    };

    values
        .iter()
        .map(|value| {
            let item = match value {
                Value::String(text) if !text.trim().is_empty() => {
                    CompletionItemContributionDescriptor {
                        label: text.clone(),
                        insert_text: text.clone(),
                        detail: String::new(),
                        text_format: CompletionItemTextFormat::PlainText,
                    }
                }
                Value::Object(object) => {
                    if object.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "label" | "insertText" | "detail" | "textFormat"
                        )
                    }) {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(provider_id),
                            "completion provider item objects accept only label, insertText, detail, and textFormat",
                        ));
                    }
                    let label = required_str_field(object, "label", ctx)?.to_string();
                    let insert_text =
                        required_str_field(object, "insertText", ctx)?.to_string();
                    let detail = match object.get("detail") {
                        None | Some(Value::Null) => String::new(),
                        Some(Value::String(detail)) => detail.clone(),
                        Some(_) => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(provider_id),
                                "completion provider item detail must be a string",
                            ));
                        }
                    };
                    let text_format = match object.get("textFormat") {
                        None | Some(Value::Null) => CompletionItemTextFormat::PlainText,
                        Some(Value::String(value)) if value == "plainText" => {
                            CompletionItemTextFormat::PlainText
                        }
                        Some(Value::String(value)) if value == "snippet" => {
                            CompletionItemTextFormat::Snippet
                        }
                        _ => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(provider_id),
                                "completion provider item textFormat must be `plainText` or `snippet`",
                            ));
                        }
                    };
                    CompletionItemContributionDescriptor {
                        label,
                        insert_text,
                        detail,
                        text_format,
                    }
                }
                _ => {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(provider_id),
                        "completion provider items must be non-empty strings or structured item objects",
                    ));
                }
            };

            for (field, chars, max) in [
                (
                    "label",
                    item.label.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
                ),
                (
                    "insertText",
                    item.insert_text.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS,
                ),
                (
                    "detail",
                    item.detail.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
                ),
            ] {
                if chars > max {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(provider_id),
                        format!("completion provider item {field} exceeds {max} characters"),
                    ));
                }
            }
            Ok(item)
        })
        .collect()
}

fn optional_string_vec(
    value: Option<&Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            None,
                            format!("{key} entries must be non-empty strings"),
                        )
                    })
            })
            .collect(),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{key} must be an array"),
        )),
    }
}

fn string_vec_field(
    value: Option<&Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            None,
                            format!("{key} entries must be non-empty strings"),
                        )
                    })
            })
            .collect(),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{key} must be an array"),
        )),
    }
}

fn validate_registered_action_targets(
    action_targets: &[String],
    registered_command_ids: &[String],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    for command_id in action_targets {
        if !registered_command_ids
            .iter()
            .any(|registered| registered == command_id)
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(command_id),
                format!(
                    "UI action target `{command_id}` must be declared in clay.contributions.commands"
                ),
            ));
        }
    }
    Ok(())
}

fn reject_ui_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::String(text) if text.contains("Deno.core.ops") || text.contains("op_clay_") => {
            Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "package UI metadata must not expose raw Deno.core.ops or op names",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "rawOps"
                        | "nativeHandle"
                        | "nativeWidget"
                        | "masonryWidget"
                        | "widgetCallback"
                        | "rendererCallback"
                        | "drawCallback"
                        | "clientHook"
                        | "clientJavaScript"
                        | "javascript"
                        | "code"
                        | "rawCss"
                        | "cssText"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(key),
                        "package UI metadata must not include raw ops, native widgets, raw CSS, renderer callbacks, or client-side JavaScript hooks",
                    ));
                }
                reject_ui_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_ui_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

struct ErrorContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
}

impl ErrorContext {
    fn error(
        &self,
        rule: PackageRecordRule,
        contribution_id: Option<&str>,
        message: impl Into<Box<str>>,
    ) -> PackageRecordError {
        PackageRecordError {
            package_name: self.package_name.clone().map(String::into_boxed_str),
            package_version: self.package_version.clone().map(String::into_boxed_str),
            api_prefix: self.api_prefix.clone().map(String::into_boxed_str),
            contribution_id: contribution_id.map(|id| id.to_string().into_boxed_str()),
            rule,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_markdown_fixture() -> Value {
        json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "permissions": [
                    "mode-registration",
                    "mode-activation",
                    "command-registration",
                    "package-configuration"
                ],
                "modes": ["markdown"],
                "docs": "./docs/index.md",
                "apiDependencies": [
                    "clay.modes.serverRegisterModePattern",
                    "clay.commands.serverRegisterCommand"
                ],
                "contributions": {
                    "commands": [{
                        "id": "markdown.togglePreview",
                        "displayName": "Toggle Markdown Preview",
                        "routingPolicy": "server-first"
                    }],
                    "configuration": [{
                        "key": "markdown.preview.enabled",
                        "type": "boolean",
                        "default": false
                    }]
                }
            }
        })
    }

    #[test]
    fn package_record_accepts_full_markdown_contract() {
        let record = assemble_package_record(&full_markdown_fixture())
            .expect("full markdown contract must validate");

        assert_eq!(record.manifest.name, "@clay/markdown");
        assert_eq!(record.manifest.version, "0.1.0");
        assert_eq!(record.manifest.clay.api_prefix, "markdown");
        assert_eq!(record.docs.docs_path, "./docs/index.md");
        assert_eq!(record.api_dependencies.len(), 2);
        assert_eq!(
            record.api_dependencies[0].api_id,
            "clay.modes.serverRegisterModePattern"
        );
        assert_eq!(record.contributions.commands.len(), 1);
        assert_eq!(
            record.contributions.commands[0].id,
            "markdown.togglePreview"
        );
        assert_eq!(record.contributions.configuration.len(), 1);
        assert_eq!(
            record.contributions.configuration[0].key,
            "markdown.preview.enabled"
        );
    }

    #[test]
    fn package_record_rejects_missing_docs_field() {
        let mut fixture = full_markdown_fixture();
        // Remove the docs field.
        fixture["clay"].as_object_mut().unwrap().remove("docs");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::MissingRequiredField);
        assert!(err.message.contains("clay.docs"));
        assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
    }

    #[test]
    fn package_record_rejects_contribution_claiming_clay_reserved_id() {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["commands"][0]["id"] = json!("clay.badCommand");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ReservedClayIdInContribution);
        assert!(err.message.contains("clay.*"));
    }

    #[test]
    fn package_record_rejects_undeclared_permission_for_contribution() {
        let mut fixture = full_markdown_fixture();
        // Strip `command-registration` so commands are undeclared.
        fixture["clay"]["permissions"] = json!([
            "mode-registration",
            "mode-activation",
            "package-configuration"
        ]);
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(
            err.rule,
            PackageRecordRule::UndeclaredPermissionForContribution
        );
        assert!(err.message.contains("command-registration"));
    }

    /// Compile-time size guard for the boxed diagnostic types. After Plan 030
    /// task "Box large diagnostic error types", each `Err`-variant diagnostic is
    /// under clippy's `result_large_err` 128-byte threshold. `const _: () =`
    /// makes the assertion a compile-time check (not a runtime test); if a
    /// future field reverts the shrink, compilation fails here instead of
    /// silently reintroducing a large `Result` copy. `PackageServiceError` is
    /// not asserted here directly because its two payload variants are already
    /// `Box<...>`, so the enum's inline size is dominated by its small variants.
    #[test]
    fn diagnostic_error_sizes_remain_under_large_err_threshold() {
        const fn assert_le_128<T>() {
            assert!(std::mem::size_of::<T>() <= 128);
        }
        const _: () = {
            assert_le_128::<PackageRecordError>();
            assert_le_128::<crate::packages::modes::ModeDiagnostic>();
            assert_le_128::<crate::packages::commands::CommandDiagnostic>();
            assert_le_128::<crate::packages::conflict::PackageConflictDiagnostic>();
        };
        // `UiContributionDiagnostic` is `pub(crate)` in `server::ui`; assert it
        // from that module to avoid a privacy error, and assert the boxed
        // `PackageServiceError` payload variants stay small via the service
        // error size being dominated by its `BackendError` (small) variant.
        assert!(std::mem::size_of::<PackageRecordError>() <= 128);
        assert!(std::mem::size_of::<crate::packages::modes::ModeDiagnostic>() <= 128);
        assert!(std::mem::size_of::<crate::packages::commands::CommandDiagnostic>() <= 128);
        assert!(std::mem::size_of::<crate::packages::conflict::PackageConflictDiagnostic>() <= 128);
    }

    /// Minimal theme-package manifest carrying only `contributions.textStyles`.
    fn theme_fixture_with_text_styles(overrides: Value) -> Value {
        json!({
            "name": "@clay/theme-test",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "clay-theme-test",
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "docs": "./docs/index.md",
                "permissions": [],
                "modes": [],
                "contributions": {
                    "textStyles": overrides
                }
            }
        })
    }

    #[test]
    fn text_style_contributions_parse_into_inert_descriptor() {
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "Keyword", "color": "#c792ea", "bold": true },
            { "token": "panelBg", "color": "#101010ff" },
            { "token": "Heading1", "italic": true }
        ]));
        let record = assemble_package_record(&fixture).expect("theme textStyles must validate");
        assert_eq!(record.contributions.text_styles.len(), 3);
        let kw = &record.contributions.text_styles[0];
        assert_eq!(kw.token, "Keyword");
        assert_eq!(kw.color, Some([0xc7, 0x92, 0xea, 0xff]));
        assert_eq!(kw.bold, Some(true));
        assert_eq!(kw.provenance, "clay-theme-test");
        // `to_override` round-trips the RGBA bytes into a peniko Color.
        let kw_override = kw.to_override();
        assert_eq!(
            kw_override.color,
            Some(masonry::peniko::Color::from_rgba8(0xc7, 0x92, 0xea, 0xff))
        );
        assert_eq!(kw_override.bold, Some(true));
    }

    #[test]
    fn text_style_unknown_token_rejected() {
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "keyword.control", "color": "#fff" }
        ]));
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("TokenType variant"));
    }

    #[test]
    fn text_style_duplicate_token_rejected() {
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "Keyword", "color": "#fff" },
            { "token": "Keyword", "color": "#000" }
        ]));
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::DuplicateContributionId);
    }

    #[test]
    fn text_style_bad_hex_color_rejected() {
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "Keyword", "color": "not-a-color" }
        ]));
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("hex string"));
    }

    #[test]
    fn text_style_raw_color_and_css_fields_rejected() {
        for bad_field in ["rawColor", "value", "css", "rawCss", "cssText"] {
            let fixture = theme_fixture_with_text_styles(json!([
                { "token": "Keyword", "color": "#fff", bad_field: "#000" }
            ]));
            let err = assemble_package_record(&fixture).unwrap_err();
            assert_eq!(
                err.rule,
                PackageRecordRule::InvalidContributionDescriptor,
                "raw color/css field `{bad_field}` must be rejected"
            );
            assert!(
                err.message.contains("validated `color` hex field") || err.message.contains("raw"),
                "raw color/css field `{bad_field}` must be denied as executable/raw-CSS"
            );
        }
    }

    #[test]
    fn text_style_executable_field_rejected() {
        // `code` is in the deny-by-default authority list; an inert theme must
        // never carry executable fields.
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "Keyword", "color": "#fff", "code": "return 0" }
        ]));
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("raw ops, native widgets, raw CSS"));
    }

    #[test]
    fn text_style_no_op_entry_rejected() {
        // An entry that overrides nothing is a useless declaration.
        let fixture = theme_fixture_with_text_styles(json!([
            { "token": "Keyword" }
        ]));
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("at least one of color"));
    }

    fn theme_fixture_with_design_tokens(tokens: Value) -> Value {
        json!({
            "name": "@clay/theme-test",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "clay-theme-test",
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "docs": "./docs/index.md",
                "permissions": [],
                "modes": [],
                "contributions": {
                    "designTokens": tokens
                }
            }
        })
    }

    #[test]
    fn design_token_contributions_parse_into_inert_descriptors_and_round_trip_to_wire() {
        let fixture = theme_fixture_with_design_tokens(json!([
            { "token": "surface.hover", "value": "#112233ff" },
            { "token": "spacing.md", "value": 18 },
            { "token": "opacity.full", "value": 0.9 },
            { "token": "dimension.sidebar.default", "value": 200.0 },
            { "token": "elevation.raised", "value": "raised" },
            { "token": "motion.fast", "value": 80 },
            { "token": "z.tooltip", "value": "tooltip" },
            { "token": "density.spacious", "value": "spacious" }
        ]));
        let record = assemble_package_record(&fixture).expect("design tokens must validate");
        assert_eq!(record.contributions.design_tokens.len(), 8);
        let hover = record
            .contributions
            .design_tokens
            .iter()
            .find(|d| d.token == "surface.hover")
            .unwrap();
        assert_eq!(
            hover.value,
            DesignTokenValueDescriptor::Color([0x11, 0x22, 0x33, 0xff])
        );
        assert_eq!(hover.provenance, "clay-theme-test");
        // to_wire reconstructs native f64/f32 from bits and preserves levels.
        let wire = hover.to_wire();
        assert_eq!(
            wire.value,
            crate::protocol::WireDesignTokenValue::Color([0x11, 0x22, 0x33, 0xff])
        );
        let spacing = record
            .contributions
            .design_tokens
            .iter()
            .find(|d| d.token == "spacing.md")
            .unwrap();
        assert_eq!(
            spacing.value,
            DesignTokenValueDescriptor::Scalar((18.0_f64).to_bits())
        );
        assert_eq!(
            spacing.to_wire().value,
            crate::protocol::WireDesignTokenValue::Scalar(18.0)
        );
    }

    #[test]
    fn design_token_rejects_unknown_type_mismatch_invalid_and_raw_fields() {
        // Unknown token.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "nope.token", "value": "#fff" }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("known Clay core token"));
        // Type mismatch: scalar for a color-role token.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "surface.hover", "value": 12 }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("hex string"));
        // Out-of-range scalar.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "dimension.sidebar.default", "value": -5.0 }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("finite, non-negative"));
        // Out-of-range opacity.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "opacity.full", "value": 2.0 }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("[0, 1]"));
        // Out-of-range motion duration.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "motion.fast", "value": 5000 }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("1000 ms"));
        // Invalid elevation level.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "elevation.raised", "value": "huge" }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("none, raised, overlay"));
        // Typography override via design tokens is not allowed.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "typography.body", "value": "#fff" }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("typography hierarchy"));
        // Duplicate token.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "surface.hover", "value": "#fff" },
            { "token": "surface.hover", "value": "#000" }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::DuplicateContributionId);
        // Raw CSS/color fields rejected.
        let err = assemble_package_record(&theme_fixture_with_design_tokens(json!([
            { "token": "surface.hover", "value": "#fff", "css": "foo" }
        ])))
        .unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(err.message.contains("raw colors or CSS"));
    }
}
