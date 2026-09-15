// Active icon-pack wire types (Plan 112 task 6).
//
// The shapes are generated from the Rust DTO layer (`src-tauri/src/bridge/dto.rs`);
// these aliases keep the icon module's own vocabulary (plan 119 SC-1).

export type {
  IconGeometryDto as IconGeometry,
  IconPackSnapshotDto as IconPackSnapshot,
  IconPathDto as IconPathData,
} from "../bridge/types";
