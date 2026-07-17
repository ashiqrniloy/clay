// Clay package primitive facade skeleton.
//
// Package APIs are server runtime load-time validation APIs. They validate
// package metadata and permissions through typed Rust validators; they do not
// install packages, execute package handlers, or expose raw Deno ops publicly.

const ops = globalThis.Deno?.core?.ops;
// Per-runtime-generation cache: repeated calls are idempotent inside one
// ClayJsRuntimeService. Hot reload invalidates it by swapping to a fresh
// runtime rather than mutating globals/module cache in place, adding a
// force flag, or invoking package-authored reload/migration hooks.
// Generation-local only: this object is empty in every candidate generation.
const loadedPackages = ((globalThis as typeof globalThis & { __clayLoadedPackages?: Record<string, unknown> }).__clayLoadedPackages ??= Object.create(null));

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.packages.runtime_unavailable: Clay package APIs require the server runtime");
  }
  return ops;
}

function parse<T>(json: string): T {
  return JSON.parse(json) as T;
}

export function serverValidatePackageManifest(manifest: unknown): unknown {
  return parse(requireOps().op_clay_packages_validate_manifest(JSON.stringify(manifest ?? null)));
}

export function serverValidatePackagePermissions(permissions: unknown): unknown {
  return parse(requireOps().op_clay_packages_validate_permissions(JSON.stringify(permissions ?? null)));
}

export function serverLoadPackage(packageJson: unknown): unknown {
  return parse(requireOps().op_clay_packages_load_package(JSON.stringify(packageJson ?? null)));
}

export function serverListFirstPartyPackageSpecifiers(): string[] {
  return parse<{ specifiers: string[] }>(requireOps().op_clay_packages_list_first_party_specifiers()).specifiers;
}

function plannedPackageApi(name: string): never {
  const unavailable = Deno?.core?.ops?.op_clay_runtime_unavailable;
  if (typeof unavailable === "function") {
    unavailable(name);
  }
  throw new Error(`${name} is planned; Clay package management op wiring is not implemented yet`);
}

/** Install a package from an npm-compatible specifier and record provenance.
 * Planned: not callable until the op wiring and authorization flow ship. */
export function install(_options: { specifier: string } & Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.install");
}

/** Enable an installed, authorized package and evaluate its package graph.
 * Planned: not callable until the op wiring ships. */
export function enable(_options: { packageName: string } & Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.enable");
}

/** Disable an enabled package and withdraw its contributions.
 * Planned: not callable until the op wiring ships. */
export function disable(_options: { packageName: string } & Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.disable");
}

/** Inspect package metadata, provenance, capabilities, and authorization state.
 * Planned: not callable until the op wiring ships. */
export function inspect(_options: { packageName: string } & Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.inspect");
}

/** List installed/bundled packages with provenance and authorization status.
 * Planned: not callable until the op wiring ships. */
export function list(): never {
  return plannedPackageApi("clay.packages.list");
}

/** Authorize capabilities and a runtime profile for a package.
 * Planned: not callable until the op wiring ships. */
export function authorize(_options: Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.authorize");
}

/** Set an explicit user-selected winner for a package contribution conflict.
 * Planned: not callable until the op wiring ships. */
export function setConflictOverride(_options: { contributionId: string; winnerPackage: string } & Record<string, unknown>): never {
  return plannedPackageApi("clay.packages.setConflictOverride");
}

/** Load and activate an installed, user-authorized package by specifier.
 *
 * This is the one-line default end-user package loader (e.g.
 * `await loadPackage("@clay/markdown")`, `await loadPackage("@vendor/foo")`,
 * or `await loadPackage("github:user/repo")` from `~/.config/clay/init.js`). It
 * resolves + validates + authorizes + enables the package through the
 * authoritative PackageService path, then imports the package's declared
 * `loadEntry` so the package registers modes, commands, parse handlers, and
 * decorations under Clay's authority. Repeated calls within one runtime
 * generation return the cached summary; hot reload reruns `init.js` in a fresh
 * generation so the cache starts empty and every `loadEntry` rebuilds
 * declarations from durable configuration/package metadata. There is no
 * `loadPackage(spec, { force: true })` and no package reload callback API.
 * The module loader imports only canonical loadEntry paths recorded in the
 * validated package allowlist, and relative imports remain confined to the
 * package root. */
export async function loadPackage(specifier: string): Promise<unknown> {
  if (typeof specifier !== "string") {
    throw new Error("clay.packages.invalid_specifier: loadPackage requires a string specifier");
  }
  if (loadedPackages[specifier]) {
    return loadedPackages[specifier];
  }
  const result = parse<{ loadEntrySpecifier: string }>(
    requireOps().op_clay_packages_load_package_by_specifier(JSON.stringify({ specifier })),
  );
  // Import the validated on-disk loadEntry, then invoke its default export so
  // the package activates (registers modes/commands/parse handlers) under Clay's
  // authority. The loadEntry contract is: a module whose default export is the
  // activation function. Curated `clay:` facade imports inside the loadEntry are
  // always allowed by the module loader; no new authority is granted here.
  const loadEntry = await import(result.loadEntrySpecifier);
  if (typeof loadEntry.default === "function") {
    await loadEntry.default();
  }
  loadedPackages[specifier] = result;
  return result;
}
