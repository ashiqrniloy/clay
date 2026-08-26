import { lazy, Suspense, useSyncExternalStore } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";

import { ClayButton, ClayText } from "../components";
import type { PackageUiSnapshot } from "../sdui/types";
import { ClayEditor } from "../editor/ClayEditor";
import type { SplitNode } from "./split-tree";
import type { PaneRecord, TabRuntime } from "./workspace-controller";

import styles from "./pane-tree.module.css";

const PackageSurfaceView = lazy(async () => {
  const module = await import("../sdui/registry");
  return { default: module.PackageSurfaceView };
});

const ChatPanel = lazy(async () => {
  const module = await import("../chat/ChatPanel");
  return { default: module.ChatPanel };
});

export interface PaneTreeProps {
  runtime: TabRuntime;
  node: SplitNode;
  path?: Array<"first" | "second">;
  onFocus: (paneId: number) => void;
  onRatio: (path: Array<"first" | "second">, ratio: number) => void;
  onOpenPath: (path: string) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
}

function PaneContent({
  pane,
  packageUi,
  uiVersion,
  onOpenPath,
  onOpenFile,
  onOpenFolder,
}: {
  pane: PaneRecord;
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
  onOpenPath: (path: string) => void;
  onOpenFile: () => void;
  onOpenFolder: () => void;
}) {
  const meta = useSyncExternalStore(
    pane.session.store.subscribe,
    pane.session.store.get,
  );
  const empty = !meta?.path && pane.session.snapshotDoc().length === 0;
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
    <Group orientation={orientation} className={styles.group}>
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
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
      <Separator
        className={styles.separator}
        style={
          orientation === "horizontal"
            ? { width: "var(--clay-dimension-border-thin, 2px)" }
            : { height: "var(--clay-dimension-border-thin, 2px)" }
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
          packageUi={packageUi}
          uiVersion={uiVersion}
        />
      </Panel>
    </Group>
  );
}
