use std::{
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
const PACKAGE_OPTION_SOURCES: &[&str] = &["init-js", "package-default", "clay-default"];
const PANEL_VISIBILITY_VALUES: &[&str] = &["visible", "hidden", "collapsed"];
const PANEL_SLOT_VALUES: &[&str] = &["left", "right", "top", "bottom"];
const FALLBACK_VALUES: &[&str] = &["package-default", "hide", "ignore"];

/// Server-side configuration root and local module loading state.
#[derive(Debug)]
pub(crate) struct ConfigurationRuntime {
    config_root: PathBuf,
    entry_point: PathBuf,
    loaded_modules: Mutex<Vec<PathBuf>>,
    package_options: Mutex<Vec<RegisteredPackageOption>>,
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

    pub(crate) fn validate_module_path(&self, path: &str) -> Result<(), ConfigurationError> {
        self.resolve_from_entry(path).map(|_| ())
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
                "source must be init-js, package-default, or clay-default".to_string(),
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
                JsErrorBox::generic(format!("clay.configuration.invalid_package_option: {self}"))
            }
            _ => JsErrorBox::generic(format!("clay.configuration.invalid_module: {self}")),
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
            if !fallback.starts_with("clay.") {
                return Err(ConfigurationError::InvalidPackageOption(
                    "themeTokenRemap.fallback must reference a Clay core theme token; same-type checks happen when a registered theme token is remapped through clay:ui".to_string(),
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

fn display_relative_to(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    format!("./{}", relative.to_string_lossy().replace('\\', "/"))
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
                "value": { "token": "markdown.preview.background", "fallback": "clay.surface.panel" }
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
}
