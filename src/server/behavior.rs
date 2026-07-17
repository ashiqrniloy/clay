use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use crate::{
    behavior::manifest::{ManifestValidationError, validate_manifest},
    perf::budgets::{PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS, PREVIOUS_BEHAVIOR_GRACE_MS},
    protocol::{
        BehaviorManifest, BehaviorVersion, ClientId, DocumentId, EditRejection,
        RuntimeGenerationId, ServerMessage, TransactionId,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveBehaviorManifest {
    manifest: BehaviorManifest,
}

impl Default for ActiveBehaviorManifest {
    fn default() -> Self {
        Self::new(BehaviorManifest::minimal_text_editing(1))
            .expect("default text editing manifest must validate")
    }
}

impl ActiveBehaviorManifest {
    pub(crate) fn new(manifest: BehaviorManifest) -> Result<Self, ManifestValidationError> {
        validate_manifest(&manifest)?;
        Ok(Self { manifest })
    }

    pub(crate) fn manifest(&self) -> &BehaviorManifest {
        &self.manifest
    }

    pub(crate) fn version(&self) -> BehaviorVersion {
        self.manifest.behavior_version
    }

    pub(crate) fn manifest_message(&self) -> ServerMessage {
        ServerMessage::BehaviorManifest(self.manifest.clone())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(
        clippy::result_large_err,
        reason = "callers send protocol ServerMessage directly on rejection; boxing would complicate a cold error path"
    )]
    pub(crate) fn validate_message_version(
        &self,
        document_id: DocumentId,
        transaction_id: TransactionId,
        behavior_version: BehaviorVersion,
    ) -> Result<(), ServerMessage> {
        if behavior_version == self.version() {
            return Ok(());
        }

        Err(invalid_behavior_version_message(
            document_id,
            transaction_id,
            behavior_version,
            self.version(),
        ))
    }

    pub(crate) fn stage_replacement(
        &self,
        mut replacement: BehaviorManifest,
    ) -> Result<BehaviorManifest, ManifestValidationError> {
        replacement.behavior_version = self.version().saturating_add(1);
        validate_manifest(&replacement)?;
        Ok(replacement)
    }

    pub(crate) fn install_staged(&mut self, replacement: BehaviorManifest) {
        debug_assert_eq!(
            replacement.behavior_version,
            self.version().saturating_add(1)
        );
        self.manifest = replacement;
    }

    pub(crate) fn publish_replacement(
        &mut self,
        replacement: BehaviorManifest,
    ) -> Result<BehaviorManifest, ManifestValidationError> {
        let replacement = self.stage_replacement(replacement)?;
        self.install_staged(replacement);
        Ok(self.manifest.clone())
    }
}

/// Outcome of a grace-aware behavior-version check for Edit/EditorIntent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BehaviorVersionDecision {
    Current,
    PreviousWithinGrace,
}

#[derive(Debug)]
struct PreviousBehaviorGrace {
    previous_manifest: BehaviorManifest,
    previous_runtime_generation_id: RuntimeGenerationId,
    committed_at: Instant,
    accepted_previous_transactions: u64,
}

impl PreviousBehaviorGrace {
    fn new(
        previous_manifest: BehaviorManifest,
        previous_runtime_generation_id: RuntimeGenerationId,
        committed_at: Instant,
    ) -> Self {
        Self {
            previous_manifest,
            previous_runtime_generation_id,
            committed_at,
            accepted_previous_transactions: 0,
        }
    }

    fn previous_version(&self) -> BehaviorVersion {
        self.previous_manifest.behavior_version
    }

    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.committed_at)
            >= Duration::from_millis(PREVIOUS_BEHAVIOR_GRACE_MS)
            || self.accepted_previous_transactions >= PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS
    }
}

/// Shared previous-generation inert-manifest retention for bounded stale edits.
#[derive(Debug, Clone, Default)]
pub(crate) struct BehaviorGraceState {
    inner: Arc<Mutex<Option<PreviousBehaviorGrace>>>,
}

impl BehaviorGraceState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Begin a grace window after a successful generation commit.
    ///
    /// Replaces any prior window. Only the immediately previous inert manifest
    /// is retained; old runtime/workers/sessions are never revived here.
    pub(crate) async fn begin(
        &self,
        previous_manifest: BehaviorManifest,
        previous_runtime_generation_id: RuntimeGenerationId,
    ) {
        *self.inner.lock().await = Some(PreviousBehaviorGrace::new(
            previous_manifest,
            previous_runtime_generation_id,
            Instant::now(),
        ));
    }

    pub(crate) async fn clear(&self) {
        *self.inner.lock().await = None;
    }

    #[cfg(test)]
    pub(crate) async fn expire_for_test(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(grace) = guard.as_mut() {
            grace.committed_at = Instant::now()
                .checked_sub(Duration::from_millis(PREVIOUS_BEHAVIOR_GRACE_MS + 1))
                .unwrap_or_else(Instant::now);
        }
    }

    #[cfg(test)]
    pub(crate) async fn set_accepted_for_test(&self, accepted: u64) {
        let mut guard = self.inner.lock().await;
        if let Some(grace) = guard.as_mut() {
            grace.accepted_previous_transactions = accepted;
        }
    }

    #[cfg(test)]
    pub(crate) async fn accepted_for_test(&self) -> Option<u64> {
        self.inner
            .lock()
            .await
            .as_ref()
            .map(|grace| grace.accepted_previous_transactions)
    }

    /// Validate an Edit/EditorIntent behavior stamp against current + grace.
    ///
    /// `acknowledged_generation` is this connection's last installed runtime
    /// generation. Once it reaches the active generation, previous-generation
    /// grace is closed for that connection.
    #[allow(
        clippy::too_many_arguments,
        clippy::result_large_err,
        reason = "cold validation path needs explicit identity/version inputs and returns the protocol rejection message directly"
    )]
    pub(crate) async fn validate_edit_version(
        &self,
        current: &ActiveBehaviorManifest,
        client_id: ClientId,
        document_id: DocumentId,
        transaction_id: TransactionId,
        behavior_version: BehaviorVersion,
        current_runtime_generation: RuntimeGenerationId,
        acknowledged_generation: Option<RuntimeGenerationId>,
        now: Instant,
    ) -> Result<BehaviorVersionDecision, ServerMessage> {
        let _ = client_id;
        if behavior_version == current.version() {
            return Ok(BehaviorVersionDecision::Current);
        }

        let mut guard = self.inner.lock().await;
        if let Some(grace) = guard.as_ref()
            && grace.expired(now)
        {
            *guard = None;
        }

        let Some(grace) = guard.as_mut() else {
            return Err(invalid_behavior_version_message(
                document_id,
                transaction_id,
                behavior_version,
                current.version(),
            ));
        };

        let immediately_previous = current_runtime_generation
            .checked_sub(1)
            .is_some_and(|previous| previous == grace.previous_runtime_generation_id);
        let client_still_eligible =
            acknowledged_generation.is_none_or(|acked| acked < current_runtime_generation);

        if immediately_previous
            && client_still_eligible
            && behavior_version == grace.previous_version()
            && grace.accepted_previous_transactions < PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS
        {
            return Ok(BehaviorVersionDecision::PreviousWithinGrace);
        }

        Err(invalid_behavior_version_message(
            document_id,
            transaction_id,
            behavior_version,
            current.version(),
        ))
    }

    /// Record one accepted previous-generation transaction after canonical apply.
    ///
    /// Returns false when the window is gone or already at the ceiling; callers
    /// treat that as a post-apply accounting miss and should not roll back the
    /// already-accepted document mutation (ceilings are checked before apply).
    pub(crate) async fn record_previous_accepted(&self, now: Instant) -> bool {
        let mut guard = self.inner.lock().await;
        let Some(grace) = guard.as_mut() else {
            return false;
        };
        if grace.expired(now) {
            *guard = None;
            return false;
        }
        if grace.accepted_previous_transactions >= PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS {
            *guard = None;
            return false;
        }
        grace.accepted_previous_transactions =
            grace.accepted_previous_transactions.saturating_add(1);
        if grace.accepted_previous_transactions >= PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS {
            *guard = None;
        }
        true
    }
}

fn invalid_behavior_version_message(
    document_id: DocumentId,
    transaction_id: TransactionId,
    behavior_version: BehaviorVersion,
    server_behavior_version: BehaviorVersion,
) -> ServerMessage {
    ServerMessage::EditRejected {
        document_id,
        transaction_id,
        reason: EditRejection::InvalidBehaviorVersion {
            behavior_version,
            server_behavior_version,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        behavior::manifest::ManifestValidationError,
        protocol::{BehaviorManifest, CommandDeclaration, EditRejection, ServerMessage},
    };

    #[test]
    fn server_publish_replacement_increments_behavior_version() {
        let mut state = ActiveBehaviorManifest::default();
        let mut replacement = BehaviorManifest::minimal_text_editing(99);
        replacement.manifest_id = "clay.default.text.replacement".to_string();

        let published = state.publish_replacement(replacement).unwrap();

        assert_eq!(published.behavior_version, 2);
        assert_eq!(state.version(), 2);
        assert_eq!(
            state.manifest().manifest_id,
            "clay.default.text.replacement"
        );
    }

    #[test]
    fn server_rejects_invalid_replacement_without_advancing_behavior_version() {
        let mut state = ActiveBehaviorManifest::default();
        let mut replacement = BehaviorManifest::minimal_text_editing(1);
        replacement.commands.push(CommandDeclaration::client_edit(
            "text.insert",
            "Duplicate Insert",
        ));

        let error = state.publish_replacement(replacement).unwrap_err();

        assert_eq!(
            error,
            ManifestValidationError::DuplicateCommandId {
                command_id: "text.insert".to_string()
            }
        );
        assert_eq!(state.version(), 1);
    }

    #[test]
    fn server_behavior_version_validation_reports_client_and_server_versions() {
        let state = ActiveBehaviorManifest::default();

        let rejection = state.validate_message_version(7, 44, 0).unwrap_err();

        assert_eq!(
            rejection,
            ServerMessage::EditRejected {
                document_id: 7,
                transaction_id: 44,
                reason: EditRejection::InvalidBehaviorVersion {
                    behavior_version: 0,
                    server_behavior_version: 1,
                },
            }
        );
    }

    #[tokio::test]
    async fn grace_accepts_immediately_previous_version_before_ack_deadline_and_cap() {
        let current = ActiveBehaviorManifest::default();
        let mut previous = current.manifest().clone();
        previous.manifest_id = "clay.default.text.previous".to_string();
        let mut next = current.clone();
        next.publish_replacement(BehaviorManifest::minimal_text_editing(2))
            .unwrap();

        let grace = BehaviorGraceState::new();
        grace.begin(previous, 1).await;

        let decision = grace
            .validate_edit_version(&next, 9, 7, 1, 1, 2, None, Instant::now())
            .await
            .expect("previous version within grace");
        assert_eq!(decision, BehaviorVersionDecision::PreviousWithinGrace);
        assert!(grace.record_previous_accepted(Instant::now()).await);
        assert_eq!(grace.accepted_for_test().await, Some(1));
    }

    #[tokio::test]
    async fn grace_rejects_after_ack_deadline_or_transaction_ceiling() {
        let current = ActiveBehaviorManifest::default();
        let previous = current.manifest().clone();
        let mut next = current.clone();
        next.publish_replacement(BehaviorManifest::minimal_text_editing(2))
            .unwrap();
        let grace = BehaviorGraceState::new();
        grace.begin(previous.clone(), 1).await;

        // Acknowledged current generation closes grace for that connection.
        let rejected = grace
            .validate_edit_version(&next, 9, 7, 1, 1, 2, Some(2), Instant::now())
            .await
            .unwrap_err();
        assert!(matches!(
            rejected,
            ServerMessage::EditRejected {
                reason: EditRejection::InvalidBehaviorVersion { .. },
                ..
            }
        ));

        grace.begin(previous.clone(), 1).await;
        grace.expire_for_test().await;
        assert!(
            grace
                .validate_edit_version(&next, 9, 7, 2, 1, 2, None, Instant::now())
                .await
                .is_err()
        );

        grace.begin(previous, 1).await;
        grace
            .set_accepted_for_test(PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS)
            .await;
        assert!(
            grace
                .validate_edit_version(&next, 9, 7, 3, 1, 2, None, Instant::now())
                .await
                .is_err()
        );
    }
}
