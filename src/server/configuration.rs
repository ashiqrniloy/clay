use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;

/// Server-side configuration root and local module loading state.
#[derive(Debug)]
pub(crate) struct ConfigurationRuntime {
    config_root: PathBuf,
    entry_point: PathBuf,
    loaded_modules: Mutex<Vec<PathBuf>>,
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
        serde_json::json!({
            "entryPoint": display_relative_to(&self.config_root, &self.entry_point),
            "loadedModules": loaded_modules,
        })
        .to_string()
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
}

impl ConfigurationError {
    pub(crate) fn to_js_error(&self) -> JsErrorBox {
        JsErrorBox::generic(format!("clay.configuration.invalid_module: {self}"))
    }
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot read configuration root: {error}"),
            Self::ReadModule(error) => {
                write!(formatter, "cannot read configuration module: {error}")
            }
            Self::InvalidRoot(message) | Self::InvalidModule(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl Error for ConfigurationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root(error) | Self::ReadModule(error) => Some(error),
            Self::InvalidRoot(_) | Self::InvalidModule(_) => None,
        }
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
