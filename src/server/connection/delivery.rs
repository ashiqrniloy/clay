//! Per-family delivery policy for the connection loop's lanes (plan 119 SC-2).
//!
//! The select loop used to inline every lane's `Ok` / `Lagged` / `Closed`
//! handling — roughly twenty lines per family, with the same shape everywhere
//! and exactly two policies. Each family now owns its decision here:
//! [`Delivery`] is the replay-vs-drop policy, [`Flow`] says whether the
//! connection keeps serving, and one helper per lane turns a receive outcome
//! into frames. Only the writes live here; the loop stays a router.
//!
//! The policy is unit-testable without a connection (see `mod tests`).

use std::{future::Future, sync::Arc};

use tokio::{io::AsyncWrite, sync::Mutex, sync::broadcast::error::RecvError};

use crate::{
    protocol::{
        ActiveTypography, AgentServerMessage, CaretStyle, ClientId, EditorCommandRequest,
        RuntimeDiagnostic, RuntimeGenerationId, ServerMessage, ShellPreferences,
        TabRegistrySnapshot, WrapPolicy,
        codec::{Codec, CodecError},
    },
    server::{
        RuntimeGenerationStore, document_analysis::DocumentAnalysisOutput,
        menu_sessions::ServerMenuSessions, tab_registry::TabRegistry,
    },
};

use super::menus;

/// How one broadcast lane treats a receiver that fell behind.
///
/// A lane carries either state or advice. State lanes must not skip: a client
/// that missed intermediate values is better served by the family's *current*
/// value than by a gap. Advisory lanes carry acts (run this command) that are
/// worthless once the moment has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delivery {
    /// State lane: on lag, deliver the family's current value instead.
    State,
    /// Advisory lane: on lag, drop what was missed.
    Advice,
}

/// One lane receive outcome with the lane's policy applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome<T> {
    /// Deliver the received value.
    Value(T),
    /// A `State` lane lagged: deliver the family's current value instead.
    Replay,
    /// An `Advice` lane lagged: deliver nothing.
    Drop,
    /// The sender is gone; the lane is finished.
    Closed,
}

impl Delivery {
    /// Classify one broadcast receive result. Pure: the caller keeps the value
    /// and owns the write.
    pub(super) fn on<T>(self, outcome: Result<T, RecvError>) -> Outcome<T> {
        match outcome {
            Ok(value) => Outcome::Value(value),
            Err(RecvError::Lagged(_)) => match self {
                Self::State => Outcome::Replay,
                Self::Advice => Outcome::Drop,
            },
            Err(RecvError::Closed) => Outcome::Closed,
        }
    }
}

/// Whether the connection loop keeps serving after a lane event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Flow {
    /// Keep serving.
    Continue,
    /// The lane's sender is gone; end the connection.
    Close,
}

/// Shared shape of the runtime state lanes: `map` frames a received value,
/// `replay` frames the family's current value and is awaited only on a lag.
async fn state_lane<S, T, M, R>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<T, RecvError>,
    map: M,
    replay: R,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
    M: FnOnce(T) -> ServerMessage,
    R: Future<Output = ServerMessage>,
{
    match Delivery::State.on(outcome) {
        Outcome::Value(value) => codec.write_server_message(stream, &map(value)).await?,
        Outcome::Replay => codec.write_server_message(stream, &replay.await).await?,
        // Impossible for a `State` lane: a policy change sends nothing rather
        // than panicking a live connection.
        Outcome::Drop => {}
        Outcome::Closed => return Ok(Flow::Close),
    }
    Ok(Flow::Continue)
}

/// Typography lane (`state`): lagged receivers get the committed typography.
pub(super) async fn typography<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<ActiveTypography, RecvError>,
    generation: &RuntimeGenerationStore,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    state_lane(
        codec,
        stream,
        outcome,
        ServerMessage::ActiveTypography,
        async { ServerMessage::ActiveTypography(generation.active_typography().await) },
    )
    .await
}

/// Programmatic editor-command lane (`advice`): a lagged request is an act
/// that has already passed its moment, so it drops.
pub(super) async fn editor_command<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<EditorCommandRequest, RecvError>,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    match Delivery::Advice.on(outcome) {
        Outcome::Value(request) => {
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::EditorCommandRequest(Box::new(request)),
                )
                .await?
        }
        Outcome::Drop => {}
        // Impossible for an `Advice` lane; see `state_lane`.
        Outcome::Replay => {}
        Outcome::Closed => return Ok(Flow::Close),
    }
    Ok(Flow::Continue)
}

/// Caret-override lane (`state`, trusted-domain only).
pub(super) async fn caret_style<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<Option<CaretStyle>, RecvError>,
    generation: &RuntimeGenerationStore,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    state_lane(
        codec,
        stream,
        outcome,
        ServerMessage::CaretStyleOverride,
        async { ServerMessage::CaretStyleOverride(generation.caret_style_override().await) },
    )
    .await
}

/// Editor wrap-policy lane (`state`, user-owned; packages cannot forge it).
pub(super) async fn editor_layout<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<Option<WrapPolicy>, RecvError>,
    generation: &RuntimeGenerationStore,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    state_lane(
        codec,
        stream,
        outcome,
        ServerMessage::EditorLayoutOverride,
        async { ServerMessage::EditorLayoutOverride(generation.editor_layout_override().await) },
    )
    .await
}

/// Shell-preferences lane (`state`).
pub(super) async fn shell_preferences<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<ShellPreferences, RecvError>,
    generation: &RuntimeGenerationStore,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    state_lane(
        codec,
        stream,
        outcome,
        ServerMessage::ShellPreferences,
        async { ServerMessage::ShellPreferences(generation.shell_preferences().await) },
    )
    .await
}

/// Tab-registry lane (`state`): the client applies a snapshot only when its
/// revision advances, so replaying the current one after a lag is idempotent.
pub(super) async fn tab_registry<S>(
    codec: Codec,
    stream: &mut S,
    outcome: Result<TabRegistrySnapshot, RecvError>,
    registry: &Arc<Mutex<TabRegistry>>,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    state_lane(codec, stream, outcome, ServerMessage::TabRegistry, async {
        ServerMessage::TabRegistry(registry.lock().await.snapshot())
    })
    .await
}

/// Runtime-generation lane. A command catalogue is generation-bound, so the
/// open menu session closes first on both a fresh event and a lag; the client
/// then gets the latest *complete* snapshot — the event carries only the
/// generation id, and a lagged receiver must not replay intermediate
/// generations. Activation re-checks the stamp if both paths race.
pub(super) async fn runtime_generation<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    generation: &RuntimeGenerationStore,
    client_id: ClientId,
    outcome: Result<RuntimeGenerationId, RecvError>,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    let state = Delivery::State.on(outcome);
    if state == Outcome::Closed {
        return Ok(Flow::Close);
    }
    if state != Outcome::Drop {
        menus::write_active_menu_session_closed(codec, stream, menu_sessions).await?;
        if let Some(snapshot) = generation.latest_runtime_snapshot_for(client_id).await {
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
                )
                .await?;
        }
    }
    Ok(Flow::Continue)
}

/// One bounded result lane (completion, language intelligence, diagnostics,
/// parse diagnostics). `None` is a closed lane: the connection keeps serving —
/// a client slower than the lane's capacity is bounded by that capacity and its
/// drop counter, never by tearing the connection down.
pub(super) async fn result<S, T>(
    codec: Codec,
    stream: &mut S,
    message: Option<T>,
    map: impl FnOnce(T) -> ServerMessage,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    if let Some(message) = message {
        codec.write_server_message(stream, &map(message)).await?;
    }
    Ok(Flow::Continue)
}

/// Document-analysis lane: decoration sets, diagnostic sets, and one-off
/// runtime diagnostics, projected from the analysis coordinator's output.
pub(super) async fn analysis<S>(
    codec: Codec,
    stream: &mut S,
    output: Option<DocumentAnalysisOutput>,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(output) = output else {
        return Ok(Flow::Continue);
    };
    let message = match output {
        DocumentAnalysisOutput::Decorations(set) => ServerMessage::DecorationSet(set),
        DocumentAnalysisOutput::Diagnostics(set) => ServerMessage::DiagnosticSet(set),
        DocumentAnalysisOutput::Diagnostic(diagnostic) => {
            ServerMessage::RuntimeDiagnostic(diagnostic)
        }
    };
    codec.write_server_message(stream, &message).await?;
    Ok(Flow::Continue)
}

/// Coding-agent lane. Two deliberate differences from the runtime lanes:
///
/// * The subscription outlives a daemon restart, so a lagged (`Lagged`) or
///   finished (`Closed`) broadcast never ends the view's connection — the
///   transcript recovers from the next snapshot.
/// * An event too large for one frame becomes a bounded diagnostic instead of
///   killing the webview connection with a codec error. The daemon already
///   clamps at source; this is the last-resort guard.
pub(super) async fn agent<S>(
    codec: Codec,
    stream: &mut S,
    event: Option<Result<Arc<AgentServerMessage>, RecvError>>,
) -> Result<Flow, CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(Ok(payload)) = event else {
        return Ok(Flow::Continue);
    };
    let message = ServerMessage::Agent(Box::new((*payload).clone()));
    if codec.encode_server_message(&message).is_err() {
        // The diagnostic write is best-effort: if the view is already gone,
        // nothing else can be reported, and the frame is small by construction.
        let _ = codec
            .write_server_message(
                stream,
                &ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::warning(
                    "agent.frame_too_large",
                    "Clay dropped an oversized agent event for this view; the run continued.",
                )),
            )
            .await;
        return Ok(Flow::Continue);
    }
    codec.write_server_message(stream, &message).await?;
    Ok(Flow::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lagged<T>() -> Result<T, RecvError> {
        Err(RecvError::Lagged(7))
    }

    fn closed<T>() -> Result<T, RecvError> {
        Err(RecvError::Closed)
    }

    fn registry_with_one_tab() -> (Arc<Mutex<TabRegistry>>, u64) {
        let mut registry = TabRegistry::new();
        let tab_id = registry.create_tab(4, 1, "/ws".to_string());
        (Arc::new(Mutex::new(registry)), tab_id)
    }

    /// The policy table: `State` lanes replay the current value on lag,
    /// `Advice` lanes drop, and a closed sender propagates either way.
    #[test]
    fn lag_policy_is_replay_for_state_and_drop_for_advice() {
        assert_eq!(Delivery::State.on(Ok(1u8)), Outcome::Value(1));
        assert_eq!(Delivery::Advice.on(Ok(1u8)), Outcome::Value(1));
        assert_eq!(Delivery::State.on(lagged::<u8>()), Outcome::Replay);
        assert_eq!(Delivery::Advice.on(lagged::<u8>()), Outcome::Drop);
        assert_eq!(Delivery::State.on(closed::<u8>()), Outcome::Closed);
        assert_eq!(Delivery::Advice.on(closed::<u8>()), Outcome::Closed);
    }

    /// A lagged state lane writes the family's current value — not a gap, not
    /// the missed value — and keeps the connection.
    #[tokio::test]
    async fn lagged_state_lane_writes_the_current_value() {
        let codec = Codec::default();
        let (registry, tab_id) = registry_with_one_tab();
        let mut out = Vec::new();

        let flow = tab_registry(codec, &mut out, lagged(), &registry)
            .await
            .unwrap();

        assert_eq!(flow, Flow::Continue);
        match codec.decode_server_message(&out).unwrap() {
            ServerMessage::TabRegistry(snapshot) => {
                assert_eq!(snapshot, registry.lock().await.snapshot());
                assert!(snapshot.tabs.iter().any(|tab| tab.tab_id == tab_id));
            }
            other => panic!("expected a tab registry snapshot, got {other:?}"),
        }
    }

    /// A lagged advisory lane writes nothing at all.
    #[tokio::test]
    async fn lagged_advice_lane_writes_nothing() {
        let codec = Codec::default();
        let mut out = Vec::new();

        let flow = editor_command(codec, &mut out, lagged()).await.unwrap();

        assert_eq!(flow, Flow::Continue);
        assert!(out.is_empty(), "advisory lanes must not replay: {out:?}");
    }

    /// A finished state lane ends the connection, writing nothing.
    #[tokio::test]
    async fn closed_state_lane_ends_the_connection() {
        let codec = Codec::default();
        let (registry, _) = registry_with_one_tab();
        let mut out = Vec::new();

        let flow = tab_registry(codec, &mut out, closed(), &registry)
            .await
            .unwrap();

        assert_eq!(flow, Flow::Close);
        assert!(out.is_empty());
    }

    /// The agent lane forwards a normal event, survives a daemon restart
    /// (closed broadcast), and turns an oversized event into a bounded
    /// diagnostic instead of a codec error that would kill the view.
    #[tokio::test]
    async fn agent_lane_bounds_oversized_events_and_survives_a_closed_daemon() {
        let payload = Arc::new(AgentServerMessage::Diagnostic {
            code: "agent.test".to_string(),
            message: "hello".to_string(),
        });
        let codec = Codec::default();
        let mut out = Vec::new();
        assert_eq!(
            agent(codec, &mut out, Some(Ok(Arc::clone(&payload))))
                .await
                .unwrap(),
            Flow::Continue
        );
        assert!(matches!(
            codec.decode_server_message(&out).unwrap(),
            ServerMessage::Agent(_)
        ));

        let mut out = Vec::new();
        assert_eq!(
            agent(codec, &mut out, Some(Err(RecvError::Closed)))
                .await
                .unwrap(),
            Flow::Continue
        );
        assert!(out.is_empty(), "a daemon restart must not end the view");

        // Well over the frame budget, but the budget still fits the bounded
        // diagnostic that replaces the dropped event.
        let small = Codec::new(512);
        let big = Arc::new(AgentServerMessage::AgentRpc {
            code: "session.search".to_string(),
            result_json: "x".repeat(8192),
        });
        let mut out = Vec::new();
        assert_eq!(
            agent(small, &mut out, Some(Ok(big))).await.unwrap(),
            Flow::Continue
        );
        match small.decode_server_message(&out).unwrap() {
            ServerMessage::RuntimeDiagnostic(diagnostic) => {
                assert_eq!(diagnostic.code, "agent.frame_too_large");
            }
            other => panic!("expected a bounded diagnostic, got {other:?}"),
        }
    }
}
