// Clay behavior manifest facade skeleton.
//
// Behavior manifests keep hot-path client behavior inert and predictable. These
// planned APIs query or inspect manifests; they do not execute arbitrary
// JavaScript in the Rust client.

export interface BehaviorManifestSummary {
  id: string;
  documentId?: string;
  version: number;
  clientFirstBehaviors: string[];
}

export interface BehaviorRoute {
  input: string;
  runtimePath: "client-first" | "server-first" | "background";
  apiId?: string;
}

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.behavior.runtime_unavailable: Clay behavior APIs require the server runtime");
  }
  return ops;
}

export async function getActiveBehaviorManifest(documentId?: string): Promise<BehaviorManifestSummary> {
  return JSON.parse(requireOps().op_clay_behavior_get_active_manifest(JSON.stringify(documentId ?? null)));
}

export async function listBehaviorRoutes(documentId?: string): Promise<BehaviorRoute[]> {
  return JSON.parse(requireOps().op_clay_behavior_list_routes(JSON.stringify(documentId ?? null)));
}
