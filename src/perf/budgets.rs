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

// Phase 16 primitive architecture budgets. These are advisory until concrete
// protocol messages and package/mode implementations promote them to hard CI
// thresholds.
pub const DECORATION_PAYLOAD_BUDGET_BYTES: usize = 8192;
pub const INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES: usize = 4096;
pub const COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES: usize = 4096;
pub const FOLDING_RANGE_PAYLOAD_BUDGET_BYTES: usize = 2048;
pub const PRIMITIVES_REGISTRY_VERSION: &str = "phase16-primitives-v1";

pub const KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS: u64 = 16;
pub const EDIT_ACK_P95_BUDGET_MS: u64 = 40;
pub const SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS: u64 = 16;
pub const RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS: u64 = 25;
pub const MODE_ACTIVATION_P95_BUDGET_MS: u64 = 100;
pub const LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB: u64 = 256;
