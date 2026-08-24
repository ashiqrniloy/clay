import type {
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
