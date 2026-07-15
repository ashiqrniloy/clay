// Phase 18.20 language-server process/session budgets. A language-server
// session is an authorized, host-owned child process speaking an opaque
// bounded byte stream (UTF-8 JSON-RPC for LSP adapters in Phase 18.21). These
// are server-owned security/performance ceilings, not user configuration:
// every read/write/stderr/process-count is hard-capped before it can allocate
// or linger, and diagnostics are sanitized. These bytes never enter the IPC
// codec frame budget; they cross only the host<->child stdio boundary.
/// Maximum bytes for one language-server stdin write / stdout read.
pub const LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES: usize = 256 * 1024;
/// Maximum accumulated child stderr retained for sanitized diagnostics.
pub const LANGUAGE_SERVER_STDERR_BUDGET_BYTES: usize = 64 * 1024;
/// Maximum concurrent language-server sessions per runtime generation.
pub const LANGUAGE_SERVER_MAX_SESSIONS: usize = 16;
/// Default wall-clock timeout for a single language-server stdout read.
pub const LANGUAGE_SERVER_READ_TIMEOUT_MS: u64 = 30_000;

pub const CLIENT_EDIT_PAYLOAD_BUDGET_BYTES: usize = 512;
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
/// Oversize manifests are rejected with `ManifestValidationFailed`/
/// `PayloadBudgetExceeded` at record time.
pub const BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const SDUI_UPDATE_PAYLOAD_BUDGET_BYTES: usize = 1024;
/// One validated three-profile typography snapshot. Family/profile limits are
/// checked before publication; this bounds its serialized protocol envelope.
pub const TYPOGRAPHY_PAYLOAD_BUDGET_BYTES: usize = 1024;

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

// Phase 18.8 command execution and transient menu budgets.
pub const COMMAND_ARGUMENT_BUDGET_BYTES: usize = 4 * 1024;
pub const TRANSIENT_MENU_MAX_ITEMS: usize = 256;
pub const TRANSIENT_MENU_MAX_QUERY_CHARS: usize = 256;
pub const TRANSIENT_MENU_MAX_LABEL_CHARS: usize = 128;
pub const TRANSIENT_MENU_MAX_DETAIL_CHARS: usize = 256;
pub const TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS: usize = 256;
pub const PRIMITIVES_REGISTRY_VERSION: &str = "phase16-primitives-v1";

pub const KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS: u64 = 16;
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
// is a memory-exhaustion vector. This gate is checked against already-fetched
// file metadata *before* `tokio_fs::read` allocates, so oversized files are
// rejected with a typed `FileTooLarge` error without ever being read.
//
// The value sits below the 1 MiB frame limit to leave headroom for the message
// envelope (variant tag + `DocumentMetadata` + rkyv overhead) so any file that
// passes this gate also fits in a single full-text frame. Larger files require
// the chunked/viewport-first loading path, which remains a documented follow-up
// (see plan 030). `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` is the future
// resident-memory budget for that chunked path and is intentionally much larger.
pub const MAX_OPENABLE_FILE_BYTES: usize = 768 * 1024;
