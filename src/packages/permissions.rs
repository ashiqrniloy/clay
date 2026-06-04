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
        prohibited if is_prohibited_authority(prohibited) => {
            Err(PermissionValidationError::ProhibitedAuthority {
                permission: prohibited.to_string(),
            })
        }
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
            | "ai-mutation"
            | "remote-listener"
            | "wasm-execution"
            | "raw-deno-ops"
            | "native-widget"
            | "client-javascript"
            | "package-installation"
            | "package-enable-disable"
            | "workspace-mutation"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionValidationError {
    UnknownPermission { permission: String },
    ProhibitedAuthority { permission: String },
}
