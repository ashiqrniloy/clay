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

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function getActiveBehaviorManifest(documentId?: string): Promise<BehaviorManifestSummary> {
  void documentId;
  plannedApi("clay.behavior.getActiveBehaviorManifest");
}

export async function listBehaviorRoutes(documentId?: string): Promise<BehaviorRoute[]> {
  void documentId;
  plannedApi("clay.behavior.listBehaviorRoutes");
}
