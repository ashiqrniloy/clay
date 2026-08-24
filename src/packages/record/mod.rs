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
use std::collections::BTreeMap;

use serde_json::Value;

use crate::packages::manifest::{
    ClayPackageManifest, DiagnosticContext, PackageDiagnostic, expand_capability_preset,
    validate_manifest_value,
};
use crate::packages::permissions::PackagePermission;

use crate::protocol::{
    CompletionItemTextFormat, DocumentFontRole, LanguageIntelligenceFeature, Modifiers, TokenType,
};

// ── Contribution descriptors ─────────────────────────────────────────────────
mod behavior;
mod documentation;
mod language;
mod theme;
mod ui;

/// Inert descriptor for a major-mode pattern declared by a package.
///
/// `editor_rules_json` is the raw `editorRules` object (if any). It is parsed
/// into [`crate::protocol::EditorBehaviorRules`] at load/activation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModePatternContributionDescriptor {
    pub mode_id: String,
    pub display_name: String,
    pub document_font_role: DocumentFontRole,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
    pub file_names: Vec<String>,
    pub file_name_patterns: Vec<String>,
    pub shebang_patterns: Vec<String>,
    pub content_probes: Vec<String>,
    pub editor_rules_json: Option<String>,
}

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
    /// Optional fill override, present only when the entry declares `background`.
    pub background: Option<[u8; 4]>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// Size-ladder thousandths (`1500` = 1.5).
    pub scale: Option<u16>,
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
                .map(|[r, g, b, a]| crate::color::Color::from_rgba8(r, g, b, a)),
            background: self
                .background
                .map(|[r, g, b, a]| crate::color::Color::from_rgba8(r, g, b, a)),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strike: self.strike,
            scale: self.scale.map(crate::editor::theme::scale_from_milli),
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
    pub mode_patterns: Vec<ModePatternContributionDescriptor>,
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
    /// Stable Clay JS API ID (e.g. `modes.serverRegisterModePattern`).
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
    let context = DiagnosticContext::new(
        value
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        value
            .get("version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        value
            .get("clay")
            .and_then(|clay| clay.get("apiPrefix"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    );
    let value = expand_capability_preset(value, &context)
        .map_err(PackageRecordError::from_manifest_diagnostic)?;
    // Step 1: reuse Phase 16.5 manifest validator.
    let manifest =
        validate_manifest_value(&value).map_err(PackageRecordError::from_manifest_diagnostic)?;

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
    let docs = documentation::parse_docs_metadata(clay.get("docs"), &ctx)?;

    // Step 4: performance metadata.
    let performance =
        documentation::parse_performance_metadata(clay.get("performance"), &value, &ctx)?;

    // Step 5: API dependencies (optional).
    let api_dependencies =
        documentation::parse_api_dependencies(clay.get("apiDependencies"), &ctx)?;
    documentation::validate_api_dependency_permissions(
        &api_dependencies,
        &manifest.clay.permissions,
        &ctx,
    )?;

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

    let mode_patterns = match map.get("modePatterns") {
        Some(v) => behavior::parse_mode_pattern_contributions(
            v,
            api_prefix,
            permissions,
            package_modes,
            ctx,
        )?,
        None => Vec::new(),
    };
    let commands = match map.get("commands") {
        Some(v) => behavior::parse_command_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let configuration = match map.get("configuration") {
        Some(v) => behavior::parse_configuration_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let key_routing = match map.get("keyRouting") {
        Some(v) => behavior::parse_key_routing_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let text_transforms = match map.get("textTransforms") {
        Some(v) => behavior::parse_text_transform_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let sdui = match map.get("sdui") {
        Some(v) => ui::parse_sdui_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let decorations = match map.get("decorations") {
        Some(v) => ui::parse_decoration_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let syntax_grammars = match map.get("syntaxGrammars") {
        Some(v) => language::parse_syntax_grammar_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let completion_providers = match map.get("completionProviders") {
        Some(v) => {
            language::parse_completion_provider_contributions(v, api_prefix, permissions, ctx)?
        }
        None => Vec::new(),
    };
    let language_servers = match map.get("languageServers") {
        Some(v) => language::parse_language_server_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let language_intelligence_providers = match map.get("languageIntelligenceProviders") {
        Some(v) => language::parse_language_intelligence_provider_contributions(
            v,
            api_prefix,
            permissions,
            ctx,
        )?,
        None => Vec::new(),
    };
    let theme_tokens = match map.get("themeTokens") {
        Some(v) => theme::parse_theme_token_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let text_styles = match map.get("textStyles") {
        Some(v) => theme::parse_text_style_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let design_tokens = match map.get("designTokens") {
        Some(v) => theme::parse_design_token_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let registered_command_ids: Vec<String> =
        commands.iter().map(|command| command.id.clone()).collect();
    let theme_resolver = theme::theme_resolver_for_package_tokens(&theme_tokens);
    let (ui_panels, ui_components, ui_overlays) = match map.get("ui") {
        Some(v) => ui::parse_ui_contributions(
            v,
            api_prefix,
            &registered_command_ids,
            &theme_resolver,
            ctx,
        )?,
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let input_contributions = match map.get("input") {
        Some(v) => ui::parse_input_contributions(
            v,
            api_prefix,
            package_modes,
            &registered_command_ids,
            ctx,
        )?,
        None => Vec::new(),
    };
    let ui_state_scopes = match map.get("uiStateScopes") {
        Some(v) => ui::parse_ui_state_scope_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let package_options = match map.get("packageOptions") {
        Some(v) => behavior::parse_package_option_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let layout_overrides = match map.get("layoutOverrides") {
        Some(v) => ui::parse_layout_override_contributions(
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
        mode_patterns,
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
                    "modes.serverRegisterModePattern",
                    "commands.serverRegisterCommand"
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
            "modes.serverRegisterModePattern"
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
        assert!(record.contributions.mode_patterns.is_empty());
    }

    #[test]
    fn package_record_parses_mode_patterns_and_editor_rules() {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["modePatterns"] = json!([{
            "mode": "markdown",
            "displayName": "Markdown",
            "extensions": ["md"],
            "defaultFontRole": "proportional",
            "editorRules": {
                "tabSpaces": 2,
                "pairs": [{ "open": "`", "close": "`" }]
            }
        }]);
        let record = assemble_package_record(&fixture).expect("modePatterns must parse");
        assert_eq!(record.contributions.mode_patterns.len(), 1);
        let pattern = &record.contributions.mode_patterns[0];
        assert_eq!(pattern.mode_id, "markdown");
        assert_eq!(pattern.extensions, ["md"]);
        assert!(
            pattern
                .editor_rules_json
                .as_ref()
                .is_some_and(|json| json.contains("tabSpaces"))
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

    #[test]
    fn assemble_expands_code_mode_api_dependencies() {
        let record = assemble_package_record(&json!({
            "name": "@clay/demo",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "demo",
                "preset": "code-mode",
                "entry": "./dist/index.js",
                "modes": ["demo"],
                "docs": "./docs/index.md"
            }
        }))
        .expect("code-mode preset assembles");
        assert!(record.api_dependencies.iter().any(|dependency| {
            dependency.api_id == "completion.serverRegisterCompletionProvider"
        }));
        assert!(!record.manifest.clay.extension_points.is_empty());
    }

    #[test]
    fn assemble_lsp_bridge_without_capability_does_not_grant_language_server() {
        let record = assemble_package_record(&json!({
            "name": "@clay/lsp-demo",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "lsp-demo",
                "preset": "lsp-bridge",
                "entry": "./dist/index.js",
                "modes": [],
                "docs": "./docs/index.md"
            }
        }))
        .expect("lsp-bridge without language-server capability assembles");
        assert!(
            !record
                .manifest
                .clay
                .permissions
                .contains(&PackagePermission::LanguageServer)
        );
        assert!(record.contributions.language_servers.is_empty());
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
            Some(crate::color::Color::from_rgba8(0xc7, 0x92, 0xea, 0xff))
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

    fn minimal_manifest_value(clay_extras: Value) -> Value {
        let mut clay = json!({
            "apiPrefix": "demo",
            "entry": "./dist/index.js",
            "permissions": [],
            "modes": []
        });
        if let (Some(object), Some(extras)) = (clay.as_object_mut(), clay_extras.as_object()) {
            for (key, value) in extras {
                object.insert(key.clone(), value.clone());
            }
        }
        json!({
            "name": "@demo/pkg",
            "version": "0.1.0",
            "clay": clay
        })
    }

    #[test]
    fn editor_control_modes_parse_exact_foreign_modes_and_default_empty() {
        use crate::packages::manifest::validate_manifest_value;

        // Absent block yields an empty declaration.
        let manifest = validate_manifest_value(&minimal_manifest_value(json!({}))).unwrap();
        assert!(manifest.clay.editor_control_modes.is_empty());

        // With `editor-control` permission, exact foreign modes parse.
        let value = minimal_manifest_value(json!({
            "permissions": ["editor-control"],
            "editorControl": { "modes": ["core.code", "markdown"] }
        }));
        let manifest = validate_manifest_value(&value).unwrap();
        assert_eq!(
            manifest.clay.editor_control_modes,
            vec!["core.code".to_string(), "markdown".to_string()]
        );
        assert!(
            manifest
                .clay
                .permissions
                .contains(&crate::packages::permissions::PackagePermission::EditorControl)
        );
    }

    #[test]
    fn editor_control_requires_permission_deny_by_default() {
        use crate::packages::manifest::validate_manifest_value;
        let value = minimal_manifest_value(json!({
            "editorControl": { "modes": ["core.code"] }
        }));
        let err = validate_manifest_value(&value).unwrap_err();
        assert!(err.message.contains("editor-control"), "{}", err.message);
    }

    #[test]
    fn editor_control_rejects_unknown_key_and_wildcard_deny_by_default() {
        use crate::packages::manifest::validate_manifest_value;

        let value = minimal_manifest_value(json!({
            "permissions": ["editor-control"],
            "editorControl": { "modes": [], "everything": true }
        }));
        let err = validate_manifest_value(&value).unwrap_err();
        assert!(
            err.message.contains("unknown clay.editorControl key"),
            "{}",
            err.message
        );

        let value = minimal_manifest_value(json!({
            "permissions": ["editor-control"],
            "editorControl": { "modes": ["core.*"] }
        }));
        let err = validate_manifest_value(&value).unwrap_err();
        assert!(err.message.contains("no wildcards"), "{}", err.message);
    }
}
