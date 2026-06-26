pub const CLIENT_EDIT_PAYLOAD_BUDGET_BYTES: usize = 512;
// Edit acknowledgement payload budget.  Advisory: rkyv union-layout sizing means
// the serialized size of `ServerMessage::EditAck` grows with the largest enum
// variant.  112 bytes reflects the current union floor after adding
// `Vec<String>`-carrying `EnterRule::ContinueLineMarkers` and
// `EnterRule::PreserveFenceBodyIndent` variants (Phase 18 — usable by any mode).
pub const EDIT_ACK_PAYLOAD_BUDGET_BYTES: usize = 112;
pub const BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES: usize = 2048;
pub const SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const SDUI_UPDATE_PAYLOAD_BUDGET_BYTES: usize = 1024;

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
pub const DECORATION_NEAR_VIEWPORT_GUARD_BYTES: u64 = 256 * 1024;
pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;
// Generic retained syntax/decorator cache budget for large-file modes. Phase
// 18.5 uses this as the 30 MiB Markdown-specific overhead target while keeping
// the primitive language-neutral for future modes.
pub const SYNTAX_CACHE_BUDGET_BYTES: usize = 30 * 1024 * 1024;
pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;
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
