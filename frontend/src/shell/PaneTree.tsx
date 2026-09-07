import { lazy, Suspense, useSyncExternalStore } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";

import { ClayButton, ClayText } from "../components";
import type { PackageSurface, PackageUiSnapshot } from "../sdui/types";
import { ClayEditor } from "../editor/ClayEditor";
import type { SplitNode } from "./split-tree";
import type { PaneRecord, TabRuntime } from "./workspace-controller";
import type { ServerKeyStroke } from "./workspace-controller";
import type { EffortChord } from "../coding-agent/CodingAgentPanel";

import styles from "./pane-tree.module.css";

const PackageSurfaceView = lazy(async () => {
  const module = await import("../sdui/registry");
  return { default: module.PackageSurfaceView };
});

const ChatPanel = lazy(async () => {
  const module = await import("../chat/ChatPanel");
  return { default: module.ChatPanel };
});

const CodingAgentPanel = lazy(async () => {
  const module = await import("../coding-agent/CodingAgentPanel");
  return { default: module.CodingAgentPanel };
});

/** Trusted `@clay/coding-agent` pane surface, if the package contributed one. */
function codingAgentSurface(
  packageUi: PackageUiSnapshot | null,
): PackageSurface | null {
  return (
    packageUi?.surfaces?.find(
      (surface) =>
        surface.provenance.packageName === "@clay/coding-agent" &&
        surface.provenance.trustDomain === "trusted",
    ) ?? null
  );
}

export interface PaneTreeProps {
  runtime: TabRuntime;
  node: SplitNode;
  path?: Array<"first" | "second">;
  onFocus: (paneId: number) => void;
  onRatio: (path: Array<"first" | "second">, ratio: number) => void;
  onOpenPath: (path: string) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onLaunchAgent: () => void;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
}

/** Plan 109 I4: the coding-agent surface's effort-cycle chord, read from
 *  the pane session's behavior manifest so `bindKey` overrides apply.
 *  Default `Shift+Tab` when unbound. */
function effortChordOf(pane: {
  session: { behaviorManifest(): { keymaps?: unknown } };
}): EffortChord | null {
  const keymaps =
    (pane.session.behaviorManifest().keymaps as
      | Array<{ commandId: string; sequence?: ServerKeyStroke[] }>
      | undefined) ?? [];
  const binding = keymaps.find(
    (entry) => entry.commandId === "coding-agent.clientCycleEffort",
  );
  const stroke = binding?.sequence?.at(-1);
  if (!stroke) return { shift: true, ctrl: false, alt: false, meta: false, key: "Tab" };
  const raw = typeof stroke.key === "string" ? stroke.key : stroke.key.character;
  if (raw.length === 0) return null;
  return {
    shift: stroke.modifiers.shift,
    ctrl: stroke.modifiers.control,
    alt: stroke.modifiers.alt,
    meta: stroke.modifiers.superKey,
    key: raw,
  };
}

function PaneContent({
  pane,
  packageUi,
  uiVersion,
  onOpenPath,
  onOpenFile,
  onOpenFolder,
  onLaunchAgent,
  runtime,
}: {
  pane: PaneRecord;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
  onOpenPath: (path: string) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onLaunchAgent: () => void;
  runtime: TabRuntime;
  /** Plan 109 I4: effective effort-cycle chord from the manifest. */
  effortChord: EffortChord | null;
}) {
  const meta = useSyncExternalStore(
    pane.session.store.subscribe,
    pane.session.store.get,
  );
  const empty = !meta?.path && pane.session.snapshotDoc().length === 0;
  // Coding Agent split surface (plan 108 task 8): launched per pane via the
  // `coding-agent.profile` command; provenance-exact host rendering like the
  // chat landing, generic SDUI renderer for every other package.
  if (
    runtime.agentSurfaceOpen &&
    runtime.agentSurfacePaneId === pane.paneId &&
    packageUi
  ) {
    const surface = codingAgentSurface(packageUi);
    if (surface) {
      const isBundledAgent =
        surface.provenance.packageName === "@clay/coding-agent" &&
        surface.provenance.trustDomain === "trusted";
      return (
        <Suspense
          fallback={
            <div className={styles.empty} role="status">
              <ClayText variant="body" muted>
                Loading package surface…
              </ClayText>
            </div>
          }
        >
          {isBundledAgent ? (
            <CodingAgentPanel
              surface={surface}
              uiVersion={uiVersion}
              workspaceRoot={runtime.workspaceRoot}
              session={pane.session}
              send={pane.session.request}
              effortChord={effortChordOf(pane)}
            />
          ) : (
            <PackageSurfaceView
              surface={surface}
              uiVersion={uiVersion}
              send={pane.session.request}
            />
          )}
        </Suspense>
      );
    }
  }
  if (empty && packageUi?.emptyTab) {
    // Provenance-exact host rendering for the bundled chat landing
    // (Phase 10). Every other package keeps the inert SDUI renderer.
    const emptyTab = packageUi.emptyTab;
    const isBundledChat =
      emptyTab.provenance.packageName === "@clay/chat" &&
      emptyTab.provenance.trustDomain === "trusted";
    return (
      <Suspense
        fallback={
          <div className={styles.empty} role="status">
            <ClayText variant="body" muted>
              Loading package surface…
            </ClayText>
          </div>
        }
      >
        {isBundledChat ? (
          <ChatPanel surface={emptyTab} uiVersion={uiVersion} />
        ) : (
          <PackageSurfaceView
            surface={emptyTab}
            uiVersion={uiVersion}
            send={pane.session.request}
          />
        )}
      </Suspense>
    );
  }
  if (empty) {
    return (
      <div className={styles.empty} role="group" aria-label="Empty tab">
        <ClayText variant="title">Start with a file or folder</ClayText>
        <div className={styles.emptyActions}>
          <ClayButton onPress={onOpenFile}>Open file</ClayButton>
          <ClayButton onPress={onOpenFolder}>Open folder</ClayButton>
          <ClayButton onPress={onLaunchAgent}>Coding Agent</ClayButton>
        </div>
        {meta?.diagnostic ? (
          <div role="alert">
            <ClayText variant="body" muted>
              {meta.diagnostic}
            </ClayText>
          </div>
        ) : null}
      </div>
    );
  }
  return <ClayEditor session={pane.session} onOpenPath={onOpenPath} />;
}

export function PaneTree({
  runtime,
  node,
  path = [],
  onFocus,
  onRatio,
  onOpenPath,
  onOpenFile,
  onOpenFolder,
  onLaunchAgent,
  packageUi,
  uiVersion,
}: PaneTreeProps) {
  if (node.kind === "leaf") {
    const pane = runtime.panes.get(node.paneId);
    const active = runtime.tree.activePaneId === node.paneId;
    return (
      <section
        className={`${styles.pane} ${active ? styles.active : ""}`}
        data-testid={`pane-${node.paneId}`}
        data-clay-ds="paneSplitTree.pane"
        aria-label={`Pane ${node.paneId}`}
        onMouseDown={() => onFocus(node.paneId)}
      >
        {pane ? (
          <PaneContent
            pane={pane}
            packageUi={packageUi}
            uiVersion={uiVersion}
            onOpenPath={onOpenPath}
            onOpenFile={onOpenFile}
            onOpenFolder={onOpenFolder}
            onLaunchAgent={onLaunchAgent}
            runtime={runtime}
            effortChord={pane ? effortChordOf(pane) : null}
          />
        ) : (
          <div className={styles.empty}>
            <ClayText variant="body" muted>
              Empty pane
            </ClayText>
          </div>
        )}
      </section>
    );
  }

  const orientation =
    node.orientation === "horizontal" ? "horizontal" : "vertical";
  return (
    <Group orientation={orientation} className={styles.group} data-clay-ds="paneSplitTree.group">
      <Panel
        id={`split-${path.join("") || "root"}-a`}
        defaultSize={`${Math.round(node.ratio * 100)}%`}
        minSize="5%"
        maxSize="95%"
        onResize={(size) => onRatio(path, size.asPercentage / 100)}
      >
        <PaneTree
          runtime={runtime}
          node={node.first}
          path={[...path, "first"]}
          onFocus={onFocus}
          onRatio={onRatio}
          onOpenPath={onOpenPath}
          onOpenFile={onOpenFile}
          onOpenFolder={onOpenFolder}
          onLaunchAgent={onLaunchAgent}
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
      <Separator
        className={styles.separator}
        data-clay-ds="paneSplitTree.handle"
        style={
          orientation === "horizontal"
            ? {
                width:
                  "var(--clay-ds-pane-split-tree-default-handle-rest-border-width, var(--clay-dimension-border-thin, 2px))",
              }
            : {
                height:
                  "var(--clay-ds-pane-split-tree-default-handle-rest-border-width, var(--clay-dimension-border-thin, 2px))",
              }
        }
      />
      <Panel
        id={`split-${path.join("") || "root"}-b`}
        minSize="5%"
        maxSize="95%"
      >
        <PaneTree
          runtime={runtime}
          node={node.second}
          path={[...path, "second"]}
          onFocus={onFocus}
          onRatio={onRatio}
          onOpenPath={onOpenPath}
          onOpenFile={onOpenFile}
          onOpenFolder={onOpenFolder}
          onLaunchAgent={onLaunchAgent}
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
    </Group>
  );
}
