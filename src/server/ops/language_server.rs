use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use deno_core::{JsBuffer, OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::server::language_server::{LanguageServerError, LanguageServerSpawn};

use super::{ClayOpState, packages::ensure_package_installed_locked};

#[op2]
#[string]
pub(super) async fn op_clay_language_server_authorize(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.invalid_grant: input must be valid JSON ({error})"
        ))
    })?;
    let object = request.as_object().ok_or_else(|| {
        JsErrorBox::generic("clay.language_server.invalid_grant: options must be an object")
    })?;
    if object.len() != 3
        || !object.contains_key("package")
        || !object.contains_key("contribution")
        || !object.contains_key("workspaceRootIds")
    {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_grant: options require only package, contribution, and workspaceRootIds",
        ));
    }
    let package = required_string(object.get("package"), "package")?;
    let contribution = required_string(object.get("contribution"), "contribution")?;
    let workspace_root_ids = parse_root_ids(object.get("workspaceRootIds"))?;

    let clay_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    ensure_authorization_open(&clay_state)?;

    let (resolved_name, descriptor) = {
        let mut service = clay_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        let (resolved_name, _, _) = ensure_package_installed_locked(&mut service, package)?;
        let (_, installed) = service
            .installed_package_for_specifier(&resolved_name)
            .expect("ensured package must remain installed");
        let record = crate::packages::record::assemble_package_record(&installed.package_json)
            .map_err(|error| {
                JsErrorBox::generic(format!(
                    "clay.language_server.invalid_contribution: {:?}: {}",
                    error.rule, error.message
                ))
            })?;
        let descriptor = record
            .contributions
            .language_servers
            .into_iter()
            .find(|descriptor| descriptor.id == contribution)
            .ok_or_else(|| {
                JsErrorBox::generic(format!(
                    "clay.language_server.unknown_contribution: package `{resolved_name}` does not declare `{contribution}`"
                ))
            })?;
        (resolved_name, descriptor)
    };

    let known_roots = clay_state.workspace().lock().await.directory_roots();
    if workspace_root_ids.iter().any(|requested| {
        !known_roots
            .iter()
            .any(|known| known.workspace_root_id == *requested)
    }) {
        return Err(JsErrorBox::generic(
            "clay.language_server.unknown_workspace_root: every workspaceRootId must name a current directory root",
        ));
    }
    let canonical_executable =
        crate::packages::authorization::resolve_language_server_executable(&descriptor.executable)
            .ok_or_else(|| {
                JsErrorBox::generic(format!(
                    "clay.language_server.executable_not_found: `{}` was not found or is not a canonical file",
                    descriptor.executable
                ))
            })?;

    // Recheck after async workspace access so a concurrent load cannot seal
    // authority while this request is pending.
    ensure_authorization_open(&clay_state)?;
    let grant = clay_state
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .authorize_language_server(
            &resolved_name,
            contribution,
            canonical_executable,
            workspace_root_ids,
            "init.js",
        )
        .map_err(|error| {
            JsErrorBox::generic(format!(
                "clay.language_server.authorization_failed: {error}"
            ))
        })?
        .clone();

    serde_json::to_string(&json!({
        "package": grant.package_name,
        "version": grant.resolved_version,
        "sourceKind": grant.source_kind.as_str(),
        "contribution": grant.contribution_id,
        "executable": grant.canonical_executable,
        "workspaceRootIds": grant.workspace_root_ids.iter().map(u64::to_string).collect::<Vec<_>>(),
        "approvedBy": grant.approved_by,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.authorization_failed: failed to serialize grant ({error})"
        ))
    })
}

fn ensure_authorization_open(state: &ClayOpState) -> Result<(), JsErrorBox> {
    if state.language_server_authorization_is_open() {
        Ok(())
    } else {
        Err(JsErrorBox::generic(
            "clay.language_server.authorization_sealed: grants are accepted only from init.js before the first package load",
        ))
    }
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, JsErrorBox> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 256)
        .ok_or_else(|| {
            JsErrorBox::generic(format!(
                "clay.language_server.invalid_grant: {field} must be a non-empty bounded string"
            ))
        })
}

fn parse_root_ids(value: Option<&Value>) -> Result<Vec<u64>, JsErrorBox> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        JsErrorBox::generic(
            "clay.language_server.invalid_grant: workspaceRootIds must be a non-empty array",
        )
    })?;
    if values.is_empty() || values.len() > 32 {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_grant: workspaceRootIds must contain 1..=32 entries",
        ));
    }
    let mut roots = Vec::with_capacity(values.len());
    for value in values {
        let root = value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
            .filter(|root| *root > 0)
            .ok_or_else(|| {
                JsErrorBox::generic(
                    "clay.language_server.invalid_grant: workspaceRootIds entries must be positive integers or decimal strings",
                )
            })?;
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots.sort_unstable();
    Ok(roots)
}

#[op2]
#[string]
pub(super) async fn op_clay_language_server_start_session(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.invalid_session: input must be valid JSON ({error})"
        ))
    })?;
    let object = request.as_object().ok_or_else(|| {
        JsErrorBox::generic("clay.language_server.invalid_session: options must be an object")
    })?;
    if object.len() != 2
        || !object.contains_key("contribution")
        || !object.contains_key("workspaceRootId")
    {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_session: options require only contribution and workspaceRootId",
        ));
    }
    // The session-owning package is the host-stamped executing package, never
    // a caller-supplied name: package A cannot start sessions as package B.
    let package = {
        let state_ref = state.borrow();
        let clay = state_ref.borrow::<Arc<ClayOpState>>();
        clay.current_package_record()?.manifest.name.clone()
    };
    let contribution = required_string(object.get("contribution"), "contribution")?;
    let workspace_root_id = object
        .get("workspaceRootId")
        .and_then(Value::as_u64)
        .or_else(|| {
            object
                .get("workspaceRootId")
                .and_then(Value::as_str)
                .and_then(|text| text.parse().ok())
        })
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            JsErrorBox::generic(
                "clay.language_server.invalid_session: workspaceRootId must be a positive integer",
            )
        })?;

    let clay_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    let (spawn, service) = {
        let mut service = clay_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        let (resolved_name, _, _) = ensure_package_installed_locked(&mut service, &package)?;
        let grant = service
            .language_server_grant(&resolved_name, contribution)
            .ok_or_else(|| {
                JsErrorBox::generic(format!(
                    "clay.language_server.missing_grant: package `{resolved_name}` has no current grant for `{contribution}`"
                ))
            })?
            .clone();
        if !grant.workspace_root_ids.contains(&workspace_root_id) {
            return Err(JsErrorBox::generic(format!(
                "clay.language_server.root_not_authorized: workspaceRootId `{workspace_root_id}` is not part of the grant for `{contribution}`"
            )));
        }
        // Re-fetch the current descriptor and verify its fingerprint matches
        // the grant: a package update that changed the executable/argv makes
        // the grant stale and the session cannot start.
        let (_, installed) = service
            .installed_package_for_specifier(&resolved_name)
            .expect("ensured package must remain installed");
        let record = crate::packages::record::assemble_package_record(&installed.package_json)
            .map_err(|error| {
                JsErrorBox::generic(format!(
                    "clay.language_server.invalid_contribution: {:?}: {}",
                    error.rule, error.message
                ))
            })?;
        let descriptor = record
            .contributions
            .language_servers
            .into_iter()
            .find(|descriptor| descriptor.id == contribution)
            .ok_or_else(|| {
                JsErrorBox::generic(format!(
                    "clay.language_server.unknown_contribution: package `{resolved_name}` does not declare `{contribution}`"
                ))
            })?;
        let fingerprint =
            crate::packages::authorization::language_server_descriptor_fingerprint(&descriptor);
        if fingerprint != grant.descriptor_fingerprint {
            return Err(JsErrorBox::generic(
                "clay.language_server.stale_grant: package contribution changed after authorization",
            ));
        }
        let canonical_executable =
            crate::packages::authorization::resolve_language_server_executable(&descriptor.executable)
                .filter(|path| path == &grant.canonical_executable)
                .ok_or_else(|| {
                    JsErrorBox::generic(format!(
                        "clay.language_server.executable_not_found: `{}` is not the authorized canonical executable",
                        descriptor.executable
                    ))
                })?;
        let spawn = LanguageServerSpawn {
            package_name: resolved_name.clone(),
            contribution_id: contribution.to_string(),
            descriptor_fingerprint: fingerprint,
            canonical_executable: canonical_executable.clone(),
            args: descriptor.args.clone(),
            inherit_environment: descriptor.inherit_environment.clone(),
            cwd: PathBuf::new(),
        };
        (spawn, clay_state.language_server_process())
    };

    // Resolve the approved root's canonical directory after releasing the
    // package-service lock; only directory roots can host a process cwd.
    let cwd = {
        let workspace = clay_state.workspace();
        let workspace = workspace.lock().await;
        workspace
            .directory_roots()
            .into_iter()
            .find(|root| root.workspace_root_id == workspace_root_id)
            .map(|root| root.canonical_path)
            .ok_or_else(|| {
                JsErrorBox::generic(format!(
                    "clay.language_server.root_not_authorized: workspaceRootId `{workspace_root_id}` is not a current directory root"
                ))
            })?
    };
    let mut spawn = spawn;
    spawn.cwd = cwd;

    let session_id = service.start(spawn).await.map_err(map_session_error)?;
    serde_json::to_string(&json!({
        "sessionId": session_id.as_u64(),
        "package": package,
        "contribution": contribution,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.session_failed: failed to serialize result ({error})"
        ))
    })
}

#[op2]
#[string]
pub(super) async fn op_clay_language_server_send_message(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let (session_id, package, contribution, message) = parse_session_message(&request_json)?;
    send_session_bytes(
        state,
        session_id,
        package,
        contribution,
        message.into_bytes(),
    )
    .await?;
    Ok("{}".to_string())
}

#[op2]
pub(super) async fn op_clay_language_server_send_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
    #[buffer] bytes: JsBuffer,
) -> Result<(), JsErrorBox> {
    let (session_id, package, contribution) = parse_session_bytes(&request_json)?;
    if bytes.len() > crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES {
        return Err(JsErrorBox::generic(format!(
            "clay.language_server.invalid_bytes: payload exceeds {} bytes",
            crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES
        )));
    }
    send_session_bytes(state, session_id, package, contribution, bytes.to_vec()).await
}

/// Bind session traffic to the executing package: the host-stamped package
/// context must name the session owner, so sibling packages cannot drive or
/// stop sessions they do not own even if they learn the opaque session id.
fn require_executing_package_owner(
    clay_state: &Arc<ClayOpState>,
    package: &str,
) -> Result<(), JsErrorBox> {
    let record = clay_state.current_package_record()?;
    if record.manifest.name != package {
        return Err(JsErrorBox::generic(format!(
            "clay.language_server.session_owner_mismatch: executing package `{}` cannot drive a session owned by `{package}`",
            record.manifest.name
        )));
    }
    Ok(())
}

async fn send_session_bytes(
    state: Rc<RefCell<OpState>>,
    session_id: crate::server::language_server::LanguageServerSessionId,
    package: String,
    contribution: String,
    bytes: Vec<u8>,
) -> Result<(), JsErrorBox> {
    let clay_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    require_executing_package_owner(&clay_state, &package)?;
    let fingerprint = require_current_fingerprint(&clay_state, &package, &contribution)?;
    clay_state
        .language_server_process()
        .write(session_id, package, contribution, fingerprint, bytes)
        .await
        .map_err(map_session_error)
}

#[op2]
#[string]
pub(super) async fn op_clay_language_server_read_message(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let bytes = read_session_bytes(state, parse_session_read(&request_json)?).await?;
    let message = String::from_utf8(bytes).map_err(|_| {
        JsErrorBox::generic(
            "clay.language_server.invalid_utf8: stdout chunk is not valid UTF-8; use readBytes for framed protocols",
        )
    })?;
    serde_json::to_string(&json!({ "message": message })).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.read_failed: failed to serialize result ({error})"
        ))
    })
}

#[op2]
#[buffer]
pub(super) async fn op_clay_language_server_read_bytes(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<Vec<u8>, JsErrorBox> {
    read_session_bytes(state, parse_session_read(&request_json)?).await
}

type SessionReadRequest = (
    crate::server::language_server::LanguageServerSessionId,
    String,
    String,
    usize,
    u64,
);

async fn read_session_bytes(
    state: Rc<RefCell<OpState>>,
    (session_id, package, contribution, max_bytes, timeout_ms): SessionReadRequest,
) -> Result<Vec<u8>, JsErrorBox> {
    let clay_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    require_executing_package_owner(&clay_state, &package)?;
    let fingerprint = require_current_fingerprint(&clay_state, &package, &contribution)?;
    clay_state
        .language_server_process()
        .read(
            session_id,
            package,
            contribution,
            fingerprint,
            max_bytes,
            timeout_ms,
        )
        .await
        .map_err(map_session_error)
}

#[op2]
#[string]
pub(super) async fn op_clay_language_server_stop_session(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.invalid_stop: input must be valid JSON ({error})"
        ))
    })?;
    let object = request.as_object().ok_or_else(|| {
        JsErrorBox::generic("clay.language_server.invalid_stop: options must be an object")
    })?;
    if object.len() != 3 || !object.contains_key("sessionId") {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_stop: options require only sessionId, package, and contribution",
        ));
    }
    let session_id = require_session_id(object.get("sessionId"))?;
    let (session_id, package, contribution) = {
        let (_, package, contribution) = parse_session_identity(object)?;
        (session_id, package, contribution)
    };
    let clay_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    require_executing_package_owner(&clay_state, &package)?;
    let fingerprint = require_current_fingerprint(&clay_state, &package, &contribution)?;
    let service = clay_state.language_server_process();
    service
        .stop(session_id, package, contribution, fingerprint)
        .await
        .map_err(map_session_error)?;
    Ok("{}".to_string())
}

fn parse_session_message(
    json_text: &str,
) -> Result<
    (
        crate::server::language_server::LanguageServerSessionId,
        String,
        String,
        String,
    ),
    JsErrorBox,
> {
    let object = parse_session_request(json_text, "invalid_message")?;
    if object.len() != 4 || !object.contains_key("message") {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_message: options require only sessionId, package, contribution, and message",
        ));
    }
    let (session_id, package, contribution) = parse_session_identity(&object)?;
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            JsErrorBox::generic("clay.language_server.invalid_message: message must be a string")
        })?;
    if message.len() > crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES {
        return Err(JsErrorBox::generic(format!(
            "clay.language_server.invalid_message: message exceeds {} bytes",
            crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES
        )));
    }
    Ok((session_id, package, contribution, message.to_string()))
}

fn parse_session_bytes(
    json_text: &str,
) -> Result<
    (
        crate::server::language_server::LanguageServerSessionId,
        String,
        String,
    ),
    JsErrorBox,
> {
    let object = parse_session_request(json_text, "invalid_bytes")?;
    if object.len() != 3 {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_bytes: options require only sessionId, package, and contribution",
        ));
    }
    parse_session_identity(&object)
}

fn parse_session_read(json_text: &str) -> Result<SessionReadRequest, JsErrorBox> {
    let object = parse_session_request(json_text, "invalid_read")?;
    if object.len() != 5 || !object.contains_key("maxBytes") || !object.contains_key("timeoutMs") {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_read: options require only sessionId, package, contribution, maxBytes, and timeoutMs",
        ));
    }
    let (session_id, package, contribution) = parse_session_identity(&object)?;
    let max_bytes = object
        .get("maxBytes")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            JsErrorBox::generic(
                "clay.language_server.invalid_read: maxBytes must be a positive bounded integer",
            )
        })?;
    if max_bytes > crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES {
        return Err(JsErrorBox::generic(format!(
            "clay.language_server.invalid_read: maxBytes exceeds {} bytes",
            crate::perf::budgets::LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES
        )));
    }
    let timeout_ms = object
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            JsErrorBox::generic(
                "clay.language_server.invalid_read: timeoutMs must be a positive integer",
            )
        })?;
    Ok((session_id, package, contribution, max_bytes, timeout_ms))
}

fn parse_session_request(
    json_text: &str,
    error_code: &str,
) -> Result<serde_json::Map<String, Value>, JsErrorBox> {
    let request: Value = serde_json::from_str(json_text).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.language_server.{error_code}: input must be valid JSON ({error})"
        ))
    })?;
    match request {
        Value::Object(object) => Ok(object),
        _ => Err(JsErrorBox::generic(format!(
            "clay.language_server.{error_code}: options must be an object"
        ))),
    }
}

fn parse_session_identity(
    object: &serde_json::Map<String, Value>,
) -> Result<
    (
        crate::server::language_server::LanguageServerSessionId,
        String,
        String,
    ),
    JsErrorBox,
> {
    if !object.contains_key("sessionId")
        || !object.contains_key("package")
        || !object.contains_key("contribution")
    {
        return Err(JsErrorBox::generic(
            "clay.language_server.invalid_session: sessionId, package, and contribution are required",
        ));
    }
    Ok((
        require_session_id(object.get("sessionId"))?,
        required_string(object.get("package"), "package")?.to_string(),
        required_string(object.get("contribution"), "contribution")?.to_string(),
    ))
}

fn require_session_id(
    value: Option<&Value>,
) -> Result<crate::server::language_server::LanguageServerSessionId, JsErrorBox> {
    value
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .and_then(Value::as_str)
                .and_then(|text| text.parse().ok())
        })
        .filter(|value| *value > 0)
        .map(crate::server::language_server::LanguageServerSessionId::from_u64)
        .ok_or_else(|| {
            JsErrorBox::generic(
                "clay.language_server.invalid_session: sessionId must be a positive integer",
            )
        })
}

fn require_current_fingerprint(
    clay_state: &ClayOpState,
    package: &str,
    contribution: &str,
) -> Result<u64, JsErrorBox> {
    let service = clay_state
        .package_service()
        .lock()
        .expect("package service mutex poisoned");
    let grant = service
        .language_server_grant(package, contribution)
        .ok_or_else(|| {
            JsErrorBox::generic(format!(
                "clay.language_server.missing_grant: package `{package}` has no current grant for `{contribution}`"
            ))
        })?;
    Ok(grant.descriptor_fingerprint)
}

fn map_session_error(error: LanguageServerError) -> JsErrorBox {
    JsErrorBox::generic(format!("clay.language_server.session_failed: {error}"))
}
