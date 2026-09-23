//! One-shot CLI verb implementations shared by `clay install`/`remove`/`list`.
//!
//! Printing lives here so launch.rs and tests share the same output contract.
//! No server spawn; every path routes through [`PackageService`].

use std::io::Write;
use std::path::Path;

use crate::packages::authorization::RuntimeProfile;
use crate::packages::init_lines::{
    AppendOutcome, InitLineError, RemoveOutcome, append_load_line, init_js_path, remove_load_line,
};
use crate::packages::manager::{PackageInstallOptions, PackageSpec};
use crate::packages::permissions::parse_permission;
use crate::packages::service::{AdoptionState, PackageInspection, PackageService};

pub fn install(
    service: &mut PackageService,
    spec: &str,
    options: PackageInstallOptions,
    config_root: &Path,
    backend_name: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    let parsed = PackageSpec::parse(spec).map_err(|error| error.to_string())?;
    writeln!(out, "Installing {spec} via {backend_name}…").map_err(io_err)?;
    service
        .install(spec, options)
        .map_err(|error| error.to_string())?;
    let inspection = service
        .inspect(&parsed.name)
        .ok_or_else(|| format!("package.json not found after installing `{spec}`"))?;
    writeln!(
        out,
        "Installed {}@{} from {spec}",
        inspection.name, inspection.version
    )
    .map_err(io_err)?;
    if options.allow_lifecycle_scripts {
        writeln!(
            out,
            "Warning: lifecycle scripts are ENABLED for {spec} — remote install scripts ran. Default installs suppress them."
        )
        .map_err(io_err)?;
    }
    writeln!(
        out,
        "Not enabled, not adopted — will not run until `clay package adopt {}`",
        inspection.name
    )
    .map_err(io_err)?;
    report_append(config_root, &inspection.name, out)?;
    Ok(())
}

pub fn remove(
    service: &mut PackageService,
    input: &str,
    config_root: &Path,
    out: &mut dyn Write,
) -> Result<(), String> {
    let name = resolve_package_name(input);
    if PackageService::inspect_bundled_inventory(&name).is_some() {
        return Err(format!(
            "cannot remove bundled package `{name}`; bundled inventory is not store-managed"
        ));
    }
    if service.inspect(&name).is_none() {
        return Err(format!("package `{name}` is not installed"));
    }
    writeln!(out, "Removing {name}…").map_err(io_err)?;
    service.remove(&name).map_err(|error| error.to_string())?;
    writeln!(out, "Removed {name}").map_err(io_err)?;
    report_remove(config_root, &name, out)?;
    Ok(())
}

pub fn list(service: &PackageService, bundled: bool, out: &mut dyn Write) -> Result<(), String> {
    let packages = service.list();
    if packages.is_empty() {
        writeln!(out, "No packages installed.").map_err(io_err)?;
    } else {
        for pkg in &packages {
            writeln!(out, "{}", format_list_line(service, pkg)).map_err(io_err)?;
        }
    }
    if bundled {
        let bundled_packages = PackageService::list_bundled_inventory();
        if !bundled_packages.is_empty() {
            writeln!(out, "Bundled:").map_err(io_err)?;
            for pkg in bundled_packages {
                writeln!(out, "  {}  {}  [bundled]", pkg.name, pkg.version).map_err(io_err)?;
            }
        }
    }
    Ok(())
}

fn resolve_package_name(input: &str) -> String {
    PackageSpec::parse(input)
        .map(|spec| spec.name)
        .unwrap_or_else(|_| input.to_string())
}

fn format_list_line(service: &PackageService, pkg: &PackageInspection) -> String {
    let record = service.install_record(&pkg.name);
    let spec = record
        .map(|record| record.spec.as_str())
        .unwrap_or(pkg.provenance.requested_spec.as_str());
    let pin = match record {
        Some(record) if record.pinned => "pinned",
        Some(_) => "floating",
        None => "unmanaged",
    };
    let source = record
        .map(|record| record.source.as_str())
        .unwrap_or("unmanaged");
    let enabled = if pkg.is_enabled {
        "enabled"
    } else {
        "installed"
    };
    let adoption = match service.adoption_state(&pkg.name) {
        Some(AdoptionState::Approved) => "adopted",
        Some(AdoptionState::Stale) => "stale",
        Some(AdoptionState::Revoked) => "revoked",
        None if record.is_none() => "unmanaged",
        Some(AdoptionState::Pending) | None => "pending",
    };
    format!(
        "  {}  {}  {spec}  {pin}  {source}  [{enabled}] [{adoption}]",
        pkg.name, pkg.version
    )
}

/// Grant capabilities to an adopted package through the shared service path
/// (Plan 136 task 5). The grant is durable: it lands on the package's current
/// approval record, so `clay package inspect`/`enable` in a later process read
/// the same authority. Adoption comes first — a CLI grant on an unadopted
/// package would be an in-memory record in a process that is about to exit,
/// so the verb refuses instead of pretending it persisted.
pub fn authorize(
    service: &mut PackageService,
    package_name: &str,
    capabilities: &[String],
    runtime_profile: Option<&str>,
    approved_by: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    if capabilities.is_empty() {
        return Err("clay package authorize requires at least one --capability".to_string());
    }
    let mut granted = Vec::with_capacity(capabilities.len());
    for capability in capabilities {
        let permission = parse_permission(capability).map_err(|error| match error {
            crate::packages::permissions::PermissionValidationError::UnknownPermission {
                permission,
            } => format!("unknown capability `{permission}`"),
            crate::packages::permissions::PermissionValidationError::ProhibitedAuthority {
                permission,
            } => format!("capability `{permission}` is reserved for Clay-owned authority"),
        })?;
        if !granted.contains(&permission) {
            granted.push(permission);
        }
    }
    let profile = match runtime_profile {
        Some(name) => RuntimeProfile::parse(name).ok_or_else(|| {
            format!(
                "unknown runtime profile `{name}`; expected: native-trust | sandboxed | restricted"
            )
        })?,
        None => RuntimeProfile::NativeTrust,
    };
    if service.inspect(package_name).is_none() {
        return Err(format!("package `{package_name}` is not installed"));
    }
    if service.adoption_state(package_name) != Some(AdoptionState::Approved) {
        return Err(format!(
            "package `{package_name}` is not adopted; run `clay package adopt {package_name}` first so the grant is durable"
        ));
    }
    service
        .authorize_package(package_name, granted, profile, approved_by)
        .map_err(|error| error.to_string())?;
    let inspection = service.inspect(package_name).expect("inspection exists");
    writeln!(
        out,
        "Authorized {}: {} ({})",
        inspection.name,
        inspection.approved_capabilities.join(", "),
        inspection.runtime_profile.as_deref().unwrap_or("unknown")
    )
    .map_err(io_err)?;
    writeln!(out, "  granted by:  {approved_by}").map_err(io_err)?;
    for line in format_grant_lines(&inspection) {
        writeln!(out, "  {line}").map_err(io_err)?;
    }
    Ok(())
}

/// Grant lines for `clay package inspect`: the granted capabilities and
/// runtime profile, who granted/approved them and when, and the capabilities
/// the manifest declares but the user has not granted yet.
pub fn format_grant_lines(inspection: &PackageInspection) -> Vec<String> {
    let mut lines = Vec::new();
    if !inspection.approved_capabilities.is_empty() {
        lines.push(format!(
            "Grants:      {} ({})",
            inspection.approved_capabilities.join(", "),
            inspection.runtime_profile.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(provenance) = &inspection.grant_provenance {
        lines.push(format!(
            "Granted by:  {} at {}",
            provenance.granted_by, provenance.granted_at
        ));
        lines.push(format!(
            "Approved by: {} at {}",
            provenance.approved_by, provenance.approved_at
        ));
    }
    let ungranted: Vec<&str> = inspection
        .requested_capabilities
        .iter()
        .filter(|capability| {
            !inspection
                .approved_capabilities
                .iter()
                .any(|granted| granted == *capability)
        })
        .map(String::as_str)
        .collect();
    if !ungranted.is_empty() {
        lines.push(format!(
            "Ungranted:   {} (declared, not granted)",
            ungranted.join(", ")
        ));
    }
    lines
}

fn report_append(config_root: &Path, name: &str, out: &mut dyn Write) -> Result<(), String> {
    let path = init_js_path(config_root);
    match append_load_line(&path, name) {
        Ok(AppendOutcome::Added) => writeln!(
            out,
            "Appended loadPackage(\"{name}\") to {}",
            path.display()
        )
        .map_err(io_err),
        Ok(AppendOutcome::AlreadyPresent) => {
            writeln!(out, "Load line already present in {}", path.display()).map_err(io_err)
        }
        Err(InitLineError::MissingInitJs { path }) => {
            writeln!(out, "Skipped load line: {} does not exist", path.display()).map_err(io_err)
        }
        Err(error) => writeln!(out, "Skipped load line: {error}").map_err(io_err),
    }
}

fn report_remove(config_root: &Path, name: &str, out: &mut dyn Write) -> Result<(), String> {
    let path = init_js_path(config_root);
    match remove_load_line(&path, name) {
        Ok(RemoveOutcome::Removed) => writeln!(
            out,
            "Removed loadPackage(\"{name}\") from {}",
            path.display()
        )
        .map_err(io_err),
        Ok(RemoveOutcome::Absent) => {
            writeln!(out, "No Clay load line for {name} in {}", path.display()).map_err(io_err)
        }
        Ok(RemoveOutcome::LeftUntouched) => writeln!(
            out,
            "Left user-written loadPackage(\"{name}\") in {}",
            path.display()
        )
        .map_err(io_err),
        Err(InitLineError::MissingInitJs { path }) => {
            writeln!(out, "Skipped load line: {} does not exist", path.display()).map_err(io_err)
        }
        Err(error) => writeln!(out, "Skipped load line: {error}").map_err(io_err),
    }
}

pub const PINNED_SKIP_HINT: &str = "pinned; reinstall with a new version to move it";

pub fn update_extensions(service: &mut PackageService, out: &mut dyn Write) -> Result<(), String> {
    let mut records: Vec<_> = service.install_records().into_iter().cloned().collect();
    records.sort_by(|left, right| left.name.cmp(&right.name));
    if records.is_empty() {
        writeln!(out, "No Clay-managed packages to update.").map_err(io_err)?;
        return Ok(());
    }
    for record in records {
        update_record(service, &record, out)?;
    }
    Ok(())
}

pub fn update_package(
    service: &mut PackageService,
    spec: &str,
    out: &mut dyn Write,
) -> Result<(), String> {
    let parsed = PackageSpec::parse(spec).map_err(|error| error.to_string())?;
    match service.install_record(&parsed.name).cloned() {
        Some(record) => update_record(service, &record, out),
        None => {
            writeln!(out, "Skipped {}: not a Clay-managed install", parsed.name).map_err(io_err)?;
            Ok(())
        }
    }
}

fn update_record(
    service: &mut PackageService,
    record: &crate::packages::ledger::InstallRecord,
    out: &mut dyn Write,
) -> Result<(), String> {
    if record.pinned {
        writeln!(
            out,
            "Skipped {} ({}); {PINNED_SKIP_HINT}",
            record.name, record.spec
        )
        .map_err(io_err)?;
        return Ok(());
    }
    writeln!(out, "Updating {} ({})…", record.name, record.spec).map_err(io_err)?;
    service
        .install(&record.spec, PackageInstallOptions::default())
        .map_err(|error| error.to_string())?;
    let inspection = service
        .inspect(&record.name)
        .ok_or_else(|| format!("package.json not found after updating `{}`", record.spec))?;
    writeln!(
        out,
        "Updated {}@{} from {}",
        inspection.name, inspection.version, record.spec
    )
    .map_err(io_err)?;
    writeln!(
        out,
        "Not enabled, not adopted — will not run until `clay package adopt {}`",
        inspection.name
    )
    .map_err(io_err)?;
    Ok(())
}

fn io_err(error: std::io::Error) -> String {
    error.to_string()
}
