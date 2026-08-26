//! Clay CLI composition root.
//!
//! Bare `clay` and `clay client` launch the Tauri desktop. `clay server`
//! remains a standalone foreground server; package and fixture commands stay
//! headless.

use std::error::Error;

use clay::perf::metrics::{PerfConfig, install_global_recorder};

mod cli;
mod launch;

use cli::{CLI_USAGE, ClayCommand, extract_profile_perf_flag, parse_command};
use launch::{
    run_desktop, run_launch, run_package_subcommand, run_perf_fixture, run_restart, run_server,
    run_smoke_gui,
};

fn main() -> Result<(), Box<dyn Error>> {
    let (args, profile_perf) = extract_profile_perf_flag(std::env::args_os().skip(1));
    install_global_recorder(PerfConfig::from_env().with_flag(profile_perf));

    match parse_command(args)? {
        ClayCommand::Server {
            endpoint,
            configuration_root,
        } => run_server(endpoint, configuration_root),
        ClayCommand::Auto { endpoint } => run_launch(endpoint),
        ClayCommand::Client { endpoint } => run_desktop(endpoint),
        ClayCommand::Restart { endpoint } => run_restart(endpoint),
        ClayCommand::SmokeGui {
            endpoint,
            configuration_root,
        } => run_smoke_gui(endpoint, configuration_root),
        ClayCommand::PerfFixture {
            kind,
            size_mib,
            seed,
            output,
        } => run_perf_fixture(kind, size_mib, seed, output),
        ClayCommand::Help => {
            println!("{CLI_USAGE}");
            Ok(())
        }
        ClayCommand::Package { subcommand } => run_package_subcommand(subcommand),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::cli::{ClayCommand, extract_profile_perf_flag, parse_command};

    #[test]
    fn bare_and_client_launch_the_tauri_desktop_route() {
        assert!(matches!(
            parse_command(vec![]).unwrap(),
            ClayCommand::Auto { .. }
        ));
        assert!(matches!(
            parse_command(vec!["client".into()]).unwrap(),
            ClayCommand::Client { .. }
        ));
    }

    #[test]
    fn standalone_server_and_smoke_routes_remain() {
        assert!(matches!(
            parse_command(vec!["server".into()]).unwrap(),
            ClayCommand::Server { .. }
        ));
        assert!(matches!(
            parse_command(vec!["smoke-gui".into()]).unwrap(),
            ClayCommand::SmokeGui { .. }
        ));
    }

    #[test]
    fn restart_is_a_command_not_an_endpoint() {
        assert!(matches!(
            parse_command(vec!["restart".into()]).unwrap(),
            ClayCommand::Restart { .. }
        ));
    }

    #[test]
    fn global_perf_flag_is_removed_before_dispatch() {
        let (args, enabled) = extract_profile_perf_flag(
            vec![OsString::from("client"), OsString::from("--profile-perf")].into_iter(),
        );
        assert!(enabled);
        assert!(matches!(
            parse_command(args).unwrap(),
            ClayCommand::Client { .. }
        ));
    }
}
