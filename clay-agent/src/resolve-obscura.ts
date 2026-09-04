/**
 * Obscura binary resolution: CLAY_OBSCURA_BIN (absolute), then PATH search,
 * then the documented absolute default. Absolute results only; undefined when
 * absent (fail closed to "hidden", never an error).
 */
import { existsSync } from "node:fs";
import { join } from "node:path";

/** Absolute default when PATH has no `obscura`. */
export const DEFAULT_OBSCURA_PATH = "/usr/local/bin/obscura";
const ENV_CACHE_TTL_MS = 5_000;

let cached: { path: string | undefined; at: number } | undefined;

export function resolveObscuraBinary(): string | undefined {
  if (cached && Date.now() - cached.at < ENV_CACHE_TTL_MS) return cached.path;
  let resolved: string | undefined;
  const fromEnv = process.env.CLAY_OBSCURA_BIN;
  const candidates: string[] = [];
  if (fromEnv && fromEnv.startsWith("/")) {
    candidates.push(fromEnv);
  } else {
    for (const dir of (process.env.PATH ?? "").split(":")) {
      if (dir.length === 0) continue;
      candidates.push(join(dir, "obscura"));
    }
    candidates.push(DEFAULT_OBSCURA_PATH);
  }
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      resolved = candidate;
      break;
    }
  }
  cached = { path: resolved, at: Date.now() };
  return resolved;
}

/** Test seam: clear the resolution cache. */
export function resetObscuraBinaryCache(): void {
  cached = undefined;
}
