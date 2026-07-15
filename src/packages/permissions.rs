#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackagePermission {
    ModeRegistration,
    ModeActivation,
    CommandRegistration,
    PackageConfiguration,
    ParseDocument,
    RenderDecorations,
    RenderFolding,
    CompletionProvider,
    LanguageServer,
    PackageControl,
    PackageImport,
    Filesystem,
    Network,
    Shell,
    Wasm,
    AiTools,
    WorkspaceMutation,
    NativeUi,
    ClientRuntime,
    RawOps,
}

impl PackagePermission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModeRegistration => "mode-registration",
            Self::ModeActivation => "mode-activation",
            Self::CommandRegistration => "command-registration",
            Self::PackageConfiguration => "package-configuration",
            Self::ParseDocument => "parse-document",
            Self::RenderDecorations => "render-decorations",
            Self::RenderFolding => "render-folding",
            Self::CompletionProvider => "completion-provider",
            Self::LanguageServer => "language-server",
            Self::PackageControl => "package-control",
            Self::PackageImport => "package-import",
            Self::Filesystem => "filesystem",
            Self::Network => "network",
            Self::Shell => "shell",
            Self::Wasm => "wasm",
            Self::AiTools => "ai-tools",
            Self::WorkspaceMutation => "workspace-mutation",
            Self::NativeUi => "native-ui",
            Self::ClientRuntime => "client-runtime",
            Self::RawOps => "raw-ops",
        }
    }
}

pub fn parse_permission(value: &str) -> Result<PackagePermission, PermissionValidationError> {
    match value {
        "mode-registration" => Ok(PackagePermission::ModeRegistration),
        "mode-activation" => Ok(PackagePermission::ModeActivation),
        "command-registration" => Ok(PackagePermission::CommandRegistration),
        "package-configuration" => Ok(PackagePermission::PackageConfiguration),
        "parse-document" => Ok(PackagePermission::ParseDocument),
        "render-decorations" => Ok(PackagePermission::RenderDecorations),
        "render-folding" => Ok(PackagePermission::RenderFolding),
        "completion-provider" => Ok(PackagePermission::CompletionProvider),
        "language-server" => Ok(PackagePermission::LanguageServer),
        "package-control" => Ok(PackagePermission::PackageControl),
        "package-import" => Ok(PackagePermission::PackageImport),
        "filesystem" => Ok(PackagePermission::Filesystem),
        "network" => Ok(PackagePermission::Network),
        "shell" => Ok(PackagePermission::Shell),
        "wasm" => Ok(PackagePermission::Wasm),
        "ai-tools" => Ok(PackagePermission::AiTools),
        "workspace-mutation" => Ok(PackagePermission::WorkspaceMutation),
        "native-ui" => Ok(PackagePermission::NativeUi),
        "client-runtime" => Ok(PackagePermission::ClientRuntime),
        "raw-ops" => Ok(PackagePermission::RawOps),
        unknown => Err(PermissionValidationError::UnknownPermission {
            permission: unknown.to_string(),
        }),
    }
}

pub fn is_prohibited_authority(value: &str) -> bool {
    matches!(
        value,
        "filesystem"
            | "network"
            | "shell"
            | "wasm"
            | "ai-tools"
            | "workspace-mutation"
            | "native-ui"
            | "client-runtime"
            | "raw-ops"
            | "language-server"
            | "package-control"
            | "package-import"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionValidationError {
    UnknownPermission { permission: String },
    ProhibitedAuthority { permission: String },
}
