// Clay package primitive facade skeleton.
//
// Package APIs are server runtime load-time validation APIs. They validate
// package metadata and permissions through typed Rust validators; they do not
// install packages, execute package handlers, or expose raw Deno ops publicly.

const ops = globalThis.Deno?.core?.ops;
// Per-runtime-generation cache: repeated calls are idempotent inside one
// ClayJsRuntimeService, and hot reload invalidates it by swapping to a fresh
// runtime rather than mutating globals/module cache in place.
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

/** Load and activate a first-party `@clay/*` package by specifier.
 *
 * This is the one-line default end-user package loader (e.g.
 * `await loadPackage("@clay/markdown")` from `~/.config/clay/init.js`). It
 * resolves + validates + enables the package through the authoritative
 * PackageService path, then imports the package's declared `loadEntry` so the
 * package registers its modes, commands, parse handlers, and decorations under
 * Clay's authority. Repeated calls within one runtime generation return the
 * cached summary; hot reload reruns `init.js` in a fresh generation so the
 * cache starts empty. Only first-party `@clay/*` specifiers are accepted; all
 * other authority (filesystem/network/shell/package-manager/enable-disable) is
 * denied by the op and the module loader. */
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
