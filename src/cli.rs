//! CLI parsing: `ClayCommand`/`PackageCliSubcommand` vocabulary, `CliError`, the
//! `CLI_USAGE` help text, and the `parse_*`/`extract_profile_perf_flag`/
//! `resolve_config_fixture` leaf parsers. Pure functions over `OsString` args
//! returning a typed `ClayCommand`; no `Driver`/Masonry/launch coupling.

use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
};

use clay::ipc::{IpcEndpoint, default_endpoint, smoke_endpoint};
use clay::perf::fixtures::FixtureKind;
use clay::perf::metrics::PERF_PROFILE_FLAG;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClayCommand {
    Auto {
        endpoint: IpcEndpoint,
    },
    Client {
        endpoint: IpcEndpoint,
    },
    Restart {
        endpoint: IpcEndpoint,
    },
    Server {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    SmokeGui {
        endpoint: IpcEndpoint,
        configuration_root: Option<PathBuf>,
    },
    PerfFixture {
        kind: FixtureKind,
        size_mib: usize,
        seed: u64,
        output: Option<PathBuf>,
    },
    Help,
    Install {
        target: InstallTarget,
    },
    Remove {
        package_name: String,
    },
    List {
        bundled: bool,
    },
    Update {
        mode: UpdateMode,
    },
    Package {
        subcommand: PackageCliSubcommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateMode {
    SelfOnly,
    Extensions,
    All,
    Package { spec: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallTarget {
    /// No argument: report binary presence/absence (never installs).
    BinariesCheck,
    /// Provision one inventory binary; requires explicit `--yes` approval.
    Binary {
        name: String,
        approved: bool,
        allow_scripts: bool,
    },
    /// Install a Clay package from the npm registry.
    Package { spec: String, allow_scripts: bool },
}

/// Trust-lifecycle subcommand for `clay package <op> [args...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageCliSubcommand {
    /// Enable a previously installed package (runs Clay-owned validation).
    Enable { package_name: String },
    /// Disable a currently enabled package.
    Disable { package_name: String },
    /// Inspect metadata for a specific package.
    Inspect { package_name: String },
    /// Approve an installed package for execution (writes a durable exact
    /// approval record after host-side fact assembly).
    Adopt { package_name: String },
    /// Revoke a package's durable approval and disable it if enabled.
    Revoke { package_name: String },
    /// Roll back an active replacement: disable the replacement and restore
    /// the named target package.
    Rollback { target_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "{}", self.message)?;
        formatter.write_str(CLI_USAGE)
    }
}

impl Error for CliError {}

pub(crate) const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint] [--config-fixture <name>]\n  clay client [endpoint]\n  clay restart [endpoint]\n  clay smoke-gui [--config-fixture <name>]\n  clay perf-fixture --kind <kind> --size-mib <n> [--output <path>] [--seed <n>]\n  clay install                        # binary presence check (never installs)\n  clay install npm:<spec> [--allow-scripts]\n  clay install --bin <name> --yes [--allow-scripts]\n  clay remove npm:<spec>\n  clay list [--bundled]\n  clay update [--extensions|--all|npm:<spec>]\n  clay package enable <name>\n  clay package disable <name>\n  clay package inspect <name>\n  clay package adopt <name>\n  clay package revoke <name>\n  clay package rollback <name>\n  clay <endpoint>\n\nModes:\n  clay                  Stop leftover servers on the default endpoint, then launch the Tauri desktop.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Open another Tauri desktop against a running server (does not kill servers).\n  clay restart          Stop leftover servers on the endpoint, start a fresh background server, then exit.\n  clay smoke-gui        Start an isolated server, launch the Tauri desktop, then clean up.\n  clay perf-fixture     Generate deterministic large UTF-8 plain-text performance fixtures.\n  clay install          No argument: binary check. npm:<spec>: install a package (no enable/adopt). --bin <name> --yes: permissioned binary install.\n  clay remove           Uninstall a Clay-installed package and strip its init.js load line.\n  clay list             List installed packages (add --bundled for first-party inventory).\n  clay update           Self-update from the install channel. --extensions updates floating packages; --all does both.\n  clay package          Trust-lifecycle verbs (enable/disable/inspect/adopt/revoke/rollback).\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n\nOptions:\n  --config-fixture <name>  Development smoke fixture under tests/fixtures/configuration/<name>.\n  --allow-scripts          Allow package lifecycle scripts during `clay install` (dangerous).\n  --bin <name>             Provision an inventory binary (requires --yes).\n  --yes                    Approve the exact provisioning command for this invocation.\n  --bundled                Include bundled first-party packages in `clay list`.\n  --profile-perf          Enable internal developer performance metric snapshots for this process.\n\nEnvironment:\n  CLAY_ALLOW_LIFECYCLE_SCRIPTS=1  Same as --allow-scripts (dangerous).\n\nPerf fixture kinds:\n  long-lines, many-short-lines, mixed-unicode, newline-heavy\n";

pub(crate) fn extract_profile_perf_flag(
    args: impl Iterator<Item = OsString>,
) -> (Vec<OsString>, bool) {
    let mut profile_perf = false;
    let mut retained = Vec::new();
    for argument in args {
        if argument == PERF_PROFILE_FLAG {
            profile_perf = true;
        } else {
            retained.push(argument);
        }
    }
    (retained, profile_perf)
}

pub(crate) fn parse_command(args: Vec<OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(ClayCommand::Auto {
            endpoint: default_endpoint(),
        });
    };

    match first.to_string_lossy().as_ref() {
        "help" | "--help" | "-h" => Ok(ClayCommand::Help),
        "server" | "--server" => parse_server_subcommand(args),
        "client" | "--client" => parse_endpoint_subcommand("client", args)
            .map(|endpoint| ClayCommand::Client { endpoint }),
        "restart" | "--restart" => parse_endpoint_subcommand("restart", args)
            .map(|endpoint| ClayCommand::Restart { endpoint }),
        "smoke-gui" | "smoke" | "--smoke-gui" => parse_smoke_gui_subcommand(args),
        "perf-fixture" => parse_perf_fixture_subcommand(args),
        "install" => parse_install_subcommand(args),
        "remove" => parse_remove_subcommand(args),
        "list" => parse_list_subcommand(args),
        "update" => parse_update_subcommand(args),
        "package" => parse_package_subcommand(args),
        _ => {
            if let Some(extra) = args.next() {
                return Err(CliError::new(format!(
                    "unexpected extra argument after endpoint shorthand: {}",
                    extra.to_string_lossy()
                )));
            }
            Ok(ClayCommand::Client {
                endpoint: IpcEndpoint::from_argument(first),
            })
        }
    }
}

pub(crate) fn parse_perf_fixture_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let mut kind = None;
    let mut size_mib = None;
    let mut seed = 0xC1A4_F14E;
    let mut output = None;
    let mut args = args.peekable();

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--kind" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --kind for 'perf-fixture'",
                    ));
                };
                let value = value.to_string_lossy();
                kind = Some(FixtureKind::parse(&value).ok_or_else(|| {
                    CliError::new(format!("unknown performance fixture kind '{value}'"))
                })?);
            }
            "--size-mib" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --size-mib for 'perf-fixture'",
                    ));
                };
                size_mib = Some(parse_positive_usize("--size-mib", &value)?);
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --seed for 'perf-fixture'",
                    ));
                };
                seed = parse_u64("--seed", &value)?;
            }
            "--output" => {
                let Some(value) = args.next() else {
                    return Err(CliError::new(
                        "missing value after --output for 'perf-fixture'",
                    ));
                };
                output = Some(PathBuf::from(value));
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for 'perf-fixture': {other}"
                )));
            }
        }
    }

    Ok(ClayCommand::PerfFixture {
        kind: kind.ok_or_else(|| CliError::new("missing --kind for 'perf-fixture'"))?,
        size_mib: size_mib.ok_or_else(|| CliError::new("missing --size-mib for 'perf-fixture'"))?,
        seed,
        output,
    })
}

pub(crate) fn parse_positive_usize(option: &str, value: &OsString) -> Result<usize, CliError> {
    let text = value.to_string_lossy();
    let parsed = text
        .parse::<usize>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))?;
    if parsed == 0 {
        return Err(CliError::new(format!("{option} must be greater than zero")));
    }
    Ok(parsed)
}

pub(crate) fn parse_u64(option: &str, value: &OsString) -> Result<u64, CliError> {
    let text = value.to_string_lossy();
    text.parse::<u64>()
        .map_err(|_| CliError::new(format!("invalid numeric value for {option}: {text}")))
}

pub(crate) fn parse_endpoint_subcommand(
    mode: &str,
    mut args: impl Iterator<Item = OsString>,
) -> Result<IpcEndpoint, CliError> {
    let endpoint = args
        .next()
        .map(IpcEndpoint::from_argument)
        .unwrap_or_else(default_endpoint);

    if let Some(extra) = args.next() {
        return Err(CliError::new(format!(
            "unexpected extra argument for '{mode}': {}",
            extra.to_string_lossy()
        )));
    }

    Ok(endpoint)
}

pub(crate) fn parse_server_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let (endpoint, configuration_root) = parse_endpoint_and_config_fixture("server", args, true)?;
    Ok(ClayCommand::Server {
        endpoint,
        configuration_root,
    })
}

pub(crate) fn parse_smoke_gui_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let (_endpoint, configuration_root) =
        parse_endpoint_and_config_fixture("smoke-gui", args, false)?;
    Ok(ClayCommand::SmokeGui {
        endpoint: smoke_endpoint("gui"),
        configuration_root,
    })
}

pub(crate) fn parse_endpoint_and_config_fixture(
    mode: &str,
    args: impl Iterator<Item = OsString>,
    allow_endpoint: bool,
) -> Result<(IpcEndpoint, Option<PathBuf>), CliError> {
    let mut endpoint = None;
    let mut configuration_root = None;

    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        if argument == "--config-fixture" {
            let Some(name) = args.next() else {
                return Err(CliError::new(format!(
                    "missing fixture name after --config-fixture for '{mode}'"
                )));
            };
            if configuration_root.is_some() {
                return Err(CliError::new(format!(
                    "duplicate --config-fixture option for '{mode}'"
                )));
            }
            configuration_root = Some(resolve_config_fixture(&name)?);
        } else if allow_endpoint && endpoint.is_none() {
            endpoint = Some(IpcEndpoint::from_argument(argument));
        } else {
            return Err(CliError::new(format!(
                "unexpected extra argument for '{mode}': {}",
                argument.to_string_lossy()
            )));
        }
    }

    Ok((
        endpoint.unwrap_or_else(default_endpoint),
        configuration_root,
    ))
}

pub(crate) fn resolve_config_fixture(name: &OsString) -> Result<PathBuf, CliError> {
    let name = name.to_string_lossy();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(CliError::new(format!(
            "invalid configuration fixture name '{name}'"
        )));
    }

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join(name.as_ref());
    if !fixture_root.join("init.js").is_file() {
        return Err(CliError::new(format!(
            "configuration fixture '{}' does not contain init.js",
            name
        )));
    }
    Ok(fixture_root)
}

fn parse_install_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut args = args.peekable();
    let mut spec = None;
    let mut bin_name = None;
    let mut allow_scripts = false;
    let mut approved = false;
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        if text == "--allow-scripts" {
            allow_scripts = true;
        } else if text == "--yes" {
            approved = true;
        } else if text == "--bin" {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("--bin requires a binary name, e.g. graft"))?;
            if bin_name.is_some() {
                return Err(CliError::new("--bin given more than once"));
            }
            bin_name = Some(name.to_string_lossy().into_owned());
        } else if text.starts_with("--") {
            return Err(CliError::new(format!(
                "unknown option `{text}` for clay install"
            )));
        } else if spec.is_none() && bin_name.is_none() {
            spec = Some(arg);
        } else if bin_name.is_some() {
            return Err(CliError::new(
                "clay install --bin cannot be combined with a package spec",
            ));
        } else {
            return Err(CliError::new(
                "clay install takes one package spec, one --bin <name>, or nothing for the binary check",
            ));
        }
    }
    if let Some(name) = bin_name {
        if spec.is_some() {
            return Err(CliError::new(
                "clay install --bin cannot be combined with a package spec",
            ));
        }
        if !clay::packages::binaries::BINARY_INVENTORY
            .iter()
            .any(|entry| entry.name == name)
        {
            return Err(CliError::new(format!(
                "unknown binary `{name}`; known binaries: {}",
                clay::packages::binaries::BINARY_INVENTORY
                    .iter()
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        return Ok(ClayCommand::Install {
            target: InstallTarget::Binary {
                name,
                approved,
                allow_scripts,
            },
        });
    }
    let Some(spec) = spec else {
        return Ok(ClayCommand::Install {
            target: InstallTarget::BinariesCheck,
        });
    };
    let spec = spec.to_string_lossy().into_owned();
    clay::packages::manager::PackageSpec::parse(&spec)
        .map_err(|error| CliError::new(error.to_string()))?;
    Ok(ClayCommand::Install {
        target: InstallTarget::Package {
            spec,
            allow_scripts,
        },
    })
}

fn parse_remove_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut name = None;
    for arg in args {
        let text = arg.to_string_lossy();
        if text.starts_with("--") {
            return Err(CliError::new(format!(
                "unknown option `{text}` for clay remove"
            )));
        } else if name.is_none() {
            name = Some(arg);
        } else {
            return Err(CliError::new("clay remove takes one package spec or name"));
        }
    }
    let name = name.ok_or_else(|| {
        CliError::new("clay remove requires a package spec or name, e.g. npm:@arnilo/st")
    })?;
    Ok(ClayCommand::Remove {
        package_name: name.to_string_lossy().into_owned(),
    })
}

fn parse_list_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut bundled = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if text == "--bundled" {
            bundled = true;
        } else {
            return Err(CliError::new("clay list accepts only optional --bundled"));
        }
    }
    Ok(ClayCommand::List { bundled })
}

fn parse_update_subcommand(args: impl Iterator<Item = OsString>) -> Result<ClayCommand, CliError> {
    let mut extensions = false;
    let mut all = false;
    let mut spec = None;
    for arg in args {
        let text = arg.to_string_lossy();
        if text == "--extensions" {
            extensions = true;
        } else if text == "--all" {
            all = true;
        } else if text.starts_with("--") {
            return Err(CliError::new(format!(
                "unknown option `{text}` for clay update"
            )));
        } else if spec.is_none() {
            let owned = text.into_owned();
            clay::packages::manager::PackageSpec::parse(&owned)
                .map_err(|error| CliError::new(error.to_string()))?;
            spec = Some(owned);
        } else {
            return Err(CliError::new("clay update takes at most one package spec"));
        }
    }
    if spec.is_some() && (extensions || all) {
        return Err(CliError::new(
            "clay update npm:<spec> cannot be combined with --extensions or --all",
        ));
    }
    if extensions && all {
        return Err(CliError::new(
            "clay update accepts only one of --extensions or --all",
        ));
    }
    let mode = if let Some(spec) = spec {
        UpdateMode::Package { spec }
    } else if all {
        UpdateMode::All
    } else if extensions {
        UpdateMode::Extensions
    } else {
        UpdateMode::SelfOnly
    };
    Ok(ClayCommand::Update { mode })
}

pub(crate) fn parse_package_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let mut args = args.peekable();
    let Some(op) = args.next() else {
        return Err(CliError::new(
            "clay package requires a subcommand: enable | disable | inspect | adopt | revoke | rollback",
        ));
    };
    match op.to_string_lossy().as_ref() {
        "add" => Err(CliError::new(
            "clay package add was replaced by `clay install npm:<spec>`",
        )),
        "remove" => Err(CliError::new(
            "clay package remove was replaced by `clay remove npm:<spec>`",
        )),
        "list" => Err(CliError::new(
            "clay package list was replaced by `clay list`",
        )),
        "enable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package enable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Enable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "disable" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package disable requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Disable {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "inspect" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package inspect requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Inspect {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "adopt" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package adopt requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Adopt {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "revoke" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package revoke requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Revoke {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "rollback" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package rollback requires a target name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Rollback {
                    target_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        unknown => Err(CliError::new(format!(
            "unknown clay package subcommand `{unknown}`; expected: enable | disable | inspect | adopt | revoke | rollback"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(|part| OsString::from(*part)).collect()
    }

    #[test]
    fn parse_install_remove_list_and_lifecycle() {
        assert_eq!(
            parse_command(args(&["install", "npm:@arnilo/st"])).unwrap(),
            ClayCommand::Install {
                target: InstallTarget::Package {
                    spec: "npm:@arnilo/st".into(),
                    allow_scripts: false,
                },
            }
        );
        assert_eq!(
            parse_command(args(&[
                "install",
                "npm:@arnilo/st@1.2.3",
                "--allow-scripts"
            ]))
            .unwrap(),
            ClayCommand::Install {
                target: InstallTarget::Package {
                    spec: "npm:@arnilo/st@1.2.3".into(),
                    allow_scripts: true,
                },
            }
        );
        assert_eq!(
            parse_command(args(&["install", "--allow-scripts", "npm:plain-mode"])).unwrap(),
            ClayCommand::Install {
                target: InstallTarget::Package {
                    spec: "npm:plain-mode".into(),
                    allow_scripts: true,
                },
            }
        );
        assert_eq!(
            parse_command(args(&["remove", "npm:@arnilo/st"])).unwrap(),
            ClayCommand::Remove {
                package_name: "npm:@arnilo/st".into(),
            }
        );
        assert_eq!(
            parse_command(args(&["remove", "@arnilo/st"])).unwrap(),
            ClayCommand::Remove {
                package_name: "@arnilo/st".into(),
            }
        );
        assert_eq!(
            parse_command(args(&["list"])).unwrap(),
            ClayCommand::List { bundled: false }
        );
        assert_eq!(
            parse_command(args(&["list", "--bundled"])).unwrap(),
            ClayCommand::List { bundled: true }
        );
        assert_eq!(
            parse_command(args(&["update"])).unwrap(),
            ClayCommand::Update {
                mode: UpdateMode::SelfOnly,
            }
        );
        assert_eq!(
            parse_command(args(&["update", "--extensions"])).unwrap(),
            ClayCommand::Update {
                mode: UpdateMode::Extensions,
            }
        );
        assert_eq!(
            parse_command(args(&["update", "--all"])).unwrap(),
            ClayCommand::Update {
                mode: UpdateMode::All,
            }
        );
        assert_eq!(
            parse_command(args(&["update", "npm:@arnilo/st"])).unwrap(),
            ClayCommand::Update {
                mode: UpdateMode::Package {
                    spec: "npm:@arnilo/st".into(),
                },
            }
        );
        assert!(matches!(
            parse_command(args(&["package", "adopt", "@arnilo/st"])).unwrap(),
            ClayCommand::Package {
                subcommand: PackageCliSubcommand::Adopt { .. }
            }
        ));
    }

    #[test]
    fn parse_install_binary_check_and_provisioning() {
        assert_eq!(
            parse_command(args(&["install"])).unwrap(),
            ClayCommand::Install {
                target: InstallTarget::BinariesCheck,
            }
        );
        assert_eq!(
            parse_command(args(&["install", "--bin", "graft"])).unwrap(),
            ClayCommand::Install {
                target: InstallTarget::Binary {
                    name: "graft".into(),
                    approved: false,
                    allow_scripts: false,
                },
            }
        );
        assert_eq!(
            parse_command(args(&[
                "install",
                "--bin",
                "graft",
                "--yes",
                "--allow-scripts"
            ]))
            .unwrap(),
            ClayCommand::Install {
                target: InstallTarget::Binary {
                    name: "graft".into(),
                    approved: true,
                    allow_scripts: true,
                },
            }
        );
        let unknown = parse_command(args(&["install", "--bin", "nope"]))
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("unknown binary"));
        let mixed = parse_command(args(&["install", "--bin", "graft", "npm:@arnilo/st"]))
            .unwrap_err()
            .to_string();
        assert!(mixed.contains("cannot be combined"));
    }

    #[test]
    fn parse_rejects_replaced_package_verbs_and_non_npm_install() {
        let add = parse_command(args(&["package", "add", "npm:@arnilo/st"]))
            .unwrap_err()
            .to_string();
        assert!(add.contains("clay install"));
        let remove = parse_command(args(&["package", "remove", "@arnilo/st"]))
            .unwrap_err()
            .to_string();
        assert!(remove.contains("clay remove"));
        let list = parse_command(args(&["package", "list"]))
            .unwrap_err()
            .to_string();
        assert!(list.contains("clay list"));
        let github = parse_command(args(&["install", "github:user/mode"]))
            .unwrap_err()
            .to_string();
        assert!(github.contains("npm:"));
        let mixed = parse_command(args(&["update", "--extensions", "--all"]))
            .unwrap_err()
            .to_string();
        assert!(mixed.contains("--extensions") && mixed.contains("--all"));
        let with_spec = parse_command(args(&["update", "--all", "npm:@arnilo/st"]))
            .unwrap_err()
            .to_string();
        assert!(with_spec.contains("cannot be combined"));
        let github_update = parse_command(args(&["update", "github:user/mode"]))
            .unwrap_err()
            .to_string();
        assert!(github_update.contains("npm:"));
    }
}
