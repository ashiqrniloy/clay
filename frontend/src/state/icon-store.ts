// Active icon-pack runtime state (Plan 112 task 6): holds the latest
// authorized ActiveIconPack snapshot and applies generation rules so stale or
// malformed wire data never displaces the last authorized pack. Dependency-free
// observable store, mirroring design-system-store.ts conventions.

import type { IconGeometry, IconPackSnapshot } from "../icons/types";

export type { IconGeometry, IconPackSnapshot };

export interface IconPackState {
  pack: IconPackSnapshot | null;
}

export interface IconStore {
  get(): IconPackState;
  setIconPack(pack: IconPackSnapshot | null): void;
  /** Geometry for one semantic key, honoring per-key fallback at consumers. */
  getIcon(key: string): IconGeometry | undefined;
  resetToFallback(): void;
  subscribe(listener: () => void): () => void;
}

/** Defensive client-side bounds mirroring the server contract (shell/icons.rs). */
const MAX_ICONS_PER_PACK = 64;
const MAX_PATHS = 8;
const MAX_COORDINATE = 4096;
const MIN_VIEWBOX_SIDE = 1;
const MAX_VIEWBOX_SIDE = 512;

/**
 * Malformed snapshots are rejected, never partially applied. `null` is
 * legitimate (host fallback subset active), everything else must carry the
 * canonical shape with finite numbers.
 */
export function isValidIconPackSnapshot(
  pack: unknown,
): pack is IconPackSnapshot {
  if (pack === null) return true;
  if (typeof pack !== "object") return false;
  const candidate = pack as Partial<IconPackSnapshot>;
  if (
    typeof candidate.specifier !== "string" ||
    candidate.specifier.trim() === "" ||
    candidate.specifier.length > 256 ||
    typeof candidate.generation !== "number" ||
    !Number.isFinite(candidate.generation) ||
    typeof candidate.schemaVersion !== "number"
  ) {
    return false;
  }
  const icons = candidate.icons;
  if (typeof icons !== "object" || icons === null) return false;
  const entries = Object.entries(icons);
  if (entries.length === 0 || entries.length > MAX_ICONS_PER_PACK) return false;
  return entries.every(([, geometry]) => isValidIconGeometry(geometry));
}

function isValidIconGeometry(geometry: unknown): geometry is IconGeometry {
  if (typeof geometry !== "object" || geometry === null) return false;
  const candidate = geometry as Partial<IconGeometry>;
  if (
    !Array.isArray(candidate.viewBox) ||
    candidate.viewBox.length !== 4 ||
    candidate.viewBox.some(
      (v) =>
        typeof v !== "number" ||
        !Number.isFinite(v) ||
        Math.abs(v) > MAX_COORDINATE,
    )
  ) {
    return false;
  }
  // viewBox is [x, y, width, height]; only width/height are range-bound.
  const [, , width, height] = candidate.viewBox;
  if (
    width < MIN_VIEWBOX_SIDE ||
    width > MAX_VIEWBOX_SIDE ||
    height < MIN_VIEWBOX_SIDE ||
    height > MAX_VIEWBOX_SIDE
  ) {
    return false;
  }
  if (!Array.isArray(candidate.paths) || candidate.paths.length === 0)
    return false;
  if (candidate.paths.length > MAX_PATHS) return false;
  return candidate.paths.every((path) => {
    if (typeof path !== "object" || path === null) return false;
    const p = path as { d?: unknown; opacity?: unknown };
    if (typeof p.d !== "string" || p.d === "" || p.d.length > 2048)
      return false;
    if (
      p.opacity !== undefined &&
      (typeof p.opacity !== "number" ||
        !Number.isFinite(p.opacity) ||
        p.opacity < 0 ||
        p.opacity > 1)
    )
      return false;
    return true;
  });
}

export function createIconStore(): IconStore {
  let state: IconPackState = { pack: null };
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of [...listeners]) listener();
  };

  return {
    get: () => state,
    setIconPack(pack) {
      if (!isValidIconPackSnapshot(pack)) {
        // Malformed wire data: retain last authorized state, no churn.
        return;
      }
      if (pack !== null && state.pack !== null) {
        if (
          pack.generation < state.pack.generation ||
          (pack.generation === state.pack.generation &&
            pack.specifier === state.pack.specifier &&
            pack.schemaVersion === state.pack.schemaVersion)
        ) {
          // Stale or identical revision: no-op (no subscriber churn).
          return;
        }
      }
      state = { pack };
      notify();
    },
    getIcon(key) {
      return state.pack?.icons[key];
    },
    resetToFallback() {
      if (state.pack === null) return;
      state = { pack: null };
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
