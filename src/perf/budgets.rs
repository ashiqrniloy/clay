// Language-server process/session budgets. A session is an authorized,
// host-owned child process speaking an opaque bounded byte stream. These are
// server-owned security/performance ceilings, not user configuration: every
// read/write/stderr/process-count is hard-capped before it can allocate or
// linger, and diagnostics are sanitized. These bytes never enter the IPC codec
// frame budget; they cross only the host<->child stdio boundary.
/// Maximum exact bytes for one language-server stdin write / stdout read.
///
/// Phase 18.21 raises the former 256 KiB text-message ceiling to the approved
/// 1 MiB serialized-frame ceiling. Package adapters still own framing.
pub const LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES: usize = 1024 * 1024;
/// Maximum accumulated child stderr retained for sanitized diagnostics.
pub const LANGUAGE_SERVER_STDERR_BUDGET_BYTES: usize = 64 * 1024;
/// Maximum concurrent language-server sessions per runtime generation.
pub const LANGUAGE_SERVER_MAX_SESSIONS: usize = 16;
/// Default wall-clock timeout for a single language-server stdout read.
pub const LANGUAGE_SERVER_READ_TIMEOUT_MS: u64 = 30_000;
/// Bounded ingress per language-server session actor. A full queue rejects
/// new work rather than blocking the central identity/session router.
pub(crate) const LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY: usize = 8;

// Background filesystem/process concurrency ceilings (Plan 060 T8). These
// are compiled server budgets, not user configuration.
/// Concurrent blocking workspace directory traversals.
pub(crate) const DIRECTORY_LISTING_MAX_CONCURRENCY: usize = 4;
/// Concurrent Git roots; commands within one root remain sequential.
pub(crate) const GIT_ROOT_CONCURRENCY: usize = 4;

// Approved Phase 18.21 analyzer-neutral package-worker limits.
pub const DOCUMENT_ANALYSIS_MAX_WORKERS: usize = 4;
pub const DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES: usize = 64 * 1024 * 1024;
pub const DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER: usize = 32;
pub const DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES: usize = 256 * 1024;
pub const DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER: usize = 8 * 1024 * 1024;
pub const DOCUMENT_ANALYSIS_MAX_DELTA_BYTES: usize = 64 * 1024;
pub const DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS: usize = 64;
pub const DOCUMENT_ANALYSIS_INPUT_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS: usize = 64;
pub const DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES: usize = 512 * 1024;
pub const DOCUMENT_ANALYSIS_MAX_PENDING_REQUESTS: usize = 8;
pub const DOCUMENT_ANALYSIS_HANDLER_TIMEOUT_MS: u64 = 5_000;
pub const DOCUMENT_ANALYSIS_GRACEFUL_SHUTDOWN_MS: u64 = 2_000;
pub const DOCUMENT_ANALYSIS_TOTAL_SHUTDOWN_MS: u64 = 5_000;

pub const CLIENT_EDIT_PAYLOAD_BUDGET_BYTES: usize = 512;
// Per-document client undo/redo depth (Phase 20). Aligned with pending-edit /
// previous-behavior-grace transaction ceilings.
pub const EDIT_HISTORY_MAX_DEPTH: usize = 256;
// Max combined forward+inverse text bytes retained in one history entry.
// Oversized edits clear history instead of retaining unbounded payloads.
pub const EDIT_HISTORY_MAX_ENTRY_BYTES: usize = 64 * 1024;
// Max retained client document sessions including the active document (Phase 20).
// Aligned with `RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS`.
pub const CLIENT_DOCUMENT_SESSION_MAX: usize = 64;

// Per-connection ceiling on simultaneously open server documents, aligned with
// the client retained-session budget: a client that retains at most 64
// sessions never legitimately holds more server documents open.
pub(crate) const MAX_DOCUMENTS_PER_CLIENT: usize = CLIENT_DOCUMENT_SESSION_MAX;
// Active IPC connection ceiling enforced at the accept loop via a semaphore
// permit owned by each connection task; excess connections are refused at
// accept time instead of spawning unbounded tasks (Plan 060 T6, P1-10).
pub(crate) const MAX_ACTIVE_CONNECTIONS: usize = 64;
// Server-wide open-document ceiling: every connection could hold the
// per-client maximum. Beyond this, opens fail closed with
// `WorkspaceLimitExceeded` rather than growing workspace registries without
// bound.
pub(crate) const MAX_SERVER_DOCUMENTS: usize = MAX_ACTIVE_CONNECTIONS * MAX_DOCUMENTS_PER_CLIENT;
// Per-connection completion / language-intelligence result lane capacity.
// These lanes carry only request-scoped results; a saturated lane means the
// client is not reading, so the server drops with a diagnostic instead of
// growing memory.
pub(crate) const CONNECTION_RESULT_LANE_CAPACITY: usize = 64;
// Server-side runtime-diagnostic retention: bounded deduplicating deque,
// aligned with the snapshot publication cap so welcome/runtime snapshots
// never grow past the frame budget.
pub(crate) const RUNTIME_DIAGNOSTIC_CAPACITY: usize = RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS;
// Edit acknowledgement payload budget.  Advisory: rkyv union-layout sizing means
// the serialized size of `ServerMessage::EditAck` grows with the largest enum
// variant.  128 bytes reflects the current union floor after adding completion
// result/rejection variants and leaves a small fixed-envelope margin without
// changing the edit-ack message shape.
pub const EDIT_ACK_PAYLOAD_BUDGET_BYTES: usize = 128;
/// Bounded load-time manifest metadata for one package, including its inert
/// `clay.contributions.*` declarations. Sized at 4096 bytes (Plan 046) to
/// accommodate first-party theme packages that declare a full inert
/// `textStyles` mapping (every `TokenType` family + base UI color ~45 entries,
/// ~2.3 KB) alongside their manifest, while still bounding package-load cost.
/// Raised to 8192 bytes (Plan 061 task 8) so manifests can also carry their
/// versioned `clay.extensionPoints` declarations (locked schema cap: 64
/// points) — the largest first-party manifest (`@clay/markdown`, ~5.5 KB)
/// defines the headroom. Oversize manifests are rejected with
/// `ManifestValidationFailed`/`PayloadBudgetExceeded` at record time.
pub const BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES: usize = 8192;
pub const SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const SDUI_UPDATE_PAYLOAD_BUDGET_BYTES: usize = 1024;
/// One validated three-profile typography snapshot. Family/profile limits are
/// checked before publication; this bounds its serialized protocol envelope.
pub const TYPOGRAPHY_PAYLOAD_BUDGET_BYTES: usize = 1024;
/// One cross-domain extension request or result payload (inert JSON bytes).
/// Rust-mediated only; checked before allocation-heavy parsing so a hostile
/// sibling package cannot force unbounded buffering across the trust boundary.
pub const CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES: usize = 8192;

// Runtime SDUI `publishTree` budgets. A package- or config-published tree is
// untrusted input parsed into a `serde_json::Value` and then converted into a
// `SduiTree`; these bounds reject a malicious or runaway huge tree before it
// can allocate proportional memory, recurse deeply, or carry oversized text.
// `RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES` is checked against the raw
// `tree_json.len()` before parsing; the remaining three are enforced while the
// tree is built. Node/depth caps mirror the registered package-UI discipline
// (`MAX_COMPONENT_NODES = 128`).
pub const RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES: usize = 16 * 1024;
pub const RUNTIME_SDUI_TREE_MAX_NODES: usize = 128;
pub const RUNTIME_SDUI_TREE_MAX_DEPTH: usize = 16;
pub const RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS: usize = 4096;

// Phase 16 primitive architecture budgets. These are advisory until concrete
// protocol messages and package/mode implementations promote them to hard CI
// thresholds.
pub const DECORATION_PAYLOAD_BUDGET_BYTES: usize = 8192;
/// One viewport/source diagnostic replacement payload and its metadata limits.
pub const DIAGNOSTIC_PAYLOAD_BUDGET_BYTES: usize = 8192;
pub const DIAGNOSTIC_MAX_SPANS_PER_SET: usize = 128;
pub const DIAGNOSTIC_MAX_CODE_BYTES: usize = 128;
pub const DIAGNOSTIC_MAX_MESSAGE_BYTES: usize = 1024;
pub const DIAGNOSTIC_MAX_SOURCE_BYTES: usize = 128;
pub const DIAGNOSTIC_MAX_PROVENANCE_FIELD_BYTES: usize = 256;
pub const DIAGNOSTIC_CACHE_BUDGET_BYTES: usize = 8 * 1024 * 1024;
pub const DECORATION_NEAR_VIEWPORT_GUARD_BYTES: u64 = 256 * 1024;
pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;
// Generic retained syntax/decorator cache budget for large-file modes. Phase
// 18.5 uses this as the 30 MiB Markdown-specific overhead target while keeping
// the primitive language-neutral for future modes.
pub const SYNTAX_CACHE_BUDGET_BYTES: usize = 30 * 1024 * 1024;
// Phase 18.11 completion result payload budget. A completion result reuses the
// `TransientMenuSession` picker, which caps display at `TRANSIENT_MENU_MAX_ITEMS`
// (256), so the wire budget must accommodate a full 256-item result with short
// labels. A representative 256-item, short-label result serializes to ~14.5 KiB
// via rkyv; 16 KiB leaves headroom for the envelope and provenance while staying
// in the same order of magnitude as `RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES`.
// Per-item and per-field budgets below are the finer guards that keep a single
// item or field from blowing the whole result budget.
pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 16 * 1024;
// Phase 18.11 completion request payload budget. A `CompletionRequest` carries
// only request/document/version/cursor/range/trigger metadata (no document
// text), so this is a small ceiling checked before the request is dispatched to
// the server-side provider lane.
pub const COMPLETION_REQUEST_PAYLOAD_BUDGET_BYTES: usize = 512;
// Phase 18.11 completion result item and per-field budgets. The result payload
// budget above is the hard ceiling checked before client publication; these
// finer budgets let a single item or field not blow the whole result budget and
// keep completion picker strings inside the transient menu label/detail caps.
pub const COMPLETION_RESULT_MAX_ITEMS: usize = 256;
pub const COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS: usize = 128;
pub const COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS: usize = 256;
pub const COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS: usize = 256;
pub const COMPLETION_RESULT_MAX_ITEM_COMMIT_CHARS: usize = 32;
pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;

// Phase 18.20 engine-neutral language-intelligence budgets. Canonical
// positions are UTF-8 byte offsets against Clay documents or known
// workspace-root-relative paths; LSP line/character/URI conversion lives in
// Phase 18.21 package adapters, never in core protocol types.
pub const LANGUAGE_INTELLIGENCE_REQUEST_PAYLOAD_BUDGET_BYTES: usize = 512;
pub const LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES: usize = 16 * 1024;
pub const LANGUAGE_INTELLIGENCE_MAX_DEFINITION_LOCATIONS: usize = 64;
pub const LANGUAGE_INTELLIGENCE_MAX_CODE_ACTIONS: usize = 64;
pub const LANGUAGE_INTELLIGENCE_MAX_SIGNATURES: usize = 16;
pub const LANGUAGE_INTELLIGENCE_MAX_PARAMETERS: usize = 32;
pub const LANGUAGE_INTELLIGENCE_MAX_EDITS_PER_PREVIEW: usize = 32;
pub const LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS: usize = 4096;
pub const LANGUAGE_INTELLIGENCE_MAX_TITLE_CHARS: usize = 256;
pub const LANGUAGE_INTELLIGENCE_MAX_LABEL_CHARS: usize = 256;
pub const LANGUAGE_INTELLIGENCE_MAX_DOCUMENTATION_CHARS: usize = 1024;
pub const LANGUAGE_INTELLIGENCE_MAX_EDIT_CHARS: usize = 4096;
pub const LANGUAGE_INTELLIGENCE_MAX_RELATIVE_PATH_CHARS: usize = 512;
pub const LANGUAGE_INTELLIGENCE_MAX_PROVENANCE_FIELD_CHARS: usize = 256;
/// Hard ceiling on concurrent in-flight language-intelligence tasks per
/// coordinator. Additional schedule attempts fail closed until a slot frees.
pub const LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS: usize = 16;
/// Default per-provider timeout when a contribution omits `timeoutMs`.
pub const LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS: u64 = 500;
/// Hard ceiling on per-provider timeout. Matches the completion lane.
pub const LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS: u64 = 5_000;
/// Bounded open-document text slice handed to a provider. Same size as the
/// completion window so analyzers never see an unbounded document.
pub const LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES: usize = 64 * 1024;

// Phase 19 runtime-generation snapshot budgets. Complete snapshots reuse the
// existing 1 MiB codec frame ceiling (`DEFAULT_MAX_FRAME_SIZE`). Document and
// diagnostic counts keep a single connection-scoped snapshot bounded before
// encode. Diffs/chunking are deferred until measured payload reaches
// `RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES` p95 or client install
// exceeds `RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS`.
pub const RUNTIME_STATE_BROADCAST_CAPACITY: usize = 16;
pub const RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS: usize = 64;
pub const RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS: usize = 32;
pub const RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES: usize = 768 * 1024;
pub const RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS: u64 = 16;
/// Fixed stale-edit grace after a successful runtime-generation commit.
///
/// Previous-generation `Edit`/`EditorIntent` stamps remain eligible only until
/// the first of: per-connection G2 acknowledgement, this deadline, the
/// accepted-transaction ceiling below, another commit, or shutdown. Not user
/// configuration.
pub const PREVIOUS_BEHAVIOR_GRACE_MS: u64 = 2_000;
/// Maximum previous-generation transactions accepted during one grace window.
pub const PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS: u64 = 256;

// Phase 18.8 command execution and transient menu budgets.
pub const COMMAND_ARGUMENT_BUDGET_BYTES: usize = 4 * 1024;
pub const TRANSIENT_MENU_MAX_ITEMS: usize = 256;
pub const TRANSIENT_MENU_MAX_QUERY_CHARS: usize = 256;
pub const TRANSIENT_MENU_MAX_LABEL_CHARS: usize = 128;
pub const TRANSIENT_MENU_MAX_DETAIL_CHARS: usize = 256;
pub const TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS: usize = 256;
pub const PRIMITIVES_REGISTRY_VERSION: &str = "phase16-primitives-v1";

pub const KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS: u64 = 16;

// Phase 22.6 (plan 077 task 5) window-model performance budgets. The two
// wall-clock budgets are advisory, pinned from `cargo bench --bench
// window_baselines` measurements (docs/development/performance.md, Phase
// 22.6 section); per the Phase 21 promotion rule they become hard CI
// thresholds only after stable CI-runner evidence. The deterministic
// CI-blocking gates are the work-count/payload invariants in
// tests/editor_performance_invariants.rs and this file's value pins.
/// One pane's paint: shell chrome geometry (dividers, slot handles, focus
/// ring) plus the pane's own surface paint (viewport-bounded, benched
/// separately). Measured `window_baselines` pane_paint_baselines; pinned
/// with margin.
pub const PANE_PAINT_P95_BUDGET_MS: u64 = 1;
/// One tab switch: mount the target tab's chrome at its pane rects and
/// repaint its geometry. No document reserialization or IPC — the switch
/// path sends no client messages (deterministic gate in
/// tests/editor_performance_invariants.rs). Measured `window_baselines`
/// tab_switch_baselines; pinned with margin.
pub const TAB_SWITCH_P95_BUDGET_MS: u64 = 1;
/// One decoration update aggregated across a 4-pane window: 4 panes × the
/// per-pane decoration payload budget. Hard gate: per-pane payloads are
/// still bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`, and the 4-pane
/// aggregate must stay within this ceiling (deterministic test in
/// tests/editor_performance_invariants.rs).
pub const MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES: usize = 4 * DECORATION_PAYLOAD_BUDGET_BYTES;
pub const EDIT_ACK_P95_BUDGET_MS: u64 = 40;
pub const SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS: u64 = 16;
// Hard wall-clock budget for a single server-side JavaScript runtime
// evaluation (configuration loading, controlled module evaluation, package
// loadEntry). A `while (true) {}` or runaway init.js cannot hang startup beyond
// this; the isolate is terminated via `v8::IsolateHandle::terminate_execution`
// and surfaced as a `clay.runtime.timeout` diagnostic.
pub const JS_RUNTIME_EVALUATION_TIMEOUT_MS: u64 = 5000;
// Hard V8 heap ceiling for Clay's in-process first-party JavaScript runtime.
// Server-owned security budget, not user configuration. Near-limit callback
// terminates execution and surfaces `clay.runtime.heap_limit`.
pub const JS_RUNTIME_HEAP_LIMIT_BYTES: usize = 128 * 1024 * 1024;
pub const RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS: u64 = 25;
pub const MODE_ACTIVATION_P95_BUDGET_MS: u64 = 100;
pub const LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB: u64 = 256;

// Hard size gate for opening a file from disk into a server document.
//
// Full-text protocol messages (`InitialDocument`, `ResyncSnapshot`,
// `DocumentOpened`, `DocumentReloaded`) carry the entire document `String` in a
// single rkyv frame, and the IPC codec caps a frame at
// `DEFAULT_MAX_FRAME_SIZE` (1 MiB). A file at or near that limit would open
// successfully only to fail at frame encode, and reading it into memory first
// is a memory-exhaustion vector. Open/reload read through one opened handle
// (`read_file_bounded` in the workspace): handle metadata is checked against
// this gate before allocation and the read itself is capped at this value
// plus one byte, so oversized files are rejected with a typed `FileTooLarge`
// error even if the file grows between validation and read.
//
// The value sits below the 1 MiB frame limit to leave headroom for the message
// envelope (variant tag + `DocumentMetadata` + rkyv overhead) so any file that
// passes this gate also fits in a single full-text frame. Larger files require
// the chunked/viewport-first loading path, which remains a documented follow-up
// (see plan 030). `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` is the future
// resident-memory budget for that chunked path and is intentionally much larger.
pub const MAX_OPENABLE_FILE_BYTES: usize = 768 * 1024;

/// Hard size gate for small trusted-local auxiliary reads (e.g. a workspace
/// root `.gitignore`). These files are read in full into memory, so a
/// symlinked or absurdly large helper file would otherwise be a
/// memory-exhaustion vector. Reads use one opened handle and allocate at most
/// this plus one byte; overflow aborts listing with a bounded diagnostic.
pub(crate) const MAX_AUXILIARY_READ_BYTES: usize = 1024 * 1024;
/// Maximum lines inspected in the root `.gitignore` used by directory listing.
pub(crate) const MAX_GITIGNORE_LINES: usize = 4096;
/// Maximum accepted root `.gitignore` rules retained by one listing.
pub(crate) const MAX_GITIGNORE_PATTERNS: usize = 1024;
/// Maximum Unicode scalar count in one supported root `.gitignore` rule.
pub(crate) const MAX_GITIGNORE_PATTERN_CHARS: usize = 256;
