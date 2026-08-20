// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: error family.
use std::{error::Error, fmt};

use tokio::task;

use crate::protocol::RuntimeDiagnostic;
use crate::server::configuration::ConfigurationError;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ClayRuntimeEvaluation {
    pub(crate) op_records: Vec<String>,
    pub(crate) published_sdui_tree: Option<crate::protocol::SduiTree>,
    pub(crate) published_decoration_set: Option<crate::protocol::DecorationSet>,
    pub(crate) published_diagnostic_set: Option<crate::protocol::DiagnosticSet>,
    pub(crate) published_folding_set: Option<crate::protocol::FoldingRangeSet>, // FOLDING_RANGE_PAYLOAD_BUDGET_BYTES
    pub(crate) parse_handlers: Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
    pub(crate) js_parse_handlers: Vec<crate::server::parse_coordinator::JsParseHandlerRegistration>,
    pub(crate) behavior_manifest: Option<crate::protocol::BehaviorManifest>,
    pub(crate) ui_contributions: crate::server::ui::PackageUiRegistrySnapshot,
    pub(crate) syntax_grammars: Vec<crate::server::syntax::SyntaxGrammarContribution>,
    pub(crate) syntax_engine_preferences:
        std::collections::BTreeMap<String, crate::server::syntax::SyntaxEngineTier>,
    pub(crate) completion_providers: Vec<crate::server::completion::CompletionProviderMeta>,
    pub(crate) js_completion_providers:
        Vec<crate::server::completion::JsCompletionProviderRegistration>,
    pub(crate) language_intelligence_providers:
        Vec<crate::server::language_intelligence::LanguageIntelligenceProviderMeta>,
    pub(crate) js_language_intelligence_providers:
        Vec<crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration>,
    pub(crate) document_analyzers:
        Vec<crate::server::document_analysis::JsDocumentAnalyzerRegistration>,
    /// Resolved active theme snapshot from `setTheme` (`clay:theme` facade). `None`
    /// when `init.js` did not select a theme (Clay default applies). Applied to
    /// the shared server slot at load/reload so the welcome handshake ships it.
    pub(crate) active_theme: Option<crate::protocol::ActiveTheme>,
    /// Complete typography candidate from `setTypography`, if this evaluation
    /// configured one. The server assigns its authoritative revision only after
    /// the evaluation succeeds.
    pub(crate) active_typography: Option<crate::protocol::ActiveTypography>,
    /// Warnings emitted by optional configuration-module imports. These are
    /// drained from the configuration runtime before the evaluation is
    /// returned so reload callers can retain and report them.
    pub(crate) configuration_diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Debug)]
pub(crate) struct DocumentAnalysisInvocation {
    pub(crate) decorations: Option<crate::protocol::DecorationSet>,
    pub(crate) diagnostics: Option<crate::protocol::DiagnosticSet>,
    pub(crate) response: crate::server::document_analysis::DocumentAnalysisResponse,
}

#[derive(Debug)]
pub(crate) enum ClayRuntimeError {
    Configuration(ConfigurationError),
    InvalidMainSpecifier(String),
    Runtime(String),
    Timeout,
    HeapLimit,
    Join(task::JoinError),
}

impl fmt::Display for ClayRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "configuration error: {error}"),
            Self::InvalidMainSpecifier(message) => {
                write!(formatter, "invalid main module: {message}")
            }
            Self::Runtime(message) => write!(formatter, "JavaScript runtime error: {message}"),
            Self::Timeout => write!(
                formatter,
                "JavaScript runtime evaluation exceeded the configured timeout"
            ),
            Self::HeapLimit => write!(formatter, "JavaScript runtime exceeded the heap limit"),
            Self::Join(error) => write!(formatter, "JavaScript runtime task failed: {error}"),
        }
    }
}

impl Error for ClayRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Join(error) => Some(error),
            Self::InvalidMainSpecifier(_) | Self::Runtime(_) | Self::Timeout | Self::HeapLimit => {
                None
            }
        }
    }
}

impl ClayRuntimeError {
    pub(crate) fn diagnostic(&self) -> RuntimeDiagnostic {
        match self {
            Self::Configuration(error) => RuntimeDiagnostic::error(
                "configuration.invalid_module",
                configuration_diagnostic_message(&error.to_string()),
            ),
            Self::InvalidMainSpecifier(_) => RuntimeDiagnostic::error(
                "runtime.invalid_main",
                "Runtime configuration entry point could not be parsed.",
            ),
            Self::Runtime(message) => runtime_error_diagnostic(message),
            Self::Timeout => RuntimeDiagnostic::error(
                "runtime.timeout",
                "JavaScript runtime evaluation timed out and was terminated.",
            ),
            Self::HeapLimit => RuntimeDiagnostic::error(
                "runtime.heap_limit",
                "JavaScript runtime exceeded its heap budget and was terminated.",
            ),
            Self::Join(_) => RuntimeDiagnostic::error(
                "runtime.task_failed",
                "JavaScript runtime worker failed before configuration completed.",
            ),
        }
    }
}

pub(super) fn runtime_error_diagnostic(message: &str) -> RuntimeDiagnostic {
    let code = extract_clay_error_code(message).unwrap_or_else(|| {
        if message.contains("SyntaxError") {
            "runtime.syntax_error".to_string()
        } else {
            "runtime.exception".to_string()
        }
    });
    let detail = match code.as_str() {
        "runtime.invalid_import" => {
            "Only clay:* facades and relative local configuration modules are allowed."
        }
        "configuration.invalid_module" => {
            // Secure but actionable: name the allowed import families (clay:*
            // facades + relative local .js) so a typo (e.g. `clay:themes` vs
            // `clay:theme`) is diagnosable without echoing the rejected
            // specifier/URL/path (which must not leak).
            "Configuration import rejected: only clay:* facades (clay:theme, clay:configuration, clay:keybindings, clay:packages, clay:ui, clay:commands, ...) and explicit relative .js files under the configuration root are allowed. Check the import specifier spelling."
        }
        "runtime.syntax_error" => {
            "JavaScript syntax error while evaluating server-side configuration."
        }
        "runtime.invalid_record" => "Runtime op validation rejected an empty record.",
        "sdui.invalid_tree" => "Published SDUI tree failed server validation.",
        "sdui.invalid_action" => "Published SDUI action contains unsupported command authority.",
        "keybindings.unknown_command" => {
            "Key binding references an unknown or unsupported command."
        }
        code if code.starts_with("ui.") => {
            "Package UI contribution registration failed server validation."
        }
        code if code.starts_with("documents.") => {
            "Document/workspace operation failed server validation."
        }
        code if code.starts_with("workspace.") => "Workspace operation failed server validation.",
        _ => "JavaScript runtime evaluation failed.",
    };

    RuntimeDiagnostic::error(code, detail)
}

pub(super) fn configuration_diagnostic_message(message: &str) -> String {
    // Secure but actionable: do not echo the rejected specifier/URL/path
    // (which must not leak). Name the allowed import families so a config
    // typo is diagnosable. `message` is only inspected to distinguish the
    // entry-point case; its contents are never surfaced.
    if message.contains("init.js") {
        "Configuration entry point init.js could not be loaded.".to_string()
    } else {
        "Configuration import rejected: only clay:* facades (clay:theme, clay:configuration, clay:keybindings, clay:packages, clay:ui, clay:commands, ...) and explicit relative .js files under the configuration root are allowed. Check the import specifier spelling.".to_string()
    }
}

/// Extract the human-readable detail from a `configuration.invalid_module: <detail>`
/// JS error string so runtime-routed configuration errors name the rejected
/// module/path instead of an opaque generic message.
pub(super) fn configuration_runtime_detail(message: &str) -> Option<&str> {
    let prefix = "configuration.invalid_module:";
    message
        .strip_prefix(prefix)
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

pub(super) fn extract_clay_error_code(message: &str) -> Option<String> {
    message
        .split(|character: char| character.is_whitespace() || character == ':' || character == '`')
        .find(|part| {
            part.contains('.')
                && part.chars().all(is_error_code_character)
                && part.split('.').next().is_some_and(|domain| {
                    crate::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&domain)
                })
        })
        .map(ToOwned::to_owned)
}

pub(super) fn is_error_code_character(character: char) -> bool {
    character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '.'
        || character == '_'
}
