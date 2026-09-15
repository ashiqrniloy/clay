import type { PackageUiSnapshotDto } from "../bridge/types";
import type {
  PackageComponentNode,
  PackageUiSnapshot,
  SduiNode,
  SduiTree,
  SduiTreeUpdate,
} from "./types";

export interface SduiState {
  version: number;
  rootId: number;
  nodes: ReadonlyMap<number, SduiNode>;
}

export interface UiProjectionState {
  runtimeGeneration: number;
  sdui: SduiState | null;
  packageUi: PackageUiSnapshot | null;
}

export const emptyUiProjection = (): UiProjectionState => ({
  runtimeGeneration: 0,
  sdui: null,
  packageUi: null,
});

/**
 * Narrow the generated wire payload to the app-level snapshot shape.
 *
 * Rust parses each surface's component JSON and bounds it, but shape validation
 * is the renderer's job (unknown node kinds render nothing), so this is the one
 * place where a `JsonValue` becomes a `PackageComponentNode` — a documented
 * narrowing at the bridge boundary instead of a hand-written mirror.
 */
export function installPackageUi(
  snapshot: PackageUiSnapshotDto,
): PackageUiSnapshot {
  const narrow = (surface: { component: unknown }): PackageComponentNode =>
    surface.component as PackageComponentNode;
  return {
    version: snapshot.version,
    emptyTab: snapshot.emptyTab
      ? { ...snapshot.emptyTab, component: narrow(snapshot.emptyTab) }
      : null,
    surfaces: snapshot.surfaces.map((surface) => ({
      ...surface,
      component: narrow(surface),
    })),
    panels: snapshot.panels.map((panel) => ({
      ...panel,
      component: narrow(panel),
    })) as PackageUiSnapshot["panels"],
    overlays: snapshot.overlays.map((overlay) => ({
      ...overlay,
      component: narrow(overlay),
    })) as PackageUiSnapshot["overlays"],
    components: snapshot.components.map((surface) => ({
      ...surface,
      component: narrow(surface),
    })),
    inputRoutes: snapshot.inputRoutes,
  };
}

export function installSduiTree(tree: SduiTree): SduiState {
  return {
    version: tree.uiVersion,
    rootId: tree.rootId,
    nodes: new Map(tree.nodes.map((node) => [node.id, node])),
  };
}

/** Stale updates are ignored; surviving node IDs keep React keys and state. */
export function applySduiUpdate(
  current: SduiState | null,
  update: SduiTreeUpdate,
): SduiState | null {
  if (!current || current.version !== update.baseUiVersion) return current;
  const nodes = new Map(current.nodes);
  let rootId = current.rootId;
  for (const operation of update.operations) {
    if ("replaceRoot" in operation) rootId = operation.replaceRoot.rootId;
    if ("replaceNode" in operation) {
      nodes.set(operation.replaceNode.node.id, operation.replaceNode.node);
    }
    if ("removeNode" in operation) nodes.delete(operation.removeNode.nodeId);
  }
  return { version: update.newUiVersion, rootId, nodes };
}
