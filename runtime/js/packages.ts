// Clay package primitive facade skeleton.
//
// Package APIs are server runtime load-time validation APIs. They validate
// package metadata and permissions through typed Rust validators; they do not
// install packages, execute package handlers, or expose raw Deno ops publicly.

const ops = globalThis.Deno?.core?.ops;

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
