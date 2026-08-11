use std::{
    collections::VecDeque,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use serde_json::Value;

use crate::packages::manifest::is_valid_api_prefix;

const PACKAGE_OPTION_PAYLOAD_BUDGET_BYTES: usize = 16 * 1024;
const MODULE_ERROR_CAPACITY: usize = 64;
const MODULE_ERROR_MESSAGE_BUDGET_BYTES: usize = 1024;
const PACKAGE_OPTION_SOURCES: &[&str] =
    &["init-js", "package-default", "clay-default", "ui-session"];
/// Bounded persisted user preferences (`~/.config/clay/preferences.json`). The
/// file is a closed JSON object: only `theme`, `appearance`, and `typography`
/// keys are recognized; unknown keys are dropped with a diagnostic. Values are
/// validated at load and persist time so a corrupted/manually-edited file falls
/// back safely without granting authority.
const PREFERENCES_PAYLOAD_BUDGET_BYTES: usize = 8 * 1024;
const PREFERENCES_KEYS: &[&str] = &["theme", "appearance", "typography"];
const PREFERENCES_APPEARANCE_VALUES: &[&str] = &["light", "dark", "system"];
const PANEL_VISIBILITY_VALUES: &[&str] = &["visible", "hidden", "collapsed"];
const PANEL_SLOT_VALUES: &[&str] = &["left", "right", "top", "bottom"];
const FALLBACK_VALUES: &[&str] = &["package-default", "hide", "ignore"];

/// Server-side configuration root and local module loading state.
#[derive(Debug)]
pub(crate) struct ConfigurationRuntime {
    config_root: PathBuf,
    entry_point: PathBuf,
    loaded_modules: Mutex<Vec<PathBuf>>,
    module_errors: Mutex<VecDeque<ConfigurationModuleError>>,
    package_options: Mutex<Vec<RegisteredPackageOption>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigurationModuleError {
    pub(crate) path: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegisteredPackageOption {
    pub(crate) package_prefix: String,
    pub(crate) option: String,
    pub(crate) value: Value,
    pub(crate) source: String,
    pub(crate) estimated_payload_bytes: usize,
}

impl ConfigurationRuntime {
    pub(crate) fn from_config_root(
        config_root: impl AsRef<Path>,
    ) -> Result<Self, ConfigurationError> {
        let config_root =
            fs::canonicalize(config_root.as_ref()).map_err(ConfigurationError::Root)?;
        if !config_root.is_dir() {
            return Err(ConfigurationError::InvalidRoot(format!(
                "configuration root {} is not a directory",
                config_root.display()
            )));
        }

        let entry_point = canonical_local_file(&config_root, &config_root.join("init.js"))?;
        Ok(Self {
            config_root,
            entry_point,
            loaded_modules: Mutex::new(Vec::new()),
            module_errors: Mutex::new(VecDeque::new()),
            package_options: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn default_config_root() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .map(|home| home.join(".config").join("clay"))
    }

    pub(crate) fn entry_specifier(&self) -> Result<ModuleSpecifier, ConfigurationError> {
        ModuleSpecifier::from_file_path(&self.entry_point).map_err(|()| {
            ConfigurationError::InvalidModule(format!(
                "configuration entry point {} cannot be represented as a file URL",
                self.entry_point.display()
            ))
        })
    }

    pub(crate) fn validate_module_path(
        &self,
        path: &str,
        allow_missing: bool,
    ) -> Result<(), ConfigurationError> {
        if !allow_missing {
            return self.resolve_from_entry(path).map(|_| ());
        }
        reject_non_local_specifier(path)?;
        validate_local_module_path_allow_missing(&self.config_root, &self.config_root.join(path))
            .map(|_| ())
    }

    pub(crate) fn record_module_error(
        &self,
        path: &str,
        message: &str,
    ) -> Result<(), ConfigurationError> {
        reject_non_local_specifier(path)?;
        let module_path = validate_local_module_path_allow_missing(
            &self.config_root,
            &self.config_root.join(path),
        )?;
        let error = ConfigurationModuleError {
            path: display_relative_to(&self.config_root, &module_path),
            message: truncate_utf8(message, MODULE_ERROR_MESSAGE_BUDGET_BYTES),
        };
        let mut errors = self
            .module_errors
            .lock()
            .expect("configuration module error mutex poisoned");
        if errors.len() >= MODULE_ERROR_CAPACITY {
            errors.pop_front();
        }
        errors.push_back(error);
        Ok(())
    }

    pub(crate) fn take_module_errors(&self) -> Vec<ConfigurationModuleError> {
        self.module_errors
            .lock()
            .expect("configuration module error mutex poisoned")
            .drain(..)
            .collect()
    }

    pub(crate) fn resolve_module(
        &self,
        specifier: &str,
        referrer: &str,
    ) -> Result<ModuleSpecifier, ConfigurationError> {
        reject_non_local_specifier(specifier)?;

        let base_directory = if referrer == "clay:configuration" {
            self.config_root.clone()
        } else {
            let referrer = ModuleSpecifier::parse(referrer)
                .map_err(|error| ConfigurationError::InvalidModule(error.to_string()))?;
            let referrer_path = referrer.to_file_path().map_err(|()| {
                ConfigurationError::InvalidModule(format!(
                    "configuration referrer {referrer} is not a local file"
                ))
            })?;
            referrer_path
                .parent()
                .ok_or_else(|| {
                    ConfigurationError::InvalidModule(format!(
                        "configuration referrer {} has no parent directory",
                        referrer_path.display()
                    ))
                })?
                .to_path_buf()
        };

        let module_path = canonical_local_file(&self.config_root, &base_directory.join(specifier))?;
        ModuleSpecifier::from_file_path(&module_path).map_err(|()| {
            ConfigurationError::InvalidModule(format!(
                "configuration module {} cannot be represented as a file URL",
                module_path.display()
            ))
        })
    }

    pub(crate) fn resolve_from_entry(
        &self,
        specifier: &str,
    ) -> Result<ModuleSpecifier, ConfigurationError> {
        self.resolve_module(specifier, "clay:configuration")
    }

    pub(crate) fn load_module_source(
        &self,
        module_specifier: &ModuleSpecifier,
    ) -> Result<String, ConfigurationError> {
        let module_path = module_specifier.to_file_path().map_err(|()| {
            ConfigurationError::InvalidModule(format!(
                "configuration module {module_specifier} is not a local file"
            ))
        })?;
        let module_path = canonical_local_file(&self.config_root, &module_path)?;
        let source = fs::read_to_string(&module_path).map_err(ConfigurationError::ReadModule)?;
        if module_path != self.entry_point {
            self.record_loaded_module(module_path);
        }
        Ok(source)
    }

    pub(crate) fn state_json(&self) -> String {
        let loaded_modules: Vec<String> = self
            .loaded_modules
            .lock()
            .expect("configuration module state mutex poisoned")
            .iter()
            .map(|path| display_relative_to(&self.config_root, path))
            .collect();
        let package_options = self
            .package_options
            .lock()
            .expect("configuration package option state mutex poisoned")
            .clone();
        serde_json::json!({
            "entryPoint": display_relative_to(&self.config_root, &self.entry_point),
            "loadedModules": loaded_modules,
            "packageOptions": package_options.iter().map(|option| serde_json::json!({
                "packagePrefix": option.package_prefix,
                "option": option.option,
                "value": option.value,
                "source": option.source,
                "estimatedPayloadBytes": option.estimated_payload_bytes,
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    pub(crate) fn set_package_option(
        &self,
        value: &Value,
    ) -> Result<RegisteredPackageOption, ConfigurationError> {
        let size = serde_json::to_vec(value)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        if size > PACKAGE_OPTION_PAYLOAD_BUDGET_BYTES {
            return Err(ConfigurationError::InvalidPackageOption(format!(
                "package option payload ({size} bytes) exceeds {PACKAGE_OPTION_PAYLOAD_BUDGET_BYTES} bytes"
            )));
        }
        reject_prohibited_authority(value)?;
        let object = value.as_object().ok_or_else(|| {
            ConfigurationError::InvalidPackageOption(
                "setPackageOption requires an object".to_string(),
            )
        })?;
        let package_prefix = required_string(object, "packagePrefix")?;
        if !is_valid_api_prefix(package_prefix) {
            return Err(ConfigurationError::InvalidPackageOption(
                "packagePrefix must be a valid package apiPrefix".to_string(),
            ));
        }
        let option = required_string(object, "option")?;
        validate_package_option_name(package_prefix, option)?;
        let option_value = object.get("value").ok_or_else(|| {
            ConfigurationError::InvalidPackageOption(
                "setPackageOption requires a typed value".to_string(),
            )
        })?;
        validate_package_option_value(package_prefix, option, option_value)?;
        let source = object
            .get("source")
            .and_then(Value::as_str)
            .filter(|source| !source.trim().is_empty())
            .unwrap_or("init-js");
        if !PACKAGE_OPTION_SOURCES.contains(&source) {
            return Err(ConfigurationError::InvalidPackageOption(
                "source must be init-js, package-default, clay-default, or ui-session".to_string(),
            ));
        }
        let registered = RegisteredPackageOption {
            package_prefix: package_prefix.to_string(),
            option: option.to_string(),
            value: option_value.clone(),
            source: source.to_string(),
            estimated_payload_bytes: size,
        };
        self.package_options
            .lock()
            .expect("configuration package option state mutex poisoned")
            .push(registered.clone());
        Ok(registered)
    }

    fn record_loaded_module(&self, module_path: PathBuf) {
        let mut loaded_modules = self
            .loaded_modules
            .lock()
            .expect("configuration module state mutex poisoned");
        if !loaded_modules.iter().any(|loaded| loaded == &module_path) {
            loaded_modules.push(module_path);
        }
    }

    /// Path to the persisted user-preferences file (`preferences.json`) inside
    /// the configuration root. Phase 20.6 configuration-precedence store.
    pub(crate) fn preferences_path(&self) -> PathBuf {
        self.config_root.join("preferences.json")
    }

    /// Load and validate persisted user preferences. A missing file yields an
    /// empty preference set. A present but malformed/oversized file, or one
    /// carrying an unknown key or an invalid value, is dropped field-by-field
    /// with diagnostics recorded in [`PersistedPreferences::diagnostics`] so
    /// startup never breaks and no authority is granted by a corrupted file.
    /// Closed surface: only `theme`, `appearance`, `typography` are read.
    pub(crate) fn load_preferences(&self) -> PersistedPreferences {
        let bytes = match fs::read(self.preferences_path()) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return PersistedPreferences::default();
            }
            Err(error) => {
                return PersistedPreferences {
                    diagnostics: vec![format!(
                        "preferences.json is unreadable: {error}; ignoring persisted preferences"
                    )],
                    ..Default::default()
                };
            }
        };
        if bytes.len() > PREFERENCES_PAYLOAD_BUDGET_BYTES {
            return PersistedPreferences {
                diagnostics: vec![format!(
                    "preferences.json payload ({} bytes) exceeds {} bytes; ignoring persisted preferences",
                    bytes.len(),
                    PREFERENCES_PAYLOAD_BUDGET_BYTES
                )],
                ..Default::default()
            };
        }
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(error) => {
                return PersistedPreferences {
                    diagnostics: vec![format!(
                        "preferences.json is not valid JSON: {error}; ignoring persisted preferences"
                    )],
                    ..Default::default()
                };
            }
        };
        let Some(object) = value.as_object() else {
            return PersistedPreferences {
                diagnostics: vec![
                    "preferences.json root must be an object; ignoring persisted preferences"
                        .to_string(),
                ],
                ..Default::default()
            };
        };
        let mut prefs = PersistedPreferences::default();
        for (key, field) in object {
            if !PREFERENCES_KEYS.contains(&key.as_str()) {
                prefs.diagnostics.push(format!(
                    "preferences.json key `{key}` is not recognized; dropping"
                ));
                continue;
            }
            // `null` means "absent" (write_preferences always emits all three
            // keys); skip it so a serialized absence does not log a diagnostic.
            if field.is_null() {
                continue;
            }
            match key.as_str() {
                "theme" => match validate_preference_theme(field) {
                    Ok(specifier) => prefs.theme = Some(specifier),
                    Err(reason) => prefs.diagnostics.push(reason),
                },
                "appearance" => match validate_preference_appearance(field) {
                    Ok(appearance) => prefs.appearance = Some(appearance),
                    Err(reason) => prefs.diagnostics.push(reason),
                },
                "typography" => match validate_preference_typography(field) {
                    Ok(()) => prefs.typography = Some(field.clone()),
                    Err(reason) => prefs.diagnostics.push(reason),
                },
                _ => unreachable!("PREFERENCES_KEYS bounds the match"),
            }
        }
        prefs
    }

    /// Persist a single validated preference field, merging with any existing
    /// file. Atomic (tmp + rename), bounded, non-blocking. Returns the merged
    /// preferences that will be loaded on the next reload.
    pub(crate) fn persist_preference(
        &self,
        key: &str,
        value: Value,
    ) -> Result<PersistedPreferences, ConfigurationError> {
        let mut prefs = self.load_preferences();
        prefs.diagnostics.clear();
        match key {
            "theme" => validate_preference_theme(&value)
                .map(|specifier| prefs.theme = Some(specifier))
                .map_err(ConfigurationError::InvalidPackageOption)?,
            "appearance" => validate_preference_appearance(&value)
                .map(|appearance| prefs.appearance = Some(appearance))
                .map_err(ConfigurationError::InvalidPackageOption)?,
            "typography" => validate_preference_typography(&value)
                .map(|_| prefs.typography = Some(value))
                .map_err(ConfigurationError::InvalidPackageOption)?,
            _ => {
                return Err(ConfigurationError::InvalidPackageOption(format!(
                    "preferences key `{key}` is not recognized"
                )));
            }
        }
        self.write_preferences(&prefs)
    }

    /// Remove every persisted preference field (settings.reset). Atomic.
    pub(crate) fn clear_preferences(&self) -> Result<(), ConfigurationError> {
        self.write_preferences(&PersistedPreferences::default())?;
        Ok(())
    }

    fn write_preferences(
        &self,
        prefs: &PersistedPreferences,
    ) -> Result<PersistedPreferences, ConfigurationError> {
        let object = serde_json::json!({
            "theme": prefs.theme,
            "appearance": prefs.appearance.map(crate::protocol::Appearance::as_str),
            "typography": prefs.typography,
        });
        let bytes = serde_json::to_vec(&object).map_err(|error| {
            ConfigurationError::InvalidPackageOption(format!(
                "preferences serialization failed: {error}"
            ))
        })?;
        if bytes.len() > PREFERENCES_PAYLOAD_BUDGET_BYTES {
            return Err(ConfigurationError::InvalidPackageOption(format!(
                "preferences payload ({} bytes) exceeds {} bytes",
                bytes.len(),
                PREFERENCES_PAYLOAD_BUDGET_BYTES
            )));
        }
        let final_path = self.preferences_path();
        let tmp_path = self.config_root.join(".preferences.json.tmp");
        fs::write(&tmp_path, &bytes).map_err(ConfigurationError::Root)?;
        fs::rename(&tmp_path, &final_path).map_err(ConfigurationError::Root)?;
        Ok(prefs.clone())
    }
}

#[derive(Debug)]
pub(crate) enum ConfigurationError {
    Root(io::Error),
    ReadModule(io::Error),
    InvalidRoot(String),
    InvalidModule(String),
    InvalidPackageOption(String),
}

impl ConfigurationError {
    pub(crate) fn to_js_error(&self) -> JsErrorBox {
        match self {
            Self::InvalidPackageOption(_) => {
                JsErrorBox::generic(format!("configuration.invalid_package_option: {self}"))
            }
            _ => JsErrorBox::generic(format!("configuration.invalid_module: {self}")),
        }
    }
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot read configuration root: {error}"),
            Self::ReadModule(error) => {
                write!(formatter, "cannot read configuration module: {error}")
            }
            Self::InvalidRoot(message)
            | Self::InvalidModule(message)
            | Self::InvalidPackageOption(message) => formatter.write_str(message),
        }
    }
}

impl Error for ConfigurationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root(error) | Self::ReadModule(error) => Some(error),
            Self::InvalidRoot(_) | Self::InvalidModule(_) | Self::InvalidPackageOption(_) => None,
        }
    }
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, ConfigurationError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ConfigurationError::InvalidPackageOption(format!("{key} must be a non-empty string"))
        })
}

fn validate_package_option_name(
    package_prefix: &str,
    option: &str,
) -> Result<(), ConfigurationError> {
    if option.starts_with("clay.")
        || !option.starts_with(&format!("{package_prefix}."))
        || option
            .split('.')
            .any(|segment| segment.is_empty() || segment.starts_with('_'))
    {
        return Err(ConfigurationError::InvalidPackageOption(
            "option must be package-prefixed and must not use hidden or empty path segments"
                .to_string(),
        ));
    }
    let suffix = option
        .strip_prefix(&format!("{package_prefix}."))
        .unwrap_or(option);
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
        return Err(ConfigurationError::InvalidPackageOption(
            "unsupported package option; use documented layout.defaultVisibility, layout.defaultSlot, layout.splitRatio, input.default, action.default, themeTokenRemap, or fallback options".to_string(),
        ));
    }
    Ok(())
}

fn validate_package_option_value(
    package_prefix: &str,
    option: &str,
    value: &Value,
) -> Result<(), ConfigurationError> {
    let suffix = option
        .strip_prefix(&format!("{package_prefix}."))
        .unwrap_or(option);
    match suffix {
        "layout.defaultVisibility" => {
            validate_string_enum(value, PANEL_VISIBILITY_VALUES, "layout.defaultVisibility")
        }
        "layout.defaultSlot" => {
            validate_string_enum(value, PANEL_SLOT_VALUES, "layout.defaultSlot")
        }
        "layout.splitRatio" => {
            let ratio = value.as_f64().ok_or_else(|| {
                ConfigurationError::InvalidPackageOption(
                    "layout.splitRatio value must be a number".to_string(),
                )
            })?;
            if !(0.1..=0.9).contains(&ratio) {
                return Err(ConfigurationError::InvalidPackageOption(
                    "layout.splitRatio value must be between 0.1 and 0.9".to_string(),
                ));
            }
            Ok(())
        }
        "input.default" | "action.default" => {
            let id = value.as_str().ok_or_else(|| {
                ConfigurationError::InvalidPackageOption(
                    "input.default and action.default values must be package-prefixed strings"
                        .to_string(),
                )
            })?;
            if !id.starts_with(&format!("{package_prefix}."))
                || id
                    .split('.')
                    .any(|segment| segment.is_empty() || segment.starts_with('_'))
            {
                return Err(ConfigurationError::InvalidPackageOption(
                    "input.default and action.default values must use package-prefixed public IDs"
                        .to_string(),
                ));
            }
            Ok(())
        }
        "themeTokenRemap" => {
            let object = value.as_object().ok_or_else(|| {
                ConfigurationError::InvalidPackageOption(
                    "themeTokenRemap value must be { token, fallback }".to_string(),
                )
            })?;
            let token = required_string(object, "token")?;
            if !token.starts_with(&format!("{package_prefix}.")) {
                return Err(ConfigurationError::InvalidPackageOption(
                    "themeTokenRemap.token must use the package apiPrefix".to_string(),
                ));
            }
            let fallback = required_string(object, "fallback")?;
            // Core theme tokens are bare `<domain>.<name>` (e.g.
            // `surface.panel`); a fallback must not be the remapping
            // package's own token or a retired `clay.*` spelling.
            if fallback.starts_with("clay.")
                || fallback.starts_with(&format!("{package_prefix}."))
                || !fallback.contains('.')
            {
                return Err(ConfigurationError::InvalidPackageOption(
                    "themeTokenRemap.fallback must reference a Clay core theme token (bare `<domain>.<name>`, e.g. surface.panel); same-type checks happen when a registered theme token is remapped through clay:ui".to_string(),
                ));
            }
            Ok(())
        }
        "fallback" => validate_string_enum(value, FALLBACK_VALUES, "fallback"),
        _ => unreachable!("package option suffix validated before value validation"),
    }
}

fn validate_string_enum(
    value: &Value,
    allowed: &[&str],
    option: &str,
) -> Result<(), ConfigurationError> {
    let text = value.as_str().ok_or_else(|| {
        ConfigurationError::InvalidPackageOption(format!("{option} value must be a string"))
    })?;
    if !allowed.contains(&text) {
        return Err(ConfigurationError::InvalidPackageOption(format!(
            "{option} value is not supported"
        )));
    }
    Ok(())
}

fn reject_prohibited_authority(value: &Value) -> Result<(), ConfigurationError> {
    match value {
        Value::String(text) if text.contains("Deno.core.ops") || text.contains("op_clay_") => {
            Err(ConfigurationError::InvalidPackageOption(
                "package options must not expose raw Deno.core.ops or op names".to_string(),
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
                        | "clientHook"
                        | "clientJavaScript"
                        | "javascript"
                        | "code"
                        | "rawCss"
                        | "cssText"
                        | "defaultValue"
                        | "initialValue"
                        | "rawValue"
                ) {
                    return Err(ConfigurationError::InvalidPackageOption(
                        "package options must not include raw ops, native widgets, raw CSS, callbacks, client-side JavaScript, or state values".to_string(),
                    ));
                }
                reject_prohibited_authority(nested)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_prohibited_authority(nested)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn reject_non_local_specifier(specifier: &str) -> Result<(), ConfigurationError> {
    if specifier.trim().is_empty() {
        return Err(ConfigurationError::InvalidModule(
            "configuration module path must not be empty".to_string(),
        ));
    }
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module `{specifier}` must be a relative local path"
        )));
    }
    if specifier.contains(':') {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module `{specifier}` must not be a URL or package specifier"
        )));
    }
    let path = Path::new(specifier);
    if path.is_absolute() {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module `{specifier}` must not be absolute"
        )));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("js") {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module `{specifier}` must include an explicit .js extension"
        )));
    }
    Ok(())
}

fn canonical_local_file(config_root: &Path, path: &Path) -> Result<PathBuf, ConfigurationError> {
    let path = fs::canonicalize(path).map_err(ConfigurationError::ReadModule)?;
    if !path.starts_with(config_root) {
        return Err(ConfigurationError::InvalidModule(
            "configuration module must stay inside the Clay configuration directory".to_string(),
        ));
    }
    if !path.is_file() {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module {} is not a file",
            path.display()
        )));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("js") {
        return Err(ConfigurationError::InvalidModule(format!(
            "configuration module {} must use an explicit .js extension",
            path.display()
        )));
    }
    Ok(path)
}

/// Validate optional-load containment without requiring the final file to
/// exist. Import still performs the authoritative read/canonical-file check;
/// this only lets an optional missing module become a caught import failure.
fn validate_local_module_path_allow_missing(
    config_root: &Path,
    path: &Path,
) -> Result<PathBuf, ConfigurationError> {
    let mut existing = path;
    let mut missing = Vec::new();
    while !existing.exists() {
        let component = existing.file_name().ok_or_else(|| {
            ConfigurationError::InvalidModule(
                "configuration module path has no local file name".to_string(),
            )
        })?;
        missing.push(component.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            ConfigurationError::InvalidModule(
                "configuration module path has no local parent".to_string(),
            )
        })?;
    }

    let canonical_existing = fs::canonicalize(existing).map_err(ConfigurationError::ReadModule)?;
    let mut candidate = canonical_existing;
    for component in missing.iter().rev() {
        candidate.push(component);
    }
    if !candidate.starts_with(config_root) {
        return Err(ConfigurationError::InvalidModule(
            "configuration module must stay inside the Clay configuration directory".to_string(),
        ));
    }
    Ok(candidate)
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn display_relative_to(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    format!("./{}", relative.to_string_lossy().replace('\\', "/"))
}

/// Phase 20.6 persisted user preferences loaded from `preferences.json`. Each
/// field is optional; an absent field means "no UI-session override, defer to
/// init.js / canonical default". `typography` is the raw validated JSON value;
/// the `setTypography` op re-validates bounds at apply time. `diagnostics`
/// carries fallback reasons for any field that was dropped during load so a
/// corrupted/manually-edited file never breaks startup.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PersistedPreferences {
    pub(crate) theme: Option<String>,
    pub(crate) appearance: Option<crate::protocol::Appearance>,
    pub(crate) typography: Option<Value>,
    pub(crate) diagnostics: Vec<String>,
}

/// Validate a persisted `theme` value: must be a first-party bundled
/// `@clay/theme-*` specifier. Rejects arbitrary specifiers so a corrupted file
/// cannot point the active theme at a non-bundled package.
fn validate_preference_theme(value: &Value) -> Result<String, String> {
    let specifier = value
        .as_str()
        .ok_or_else(|| "preferences.json `theme` must be a string; dropping".to_string())?;
    if !specifier.starts_with("@clay/theme-")
        || crate::packages::bundled::bundled_entry(specifier).is_none()
    {
        return Err(format!(
            "preferences.json `theme` `{specifier}` is not a bundled first-party @clay/theme-* specifier; dropping"
        ));
    }
    Ok(specifier.to_string())
}

/// Validate a persisted `appearance` value: bounded light/dark/system enum.
fn validate_preference_appearance(value: &Value) -> Result<crate::protocol::Appearance, String> {
    let text = value
        .as_str()
        .ok_or_else(|| "preferences.json `appearance` must be a string; dropping".to_string())?;
    if !PREFERENCES_APPEARANCE_VALUES.contains(&text) {
        return Err(format!(
            "preferences.json `appearance` `{text}` is not light/dark/system; dropping"
        ));
    }
    crate::protocol::Appearance::parse(text)
        .ok_or_else(|| format!("preferences.json `appearance` `{text}` failed to parse; dropping"))
}

/// Validate a persisted `typography` value structurally (the `setTypography`
/// op re-validates numeric bounds at apply time). Rejects authority-bearing
/// shapes (raw ops/css/callbacks) via [`reject_prohibited_authority`].
fn validate_preference_typography(value: &Value) -> Result<(), String> {
    reject_prohibited_authority(value).map_err(|error| {
        format!("preferences.json `typography` carries prohibited authority: {error}; dropping")
    })?;
    if !value.is_object() {
        return Err("preferences.json `typography` must be an object; dropping".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    fn runtime() -> ConfigurationRuntime {
        let root = std::env::temp_dir().join(format!(
            "clay-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp config root");
        fs::write(root.join("init.js"), "// test init\n").expect("write init.js");
        ConfigurationRuntime::from_config_root(&root).expect("create configuration runtime")
    }

    #[test]
    fn module_error_storage_is_relative_bounded_and_drained() {
        let runtime = runtime();
        runtime
            .record_module_error("./missing.js", &"x".repeat(2048))
            .expect("optional module path stays inside configuration root");

        let errors = runtime.take_module_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "./missing.js");
        assert_eq!(errors[0].message.len(), MODULE_ERROR_MESSAGE_BUDGET_BYTES);
        assert!(runtime.take_module_errors().is_empty());
    }

    #[test]
    fn package_option_configuration_accepts_supported_typed_options_only() {
        let runtime = runtime();
        let option = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown.layout.defaultVisibility",
                "value": "hidden",
                "source": "init-js"
            }))
            .unwrap();
        assert_eq!(option.package_prefix, "markdown");
        assert_eq!(option.source, "init-js");
        assert!(
            runtime
                .state_json()
                .contains("markdown.layout.defaultVisibility")
        );

        let token_remap = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown.themeTokenRemap",
                "value": { "token": "markdown.preview.background", "fallback": "surface.panel" }
            }))
            .unwrap();
        assert_eq!(token_remap.option, "markdown.themeTokenRemap");
    }

    #[test]
    fn package_option_configuration_rejects_hidden_ad_hoc_and_raw_authority_keys() {
        let runtime = runtime();
        let hidden = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown._hidden",
                "value": true
            }))
            .unwrap_err();
        assert!(hidden.to_string().contains("hidden"));

        let ad_hoc = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown.preview.position",
                "value": "right"
            }))
            .unwrap_err();
        assert!(ad_hoc.to_string().contains("unsupported package option"));

        let raw = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown.fallback",
                "value": { "rawOps": "Deno.core.ops.op_clay_runtime_ping" }
            }))
            .unwrap_err();
        assert!(raw.to_string().contains("raw ops"));
    }

    #[test]
    fn configuration_rejects_watcher_control_keys() {
        // Plan 080: the configuration-root watcher is fixed automatic server
        // behavior. Interval, debounce, and enable/disable stay compiled
        // constants; any `core.watch.*` style key a user tries from
        // `~/.config/clay/init.js` is rejected by the closed package-option
        // allowlist — never a hidden configuration key.
        let runtime = runtime();
        for option in [
            "core.watch.intervalMs",
            "core.watch.debounceMs",
            "core.watch.enabled",
        ] {
            let error = runtime
                .set_package_option(&json!({
                    "packagePrefix": "core",
                    "option": option,
                    "value": 1
                }))
                .unwrap_err();
            assert!(
                error.to_string().contains("unsupported package option"),
                "watcher key {option} must fail closed: {error}"
            );
        }
    }

    #[test]
    fn plan060_internal_security_and_performance_controls_are_not_configurable() {
        let runtime = runtime();
        for suffix in [
            "runtime.domain",
            "runtime.packageContext",
            "runtime.crossDomainPayloadBytes",
            "ipc.clientId",
            "ipc.connectionIdentity",
            "queue.capacity",
            "completion.resultLaneCapacity",
            "documents.maxPerClient",
            "documents.maxServer",
            "connections.maxActive",
            "save.atomicMode",
            "save.tempRetries",
            "listing.maxConcurrency",
            "listing.ignoreMaxPatterns",
            "git.rootConcurrency",
            "languageServer.sessionQueueCapacity",
            "sandbox.frameBytes",
            "dialog.maxInFlight",
            "clipboard.backend",
            "build.debugProfile",
            "build.targetDirectory",
        ] {
            let option = format!("audit.{suffix}");
            let error = runtime
                .set_package_option(&json!({
                    "packagePrefix": "audit",
                    "option": option,
                    "value": 1
                }))
                .unwrap_err();
            assert!(
                error.to_string().contains("unsupported package option"),
                "internal setting {suffix} must fail closed: {error}"
            );
        }
        assert_eq!(
            runtime.state_json(),
            r#"{"entryPoint":"./init.js","loadedModules":[],"packageOptions":[]}"#
        );
    }

    /// Phase 18.9 introduced hardcoded structural defaults (the core.text/
    /// core.code fallback modes, electric-character outdent, generic pair
    /// insertion and comment continuation) rather than runtime-configurable
    /// Clay JS configuration settings. This test pins that contract: any
    /// behavior-changing Phase 18.9 key a user might try to set from
    /// `~/.config/clay/init.js` is rejected by the closed package-option
    /// allowlist (Plan 037 Task 10 Test Case 1) rather than silently accepted
    /// as an undocumented setting, and built-in mode defaults therefore
    /// cannot be overridden through configuration (Security criterion).
    #[test]
    fn phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected() {
        let runtime = runtime();

        // The plan's illustrative example: a preferred core fallback mode.
        let preferred_fallback = runtime
            .set_package_option(&json!({
                "packagePrefix": "core",
                "option": "core.preferredFallbackMode",
                "value": "core.code"
            }))
            .unwrap_err();
        assert!(
            preferred_fallback
                .to_string()
                .contains("unsupported package option"),
            "core.preferredFallbackMode must be rejected as unsupported, got: {preferred_fallback}"
        );

        // Electric-character behavior is a hardcoded core.code default, not a knob.
        let electric = runtime
            .set_package_option(&json!({
                "packagePrefix": "core",
                "option": "core.electricCharacters",
                "value": false
            }))
            .unwrap_err();
        assert!(
            electric.to_string().contains("unsupported package option"),
            "core.electricCharacters must be rejected as unsupported, got: {electric}"
        );

        // Generic pair-insertion / comment-continuation toggles are likewise
        // hardcoded behavior-rule defaults, not configurable settings.
        let pair_insertion = runtime
            .set_package_option(&json!({
                "packagePrefix": "core",
                "option": "core.pairInsertion",
                "value": false
            }))
            .unwrap_err();
        assert!(
            pair_insertion
                .to_string()
                .contains("unsupported package option")
        );

        let comment_continuation = runtime
            .set_package_option(&json!({
                "packagePrefix": "core",
                "option": "core.commentContinuation",
                "value": false
            }))
            .unwrap_err();
        assert!(
            comment_continuation
                .to_string()
                .contains("unsupported package option")
        );

        // No Phase 18.9 behavior-changing key was recorded as configuration state.
        let state = runtime.state_json();
        assert!(!state.contains("preferredFallbackMode"));
        assert!(!state.contains("electricCharacters"));
        assert!(!state.contains("pairInsertion"));
        assert!(!state.contains("commentContinuation"));
    }

    fn preferences_runtime() -> ConfigurationRuntime {
        let root = std::env::temp_dir().join(format!(
            "clay-prefs-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp config root");
        fs::write(root.join("init.js"), "// test init\n").expect("write init.js");
        ConfigurationRuntime::from_config_root(&root).expect("create configuration runtime")
    }

    #[test]
    fn package_option_source_taxonomy_accepts_ui_session() {
        let runtime = runtime();
        let option = runtime
            .set_package_option(&json!({
                "packagePrefix": "markdown",
                "option": "markdown.layout.defaultVisibility",
                "value": "collapsed",
                "source": "ui-session"
            }))
            .unwrap();
        assert_eq!(option.source, "ui-session");
    }

    #[test]
    fn preferences_persist_and_round_trip_through_a_fresh_runtime() {
        let runtime = preferences_runtime();
        let root = runtime.preferences_path().parent().unwrap().to_path_buf();
        // Persist theme + appearance.
        runtime
            .persist_preference("theme", json!("@clay/theme-modus-vivendi"))
            .expect("persist theme");
        runtime
            .persist_preference("appearance", json!("dark"))
            .expect("persist appearance");
        // A fresh runtime (simulating reload) reads the file back.
        let reloaded = ConfigurationRuntime::from_config_root(&root).expect("reload runtime");
        let prefs = reloaded.load_preferences();
        assert_eq!(prefs.theme.as_deref(), Some("@clay/theme-modus-vivendi"));
        assert_eq!(prefs.appearance, Some(crate::protocol::Appearance::Dark));
        assert!(prefs.diagnostics.is_empty(), "no diagnostics: {prefs:?}");
    }

    #[test]
    fn preferences_clear_wipes_the_store() {
        let runtime = preferences_runtime();
        let root = runtime.preferences_path().parent().unwrap().to_path_buf();
        runtime
            .persist_preference("theme", json!("@clay/theme-modus-operandi"))
            .unwrap();
        runtime.clear_preferences().expect("clear");
        let reloaded = ConfigurationRuntime::from_config_root(&root).expect("reload runtime");
        let prefs = reloaded.load_preferences();
        assert!(prefs.theme.is_none());
        assert!(prefs.appearance.is_none());
    }

    #[test]
    fn preferences_persist_rejects_non_first_party_theme() {
        let runtime = preferences_runtime();
        let err = runtime
            .persist_preference("theme", json!("@vendor/evil"))
            .expect_err("non-first-party theme must be rejected at persist");
        assert!(err.to_string().contains("first-party"));
        // File must not have been written with the bad value.
        let prefs = runtime.load_preferences();
        assert!(prefs.theme.is_none());
    }

    #[test]
    fn preferences_persist_rejects_unknown_appearance() {
        let runtime = preferences_runtime();
        runtime
            .persist_preference("appearance", json!("auto"))
            .expect_err("unknown appearance must be rejected");
        let prefs = runtime.load_preferences();
        assert!(prefs.appearance.is_none());
    }

    #[test]
    fn preferences_persist_rejects_unknown_key() {
        let runtime = preferences_runtime();
        runtime
            .persist_preference("authority", json!("root"))
            .expect_err("unknown preferences key must be rejected");
    }

    #[test]
    fn preferences_load_drops_corrupted_field_with_diagnostic() {
        let runtime = preferences_runtime();
        fs::write(runtime.preferences_path(), r#"{"theme":42,"unknown":"x"}"#).unwrap();
        let prefs = runtime.load_preferences();
        assert!(prefs.theme.is_none(), "non-string theme dropped");
        assert!(
            !prefs.diagnostics.is_empty(),
            "diagnostics recorded: {prefs:?}"
        );
        assert!(
            prefs.diagnostics.iter().any(|d| d.contains("theme")),
            "theme diagnostic present"
        );
        assert!(
            prefs.diagnostics.iter().any(|d| d.contains("unknown")),
            "unknown-key diagnostic present"
        );
    }

    #[test]
    fn preferences_load_falls_back_when_file_is_not_json() {
        let runtime = preferences_runtime();
        fs::write(runtime.preferences_path(), "not json {{{").unwrap();
        let prefs = runtime.load_preferences();
        assert!(prefs.theme.is_none());
        assert!(prefs.appearance.is_none());
        assert!(!prefs.diagnostics.is_empty());
    }

    #[test]
    fn preferences_load_falls_back_when_payload_exceeds_budget() {
        let runtime = preferences_runtime();
        let huge = format!(
            "{{\"theme\":\"{}\"}}",
            "x".repeat(PREFERENCES_PAYLOAD_BUDGET_BYTES)
        );
        fs::write(runtime.preferences_path(), huge).unwrap();
        let prefs = runtime.load_preferences();
        assert!(prefs.theme.is_none());
        assert!(!prefs.diagnostics.is_empty());
    }

    #[test]
    fn preferences_typography_persists_and_round_trips() {
        let runtime = preferences_runtime();
        let root = runtime.preferences_path().parent().unwrap().to_path_buf();
        let typography = json!({
            "monospace": { "families": ["JetBrains Mono"], "size": 16 },
            "proportional": { "families": ["Inter"], "size": 17 },
            "ui": { "families": ["system-ui"], "size": 13 }
        });
        runtime
            .persist_preference("typography", typography.clone())
            .expect("persist typography");
        let reloaded = ConfigurationRuntime::from_config_root(&root).expect("reload runtime");
        let prefs = reloaded.load_preferences();
        let typography_back = prefs.typography.expect("typography round-tripped");
        assert_eq!(typography_back, typography);
        assert!(prefs.diagnostics.is_empty());
    }

    #[test]
    fn preferences_typography_rejects_authority_bearing_shape() {
        let runtime = preferences_runtime();
        let err = runtime
            .persist_preference(
                "typography",
                json!({ "monospace": { "families": ["x"], "size": 16 }, "rawOps": "Deno.core.ops" }),
            )
            .expect_err("authority-bearing typography must be rejected");
        assert!(err.to_string().contains("prohibited"));
    }
}
