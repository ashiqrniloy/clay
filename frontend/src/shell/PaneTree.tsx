import { lazy, Suspense, useSyncExternalStore } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";

import { ClayButton, ClayText } from "../components";
import type { PackageSurface, PackageUiSnapshot } from "../sdui/types";
import type { TabAgent } from "./tab-store";
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

const LauncherPanel = lazy(async () => {
  const module = await import("../launcher/LauncherPanel");
  return { default: module.LauncherPanel };
});

/** First-party pane surfaces the host renders itself: exact package
 *  provenance *and* trusted-domain status, never a name match alone. Any
 *  other package's contribution (pane or empty-tab) renders through the
 *  generic SDUI view. */
const HOST_RENDERED_SURFACES: Record<string, string> = {
  "@clay/coding-agent": "coding-agent",
  "@clay/launcher": "launcher",
};

/** Which host panel renders a package's surface, if any. */
export function hostRenderedSurface(surface: PackageSurface): string | null {
  if (surface.provenance.trustDomain !== "trusted") return null;
  return HOST_RENDERED_SURFACES[surface.provenance.packageName] ?? null;
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
  /** Attach a server-listed agent to this tab (the launcher's agent pane). */
  onPickAgent: (agent: TabAgent) => void;
  /** Launcher: pick a server-listed workspace root for this tab. */
  onOpenWorkspace: (root: string) => Promise<void> | void;
  /** Render the launcher for an empty pane (uncommitted tab only). */
  showLauncher: boolean;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
}

/** Plan 109 I4: the coding-agent surface's effort-cycle chord, read from
 *  the pane session's behavior manifest so `bindKey` overrides apply.
 *  Default `Shift+Tab` when unbound. */
export function effortChordOf(pane: {
  session: { behaviorManifest(): { keymaps?: unknown } };
}): EffortChord | null {
  const keymaps =
    (pane.session.behaviorManifest().keymaps as
      Array<{ commandId: string; sequence?: ServerKeyStroke[] }> | undefined) ??
    [];
  const binding = keymaps.find(
    (entry) => entry.commandId === "coding-agent.clientCycleEffort",
  );
  const stroke = binding?.sequence?.at(-1);
  if (!stroke)
    return { shift: true, ctrl: false, alt: false, meta: false, key: "Tab" };
  const raw =
    typeof stroke.key === "string" ? stroke.key : stroke.key.character;
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
  onPickAgent,
  onOpenWorkspace,
  showLauncher,
}: {
  pane: PaneRecord;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
  onOpenPath: (path: string) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onPickAgent: (agent: TabAgent) => void;
  onOpenWorkspace: (root: string) => Promise<void> | void;
  showLauncher: boolean;
  runtime: TabRuntime;
  /** Plan 109 I4: effective effort-cycle chord from the manifest. */
  effortChord: EffortChord | null;
}) {
  const meta = useSyncExternalStore(
    pane.session.store.subscribe,
    pane.session.store.get,
  );
  const empty = !meta?.path && pane.session.snapshotDoc().length === 0;
  // The launcher is the landing of an *uncommitted* tab (plan 118 task 33): a
  // tab that has picked neither a folder nor an agent. With either half picked
  // the pane is an ordinary empty editor — the tab now has a context, and the
  // launcher's rows would be a second, stale picker.
  if (empty && showLauncher && packageUi?.emptyTab) {
    // Plan 118 Part D: the window's landing is a package contribution. The
    // bundled launcher renders as the host panel for its trusted provenance;
    // any other empty-tab contribution renders through the generic SDUI view.
    // With no contribution installed, the core fallback below stays Open
    // File / Open Folder only — no product name lives in core.
    const emptyTab = packageUi.emptyTab;
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
        {hostRenderedSurface(emptyTab) === "launcher" ? (
          <LauncherPanel
            session={pane.session}
            onOpenWorkspace={onOpenWorkspace}
            onOpenFolder={onOpenFolder}
            onPickAgent={onPickAgent}
          />
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
  onPickAgent,
  onOpenWorkspace,
  showLauncher,
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
            onPickAgent={onPickAgent}
            onOpenWorkspace={onOpenWorkspace}
            showLauncher={showLauncher}
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
    <Group
      orientation={orientation}
      className={styles.group}
      data-clay-ds="paneSplitTree.group"
      // The visible handle is one hairline; the pointer target is wider so a
      // split stays grabbable without drawing a thick divider (DESIGN.md §13.3).
      resizeTargetMinimumSize={{ coarse: 24, fine: 8 }}
    >
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
          onPickAgent={onPickAgent}
          onOpenWorkspace={onOpenWorkspace}
          showLauncher={showLauncher}
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
      <Separator
        className={styles.separator}
        data-clay-ds="paneSplitTree.handle"
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
          onPickAgent={onPickAgent}
          onOpenWorkspace={onOpenWorkspace}
          showLauncher={showLauncher}
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
    </Group>
  );
}
