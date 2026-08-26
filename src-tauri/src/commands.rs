//! Typed Tauri commands exposed to the main webview.
//!
//! Phase 2 surface: process-supervision status/restart.
//! Phase 3 surface: the typed session bridge (bootstrap, subscription,
//! reconnect, validated request forwarding).

use std::{path::PathBuf, sync::Arc};

use tauri::{State, ipc::Channel};

use crate::bridge::{BootstrapDto, BridgeEnvelope, BridgeError, BridgeState};
use crate::server::{ServerStatus, Supervisor};

type Supervised<'a> = State<'a, Arc<Supervisor>>;
type Bridged<'a> = State<'a, Arc<BridgeState>>;

#[derive(Default)]
pub struct DialogState {
    file: tokio::sync::Mutex<()>,
    folder: tokio::sync::Mutex<()>,
}

type Dialogs<'a> = State<'a, DialogState>;

/// Adapter delivering bridge envelopes into the webview channel.
struct ChannelSink(Channel<BridgeEnvelope>);

impl crate::bridge::forwarder::EventSink for ChannelSink {
    fn deliver(&self, envelope: BridgeEnvelope) -> Result<(), String> {
        self.0.send(envelope).map_err(|error| error.to_string())
    }
}

/// Current typed connection status for the supervised Clay server.
#[tauri::command]
pub async fn server_status(supervisor: Supervised<'_>) -> Result<ServerStatus, String> {
    Ok(supervisor.status())
}

/// Restart the supervised Clay server (kill + relaunch) and report the new
/// attempt's status. Used by the frontend reconnect affordance.
#[tauri::command]
pub async fn server_restart(supervisor: Supervised<'_>) -> Result<ServerStatus, String> {
    supervisor.restart();
    Ok(supervisor.status())
}

/// Connects to the Clay server and installs one complete bootstrap snapshot.
/// Idempotent while connected: returns the cached snapshot.
#[tauri::command]
pub async fn session_bootstrap(
    bridge: Bridged<'_>,
) -> Result<crate::bridge::BootstrapDto, BridgeError> {
    bridge.bootstrap().await
}

/// Subscribes the calling webview to session events (server message families
/// plus lifecycle notices). Replaces any previous subscription.
#[tauri::command]
pub async fn session_subscribe(
    bridge: Bridged<'_>,
    on_event: Channel<BridgeEnvelope>,
) -> Result<(), BridgeError> {
    bridge.subscribe(ChannelSink(on_event));
    Ok(())
}

/// Drops the active subscription; no further events are delivered.
#[tauri::command]
pub async fn session_unsubscribe(bridge: Bridged<'_>) -> Result<(), BridgeError> {
    bridge.unsubscribe();
    Ok(())
}

/// Subscribes the calling webview to the AG-UI agent event relay (Phase 10).
/// Events are adapted from Clay's internal agent union in Rust; prompt,
/// cancel, and session operations reuse `session_request`.
#[tauri::command]
pub async fn agent_subscribe(
    bridge: Bridged<'_>,
    on_event: Channel<crate::bridge::AgentStreamEvent>,
) -> Result<(), BridgeError> {
    bridge.subscribe_agent(on_event);
    Ok(())
}

/// Drops every AG-UI agent relay registration for this webview.
#[tauri::command]
pub async fn agent_unsubscribe(bridge: Bridged<'_>) -> Result<(), BridgeError> {
    bridge.unsubscribe_agent();
    Ok(())
}

/// Tears down the current session, reconnects (reclaiming the tab when the
/// server still knows it), and returns a fresh complete bootstrap.
#[tauri::command]
pub async fn session_reconnect(
    bridge: Bridged<'_>,
) -> Result<crate::bridge::BootstrapDto, BridgeError> {
    bridge.reconnect().await
}

/// Diagnostics for the bridge session (connected/generation/coalesced).
#[tauri::command]
pub async fn session_stats(
    bridge: Bridged<'_>,
) -> Result<crate::bridge::session::BridgeStats, BridgeError> {
    Ok(bridge.stats())
}

/// Forwards one validated frontend request to the server. `payload` is JSON
/// matching the typed `ClientMessage` shape; size-capped and sanitized before
/// anything is parsed or sent.
#[tauri::command]
pub async fn session_request(
    bridge: Bridged<'_>,
    payload: String,
    tab_id: Option<u64>,
) -> Result<(), BridgeError> {
    bridge.request_on(tab_id, &payload)
}

#[tauri::command]
pub async fn tab_open(
    bridge: Bridged<'_>,
    workspace_root: String,
) -> Result<BootstrapDto, BridgeError> {
    bridge.open_tab(workspace_root).await
}

#[tauri::command]
pub async fn tab_close(bridge: Bridged<'_>, tab_id: u64) -> Result<(), BridgeError> {
    bridge.close_tab(tab_id).await
}

#[tauri::command]
pub async fn tab_activate(bridge: Bridged<'_>, tab_id: u64) -> Result<(), BridgeError> {
    bridge.activate_tab(tab_id).await
}

/// Native open dialog via the XDG file-chooser portal (ashpd). Runs on the
/// Tauri tokio runtime; ashpd owns the portal request/response handshake, so
/// a selection can never be lost to a signal race and the caller's per-dialog
/// lock is always released.
async fn pick_path(folder: bool) -> Result<Option<PathBuf>, BridgeError> {
    use ashpd::Error as PortalError;
    use ashpd::desktop::ResponseError;
    use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest};

    let request = if folder {
        OpenFileRequest::default()
            .title("Open Folder")
            .modal(false)
            .directory(true)
    } else {
        OpenFileRequest::default()
            .title("Open File")
            .modal(false)
            .filters(vec![
                FileFilter::new("Markdown files")
                    .glob("*.md")
                    .glob("*.markdown")
                    .glob("*.mdown"),
                FileFilter::new("All files").glob("*"),
            ])
    };
    match request.send().await {
        Ok(response) => match response.response() {
            Ok(files) => Ok(files.uris().first().and_then(|uri| {
                url::Url::parse(uri.as_str())
                    .ok()
                    .and_then(|parsed| parsed.to_file_path().ok())
            })),
            Err(PortalError::Response(ResponseError::Cancelled)) => Ok(None),
            Err(error) => Err(BridgeError::invalid_request(format!(
                "file dialog failed: {error}"
            ))),
        },
        Err(PortalError::Response(ResponseError::Cancelled)) => Ok(None),
        Err(error) => Err(BridgeError::invalid_request(format!(
            "file dialog failed: {error}"
        ))),
    }
}

#[tauri::command]
pub async fn dialog_open_file(
    bridge: Bridged<'_>,
    dialogs: Dialogs<'_>,
    tab_id: Option<u64>,
) -> Result<bool, BridgeError> {
    let _guard = dialogs.file.try_lock().map_err(|_| BridgeError::busy())?;
    let Some(path) = pick_path(false).await? else {
        return Ok(false);
    };
    bridge.accept_selected_path(tab_id, path, false)?;
    Ok(true)
}

#[tauri::command]
pub async fn dialog_open_folder(
    bridge: Bridged<'_>,
    dialogs: Dialogs<'_>,
    tab_id: Option<u64>,
) -> Result<bool, BridgeError> {
    let _guard = dialogs.folder.try_lock().map_err(|_| BridgeError::busy())?;
    let Some(path) = pick_path(true).await? else {
        return Ok(false);
    };
    bridge.accept_selected_path(tab_id, path, true)?;
    Ok(true)
}

#[tauri::command]
pub async fn tab_open_dialog(
    bridge: Bridged<'_>,
    dialogs: Dialogs<'_>,
) -> Result<Option<BootstrapDto>, BridgeError> {
    let _guard = dialogs.folder.try_lock().map_err(|_| BridgeError::busy())?;
    let Some(path) = pick_path(true).await? else {
        return Ok(None);
    };
    bridge
        .open_tab(path.to_string_lossy().into_owned())
        .await
        .map(Some)
}

#[tauri::command]
pub fn layout_load() -> Option<serde_json::Value> {
    clay::shell::load_window_state_json()
}

#[tauri::command]
pub fn layout_save(state: serde_json::Value) -> Result<(), BridgeError> {
    clay::shell::save_window_state_from_json(&state).map_err(BridgeError::invalid_request)
}
