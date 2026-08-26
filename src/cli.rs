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
    Package {
        subcommand: PackageCliSubcommand,
    },
}

/// Subcommand for `clay package <op> [args...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageCliSubcommand {
    /// Install a package by spec (delegates to the configured npm-compatible manager).
    Add {
        package_spec: String,
        /// Allow third-party lifecycle scripts to run during install.
        allow_scripts: bool,
    },
    /// Remove an installed package.
    Remove { package_name: String },
    /// List all installed packages and their enabled status.
    List,
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

pub(crate) const CLI_USAGE: &str = "Usage:\n  clay\n  clay server [endpoint] [--config-fixture <name>]\n  clay client [endpoint]\n  clay restart [endpoint]\n  clay smoke-gui [--config-fixture <name>]\n  clay perf-fixture --kind <kind> --size-mib <n> [--output <path>] [--seed <n>]\n  clay package add <spec> [--allow-scripts]\n  clay package remove <name>\n  clay package list\n  clay package enable <name>\n  clay package disable <name>\n  clay package inspect <name>\n  clay package adopt <name>\n  clay package revoke <name>\n  clay package rollback <name>\n  clay <endpoint>\n\nModes:\n  clay                  Stop leftover servers on the default endpoint, then launch the Tauri desktop.\n  clay server           Run a foreground server on the default local endpoint.\n  clay client           Open another Tauri desktop against a running server (does not kill servers).\n  clay restart          Stop leftover servers on the endpoint, start a fresh background server, then exit.\n  clay smoke-gui        Start an isolated server, launch the Tauri desktop, then clean up.\n  clay perf-fixture     Generate deterministic large UTF-8 plain-text performance fixtures.\n  clay package         Manage Clay packages (install/enable/disable/list/inspect/adopt/revoke/rollback).\n  clay <endpoint>       Advanced debugging shorthand for 'clay client <endpoint>'.\n\nOptions:\n  --config-fixture <name>  Development smoke fixture under tests/fixtures/configuration/<name>.\n  --allow-scripts          Allow package lifecycle scripts during `clay package add` (dangerous).\n  --profile-perf          Enable internal developer performance metric snapshots for this process.\n\nEnvironment:\n  CLAY_ALLOW_LIFECYCLE_SCRIPTS=1  Same as --allow-scripts (dangerous).\n\nPerf fixture kinds:\n  long-lines, many-short-lines, mixed-unicode, newline-heavy\n";

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

pub(crate) fn parse_package_subcommand(
    args: impl Iterator<Item = OsString>,
) -> Result<ClayCommand, CliError> {
    let mut args = args.peekable();
    let Some(op) = args.next() else {
        return Err(CliError::new(
            "clay package requires a subcommand: add | remove | list | enable | disable | inspect | adopt | revoke | rollback",
        ));
    };
    match op.to_string_lossy().as_ref() {
        "add" => {
            let mut spec = None;
            let mut allow_scripts = false;
            for arg in args {
                let text = arg.to_string_lossy();
                if text == "--allow-scripts" {
                    allow_scripts = true;
                } else if spec.is_none() {
                    spec = Some(arg);
                } else {
                    return Err(CliError::new(
                        "clay package add takes one package spec and optional --allow-scripts",
                    ));
                }
            }
            let spec = spec.ok_or_else(|| {
                CliError::new("clay package add requires a package spec, e.g. @clay/markdown")
            })?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Add {
                    package_spec: spec.to_string_lossy().into_owned(),
                    allow_scripts,
                },
            })
        }
        "remove" => {
            let name = args
                .next()
                .ok_or_else(|| CliError::new("clay package remove requires a package name"))?;
            Ok(ClayCommand::Package {
                subcommand: PackageCliSubcommand::Remove {
                    package_name: name.to_string_lossy().into_owned(),
                },
            })
        }
        "list" => Ok(ClayCommand::Package {
            subcommand: PackageCliSubcommand::List,
        }),
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
            "unknown clay package subcommand `{unknown}`; expected: add | remove | list | enable | disable | inspect | adopt | revoke | rollback"
        ))),
    }
}
