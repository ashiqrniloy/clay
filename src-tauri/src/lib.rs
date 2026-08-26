//! Clay Tauri v2 desktop shell.
//!
//! The shell owns windowing and the server *process* lifecycle only. All
//! document, package, configuration, and agent authority stays in the Clay
//! server (`clay` crate); the webview receives validated, typed projections
//! introduced in later bridge phases.
//!
//! Security posture (Phase 2):
//! - Strict CSP with `default-src 'none'`; no remote origins.
//! - The `main` capability grants only `core:default` permissions. No
//!   filesystem, shell, process, or HTTP plugin is compiled in or permitted.
//! - Package code never runs with Tauri APIs; packages contribute validated
//!   declarative UI through the Clay server.

pub mod bridge;
mod commands;
pub mod release;
pub mod server;

use std::sync::Arc;

use bridge::BridgeState;
use release::desktop_endpoint;
use server::Supervisor;
use tauri::{Manager, RunEvent};

pub fn run() {
    let endpoint = match desktop_endpoint() {
        Ok(endpoint) => endpoint,
        Err(reason) => {
            // Fail closed: still open the window so the status line can show
            // the typed rejection instead of refusing to launch.
            let fallback = clay::ipc::default_endpoint();
            let supervisor = Arc::new(Supervisor::new(fallback.clone()));
            supervisor.mark_disconnected(reason);
            return run_with(supervisor, fallback);
        }
    };
    let supervisor = Arc::new(Supervisor::new(endpoint.clone()));
    supervisor.start();
    run_with(supervisor, endpoint);
}

fn run_with(supervisor: Arc<Supervisor>, endpoint: clay::ipc::IpcEndpoint) {
    tauri::Builder::default()
        .manage(supervisor)
        .manage(Arc::new(BridgeState::new(endpoint)))
        .manage(commands::DialogState::default())
        .invoke_handler(tauri::generate_handler![
            commands::server_status,
            commands::server_restart,
            commands::session_bootstrap,
            commands::session_subscribe,
            commands::session_unsubscribe,
            commands::agent_subscribe,
            commands::agent_unsubscribe,
            commands::session_reconnect,
            commands::session_request,
            commands::session_stats,
            commands::tab_open,
            commands::tab_close,
            commands::tab_activate,
            commands::dialog_open_file,
            commands::dialog_open_folder,
            commands::tab_open_dialog,
            commands::layout_load,
            commands::layout_save,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                // Clean shutdown: kill + reap the supervised server so the
                // desktop app can never orphan it.
                app_handle.state::<Arc<Supervisor>>().shutdown();
            }
        });
}
