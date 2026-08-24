//! CLI launch boundary for the standalone server, package manager, fixtures,
//! and Tauri desktop process. Native window creation lives only in `src-tauri`.

use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};

use clay::ipc::IpcEndpoint;
use clay::perf::fixtures::{FixtureKind, FixtureSpec, default_fixture_path, generate_fixture_file};
#[cfg(any(unix, windows))]
use clay::server::{IpcServer, ServerConfig};

use crate::cli::PackageCliSubcommand;

pub(crate) fn run_package_subcommand(
    subcommand: PackageCliSubcommand,
) -> Result<(), Box<dyn Error>> {
    use clay::packages::manager::PnpmBackend;
    use clay::packages::service::PackageService;

    // Default store: ~/.config/clay/packages. The durable approval store
    // under the same root fails closed on corruption/unsafe permissions.
    let store_root = clay::packages::service::default_store_root();
    let mut service = PackageService::open(store_root, Box::new(PnpmBackend::new()))?;

    // A fresh service starts with an empty installed map. Repopulate it from
    // the package-manager store so `list`/`enable`/`disable`/`inspect`/`remove`
    // reflect packages installed by previous `clay package add` invocations.
    // `add` skips this: it installs via the backend (which re-discovers
    // internally) and a missing pnpm binary should fail at `pnpm add`, not at
    // the pre-list step.
    if !matches!(&subcommand, PackageCliSubcommand::Add { .. })
        && let Err(error) = service.refresh_installed()
        && !matches!(&subcommand, PackageCliSubcommand::Inspect { .. })
    {
        return Err(error.into());
    }

    match subcommand {
        PackageCliSubcommand::Add {
            package_spec,
            allow_scripts,
        } => {
            let allow_scripts = allow_scripts
                || std::env::var_os("CLAY_ALLOW_LIFECYCLE_SCRIPTS")
                    .is_some_and(|value| value == "1" || value == "true");
            println!("Installing {package_spec}…");
            service.install(
                &package_spec,
                clay::packages::manager::PackageInstallOptions {
                    allow_lifecycle_scripts: allow_scripts,
                },
            )?;
            println!("Installed {package_spec}");
        }
        PackageCliSubcommand::Remove { package_name } => {
            println!("Removing {package_name}…");
            service.remove(&package_name)?;
            println!("Removed {package_name}");
        }
        PackageCliSubcommand::List => {
            let packages = service.list();
            if packages.is_empty() {
                println!("No packages installed.");
            } else {
                for pkg in &packages {
                    let status = if pkg.is_enabled {
                        "[enabled]"
                    } else {
                        "[installed]"
                    };
                    println!("  {} {} {} {status}", pkg.name, pkg.version, pkg.api_prefix);
                }
            }
        }
        PackageCliSubcommand::Enable { package_name } => {
            println!("Enabling {package_name}…");
            service.enable(&package_name)?;
            println!("Enabled {package_name}");
        }
        PackageCliSubcommand::Disable { package_name } => {
            println!("Disabling {package_name}…");
            service.disable(&package_name)?;
            println!("Disabled {package_name}");
        }
        PackageCliSubcommand::Inspect { package_name } => {
            let from_store = service.inspect(&package_name);
            let store_hit = from_store.is_some();
            match from_store.or_else(|| PackageService::inspect_bundled_inventory(&package_name)) {
                Some(inspection) => {
                    println!("Package:     {}", inspection.name);
                    println!("Version:     {}", inspection.version);
                    println!("API prefix:  {}", inspection.api_prefix);
                    println!(
                        "Status:      {}",
                        if inspection.is_enabled {
                            "enabled"
                        } else if store_hit {
                            "installed"
                        } else {
                            "bundled"
                        }
                    );
                    println!("Modes:       {:?}", inspection.modes);
                    if let Some(preset) = &inspection.preset {
                        println!("Preset:      {preset}");
                    }
                    println!("Permissions: {:?}", inspection.permissions);
                    println!("Commands:    {}", inspection.command_count);
                    println!("Config keys: {}", inspection.configuration_count);
                    if !inspection.native_syntax_languages.is_empty() {
                        println!(
                            "Syntax:      {} — owned by native descriptor (FIRST_PARTY_NATIVE_GRAMMARS)",
                            inspection.native_syntax_languages.join(", ")
                        );
                    }
                    if let Some(docs) = &inspection.docs_path {
                        println!("Docs:        {docs}");
                    }
                    let adoption = match service.adoption_state(&package_name) {
                        Some(clay::packages::service::AdoptionState::Pending) => {
                            "pending adoption (cannot execute)"
                        }
                        Some(clay::packages::service::AdoptionState::Approved) => "approved",
                        Some(clay::packages::service::AdoptionState::Stale) => {
                            "stale approval (re-adopt required)"
                        }
                        Some(clay::packages::service::AdoptionState::Revoked) => "approval revoked",
                        None => "unknown",
                    };
                    println!("Adoption:    {adoption}");
                }
                None => eprintln!("Package `{package_name}` is not installed."),
            }
        }
        PackageCliSubcommand::Adopt { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let approval = service.approve_package(&package_name, "cli")?;
            println!("Adopted {} {}", approval.package, approval.resolved_version);
            println!("  capabilities: {}", approval.capabilities.join(", "));
            if !approval.processes.is_empty() {
                println!("  processes:    {}", approval.processes.join(", "));
            }
            for relation in &approval.relations {
                println!(
                    "  relation:     {} {} {}@{}",
                    relation.operation,
                    relation.package,
                    relation.extension_point,
                    relation.version
                );
            }
            for replacement in &approval.replacements {
                println!("  replaces:     {}", replacement.target);
            }
        }
        PackageCliSubcommand::Revoke { package_name } => {
            if service.inspect(&package_name).is_none() {
                eprintln!("Package `{package_name}` is not installed.");
                return Ok(());
            }
            let revoked = service.revoke_package_approval(&package_name)?;
            if service.disable(&package_name).is_ok() {
                println!("Disabled {package_name}");
            }
            if revoked {
                println!("Revoked approval for {package_name}");
            } else {
                println!("No approval recorded for {package_name}");
            }
        }
        PackageCliSubcommand::Rollback { target_name } => {
            let replacement = service.rollback_replacement(&target_name)?;
            println!("Disabled replacement {replacement}; restored {target_name}");
        }
    }
    Ok(())
}

#[cfg(any(unix, windows))]
pub(crate) fn run_server(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    eprintln!("clay server listening on {endpoint}");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = configuration_root;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { IpcServer::try_new(config)?.run().await })?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn run_server(
    endpoint: IpcEndpoint,
    _configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    Err(format!("Clay server IPC is unsupported on this platform: {endpoint}").into())
}

pub(crate) fn run_perf_fixture(
    kind: FixtureKind,
    size_mib: usize,
    seed: u64,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let size_bytes = size_mib
        .checked_mul(1024 * 1024)
        .ok_or("--size-mib is too large")?;
    let spec = FixtureSpec {
        kind,
        size_bytes,
        seed,
    };
    let output = output.unwrap_or_else(|| default_fixture_path(kind, size_mib));
    let output = generate_fixture_file(&spec, &output)?;
    println!(
        "generated {size_mib} MiB {} fixture at {}",
        kind.as_str(),
        output.display()
    );
    Ok(())
}

pub(crate) fn resolve_desktop_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CLAY_DESKTOP_BIN") {
        return PathBuf::from(path);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(if cfg!(windows) {
            "clay-desktop.exe"
        } else {
            "clay-desktop"
        });
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from(if cfg!(windows) {
        "clay-desktop.exe"
    } else {
        "clay-desktop"
    })
}

pub(crate) fn desktop_command(endpoint: &IpcEndpoint) -> Command {
    let mut command = Command::new(resolve_desktop_binary());
    command.env("CLAY_ENDPOINT", endpoint.as_child_arg());
    command
}

pub(crate) fn run_desktop(endpoint: IpcEndpoint) -> Result<(), Box<dyn Error>> {
    let status = desktop_command(&endpoint).status().map_err(|error| {
        format!(
            "failed to launch clay-desktop: {error}; set CLAY_DESKTOP_BIN or install the matching desktop artifact"
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("clay-desktop exited with status {status}").into())
    }
}

pub(crate) fn run_smoke_gui(
    endpoint: IpcEndpoint,
    configuration_root: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?.into_os_string();
    let mut server = ManagedServer::spawn(executable, &endpoint, configuration_root.as_deref())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(wait_for_server(&endpoint, || server.try_wait()))?;
    let result = run_desktop(endpoint);
    server.shutdown();
    result
}

async fn wait_for_server(
    endpoint: &IpcEndpoint,
    mut child_status: impl FnMut() -> Result<Option<ExitStatus>, std::io::Error>,
) -> Result<(), Box<dyn Error>> {
    let mut last = None;
    for _ in 0..50 {
        if let Some(status) = child_status()? {
            return Err(format!("managed Clay server exited before readiness: {status}").into());
        }
        match clay::client::connect(endpoint).await {
            Ok(_) => return Ok(()),
            Err(error) => last = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(format!(
        "Clay server did not become ready: {}",
        last.expect("retry recorded error")
    )
    .into())
}

struct ManagedServer {
    child: Option<Child>,
    endpoint: IpcEndpoint,
}

impl ManagedServer {
    fn spawn(
        executable: OsString,
        endpoint: &IpcEndpoint,
        configuration_root: Option<&Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut command = Command::new(executable);
        command.arg("server").arg(endpoint.as_child_arg());
        if let Some(root) = configuration_root {
            command
                .arg("--config-fixture")
                .arg(root.file_name().ok_or("fixture root has no name")?);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        Ok(Self {
            child: Some(command.spawn()?),
            endpoint: endpoint.clone(),
        })
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, std::io::Error> {
        self.child.as_mut().map_or(Ok(None), Child::try_wait)
    }

    fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take()
            && matches!(child.try_wait(), Ok(None))
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        #[cfg(unix)]
        if let Err(error) = std::fs::remove_file(self.endpoint.as_unix_socket_path())
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "failed to remove managed Clay socket {}: {error}",
                self.endpoint
            );
        }
    }
}

impl Drop for ManagedServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_command_uses_tauri_binary_and_local_endpoint() {
        let endpoint = IpcEndpoint::from_argument("clay-cutover-test");
        let command = desktop_command(&endpoint);
        assert!(
            command
                .get_program()
                .to_string_lossy()
                .contains("clay-desktop")
        );
        assert!(command.get_envs().any(|(name, value)| {
            name == "CLAY_ENDPOINT" && value == Some(endpoint.as_child_arg().as_os_str())
        }));
    }
}
