#[cfg(unix)]
use std::path::PathBuf;
use std::{ffi::OsString, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcEndpoint {
    #[cfg(unix)]
    UnixSocket(PathBuf),
    #[cfg(windows)]
    WindowsNamedPipe(String),
    #[cfg(not(any(unix, windows)))]
    Unsupported(String),
}

#[cfg(unix)]
impl From<PathBuf> for IpcEndpoint {
    fn from(path: PathBuf) -> Self {
        Self::UnixSocket(path)
    }
}

#[cfg(unix)]
impl From<&std::path::Path> for IpcEndpoint {
    fn from(path: &std::path::Path) -> Self {
        Self::UnixSocket(path.to_path_buf())
    }
}

#[cfg(unix)]
impl From<&PathBuf> for IpcEndpoint {
    fn from(path: &PathBuf) -> Self {
        Self::UnixSocket(path.clone())
    }
}

impl IpcEndpoint {
    pub fn from_argument(argument: impl Into<OsString>) -> Self {
        let argument = argument.into();
        #[cfg(unix)]
        {
            Self::UnixSocket(PathBuf::from(argument))
        }
        #[cfg(windows)]
        {
            Self::WindowsNamedPipe(argument.to_string_lossy().into_owned())
        }
        #[cfg(not(any(unix, windows)))]
        {
            Self::Unsupported(argument.to_string_lossy().into_owned())
        }
    }

    pub fn as_child_arg(&self) -> OsString {
        match self {
            #[cfg(unix)]
            Self::UnixSocket(path) => path.as_os_str().to_owned(),
            #[cfg(windows)]
            Self::WindowsNamedPipe(name) => OsString::from(name),
            #[cfg(not(any(unix, windows)))]
            Self::Unsupported(name) => OsString::from(name),
        }
    }

    #[cfg(unix)]
    pub fn as_unix_socket_path(&self) -> &std::path::Path {
        match self {
            Self::UnixSocket(path) => path.as_path(),
        }
    }

    #[cfg(windows)]
    pub fn as_windows_named_pipe(&self) -> &str {
        match self {
            Self::WindowsNamedPipe(name) => name.as_str(),
        }
    }

    #[cfg(windows)]
    pub fn validate_windows_named_pipe(&self) -> Result<(), String> {
        let name = self.as_windows_named_pipe();
        if name.is_empty() {
            return Err("named pipe name must not be empty".to_string());
        }
        if !name.starts_with(r"\\.\pipe\") {
            return Err(format!(
                "named pipe name must use the local \\\\.\\pipe\\ prefix: {name}"
            ));
        }
        Ok(())
    }
}

impl fmt::Display for IpcEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(unix)]
            Self::UnixSocket(path) => write!(formatter, "{}", path.display()),
            #[cfg(windows)]
            Self::WindowsNamedPipe(name) => formatter.write_str(name),
            #[cfg(not(any(unix, windows)))]
            Self::Unsupported(name) => formatter.write_str(name),
        }
    }
}

pub fn default_endpoint() -> IpcEndpoint {
    #[cfg(unix)]
    {
        IpcEndpoint::UnixSocket(default_socket_path())
    }
    #[cfg(windows)]
    {
        IpcEndpoint::WindowsNamedPipe(default_named_pipe_name())
    }
    #[cfg(not(any(unix, windows)))]
    {
        IpcEndpoint::Unsupported("clay".to_string())
    }
}

#[cfg(unix)]
pub fn default_socket_path() -> PathBuf {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("clay.sock");
    }

    std::env::temp_dir().join(format!("clay-{}.sock", current_user_suffix()))
}

#[cfg(windows)]
pub fn default_named_pipe_name() -> String {
    format!(r"\\.\pipe\clay-{}", current_user_suffix())
}

fn current_user_suffix() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    sanitize_endpoint_suffix(&user)
}

fn sanitize_endpoint_suffix(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::{IpcEndpoint, default_endpoint};

    #[test]
    fn default_endpoint_is_platform_valid() {
        let endpoint = default_endpoint();

        #[cfg(unix)]
        assert!(matches!(endpoint, IpcEndpoint::UnixSocket(_)));

        #[cfg(windows)]
        match endpoint {
            IpcEndpoint::WindowsNamedPipe(name) => {
                assert!(name.starts_with(r"\\.\pipe\clay-"));
            }
        }
    }

    #[test]
    fn endpoint_display_does_not_panic() {
        let endpoint = default_endpoint();
        assert!(!endpoint.to_string().is_empty());
    }
}
