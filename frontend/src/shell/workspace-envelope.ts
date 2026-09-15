// Server bridge envelope routing for the per-window workspace controller:
// runtime snapshots, registry, and routed pane events -> tab runtime state.
// Extracted from the createWorkspace closure (2026-08-31 review P2-2) with
// an explicit context seam; the controller keeps sole ownership of
// tabs/panes state.

import type {
  BridgeEnvelope,
  RuntimeDiagnosticDto,
  TransientMenuSnapshotDto,
} from "../bridge/types";
import {
  applySduiUpdate,
  installPackageUi,
  installSduiTree,
} from "../sdui/state";
import type { SduiTree, SduiTreeUpdate } from "../sdui/types";
import { applyRegistry, type TabStore } from "./tab-store";
import { detached } from "../lib/detached";
import type {
  PaneRecord,
  TabRuntime,
  WorkspaceAdapters,
} from "./workspace-controller";

export interface EnvelopeContext {
  adapters: WorkspaceAdapters;
  runtimes: Map<number, TabRuntime>;
  tabs: TabStore;
  registryRootsByClient: Map<number, number>;
  registryTabsByClient: Map<number, number>;
  notify: () => void;
  deliverRootId: (runtime: TabRuntime, rootId: number) => void;
  ensurePane: (runtime: TabRuntime, paneId: number) => PaneRecord;
  dispatchClientCommand: (runtime: TabRuntime, commandId: string) => void;
}

export function handleEnvelope(ctx: EnvelopeContext, envelope: BridgeEnvelope) {
  const {
    adapters,
    deliverRootId,
    dispatchClientCommand,
    ensurePane,
    notify,
    registryRootsByClient,
    registryTabsByClient,
    runtimes,
    tabs,
  } = ctx;
  if (envelope.kind === "runtimeSnapshot") {
    const runtime = runtimes.get(envelope.data.clientId);
    const snapshot = envelope.data.snapshot;
    if (
      !runtime ||
      snapshot.runtimeGenerationId <= runtime.ui.runtimeGeneration
    )
      return;
    runtime.ui = {
      runtimeGeneration: snapshot.runtimeGenerationId,
      sdui: installSduiTree(snapshot.sduiTree),
      packageUi: installPackageUi(snapshot.packageUi),
    };
    runtime.diagnostic = snapshot.diagnostics.at(-1) ?? null;
    if (
      !snapshot.packageUi.panels.some(
        (panel) => panel.provenance.packageName === "@clay/settings",
      )
    )
      runtime.settingsOpen = false;
    for (const pane of runtime.panes.values()) {
      const document = snapshot.documents.find(
        (candidate) =>
          Number(candidate.documentId) === pane.session.store.get()?.documentId,
      );
      const manifest =
        (document?.behaviorManifest as Record<string, unknown> | null) ??
        snapshot.behaviorManifest;
      pane.session.handleEnvelope({
        kind: "event",
        data: {
          kind: "behaviorManifestInstalled",
          data: {
            behaviorVersion: Number(manifest.behaviorVersion ?? 0),
            manifest,
          },
        },
      });
      if (document?.initialDecorations) {
        pane.session.handleEnvelope({
          kind: "event",
          data: {
            kind: "decorationSet",
            data: document.initialDecorations,
          },
        });
      }
      if (document?.initialDiagnostics) {
        pane.session.handleEnvelope({
          kind: "event",
          data: {
            kind: "diagnosticSet",
            data: document.initialDiagnostics,
          },
        });
      }
    }
    detached(
      adapters.send(
        JSON.stringify({
          family: "runtimeGenerationInstalled",
          payload: {
            clientId: 0,
            runtimeGenerationId: snapshot.runtimeGenerationId,
          },
        }),
        runtime.tabId ?? undefined,
      ),
    );
    notify();
    return;
  }
  if (envelope.kind === "disconnected") {
    const clientId = envelope.data.clientId;
    if (clientId != null && runtimes.has(clientId)) {
      const next = {
        ...tabs.get(),
        tabs: tabs
          .get()
          .tabs.map((tab) =>
            tab.clientId === clientId ? { ...tab, disconnected: true } : tab,
          ),
      };
      tabs.set(next);
      notify();
    }
    return;
  }
  if (envelope.kind !== "event" && envelope.kind !== "routed") return;
  const routed =
    envelope.kind === "routed"
      ? envelope.data
      : { clientId: null, tabId: null, event: envelope.data };
  if (
    routed.event &&
    (routed.event as { kind?: string }).kind === "tabRegistry"
  ) {
    const data = (routed.event as { data: Parameters<typeof applyRegistry>[1] })
      .data;
    tabs.set(applyRegistry(tabs.get(), data));
    // The registry carries the server root id for each tab. The initial
    // document's own status lookup can fail ("unknownDocument") before
    // the tab binding registers it, so this is the authoritative moment
    // pane sessions learn the root id and any restore-queued open fires.
    const registryRoots = new Map(
      (
        data.tabs as Array<{
          clientId: number;
          workspaceRootId?: number;
        }>
      ).map((entry) => [entry.clientId, entry.workspaceRootId]),
    );
    for (const [clientId, rootId] of registryRoots) {
      if (rootId != null) registryRootsByClient.set(clientId, rootId);
    }
    for (const tab of tabs.get().tabs) {
      const runtime = runtimes.get(tab.clientId);
      if (runtime) {
        runtime.tabId = tab.tabId;
        runtime.sessionRoot = tab.workspaceRoot || runtime.sessionRoot;
        const rootId = registryRoots.get(tab.clientId);
        if (rootId != null) {
          runtime.workspaceRootId = rootId;
          deliverRootId(runtime, rootId);
        }
      }
    }
    // Remember tab ids even for clients whose runtime has not mounted yet
    // (the registry broadcast races the bootstrap); mountRuntime adopts it.
    for (const tab of tabs.get().tabs) {
      if (tab.tabId != null) registryTabsByClient.set(tab.clientId, tab.tabId);
    }
    notify();
    return;
  }
  const eventEnvelope =
    envelope.kind === "routed"
      ? { kind: "event" as const, data: routed.event }
      : envelope;
  const owners =
    routed.clientId != null
      ? [runtimes.get(routed.clientId)].filter(Boolean)
      : [...runtimes.values()];
  const event = routed.event as {
    kind?: string;
    data?: Record<string, unknown>;
  };
  if (!owners.length) return;
  for (const runtime of owners) {
    if (!runtime) continue;
    if (event.kind === "sduiSnapshot") {
      const tree = event.data?.tree as SduiTree | undefined;
      if (tree) runtime.ui = { ...runtime.ui, sdui: installSduiTree(tree) };
      notify();
      continue;
    }
    if (event.kind === "sduiUpdate") {
      const update = event.data as unknown as SduiTreeUpdate;
      runtime.ui = {
        ...runtime.ui,
        sdui: applySduiUpdate(runtime.ui.sdui, update),
      };
      notify();
      continue;
    }
    if (event.kind === "transientMenuSnapshot") {
      runtime.menu = event.data as unknown as TransientMenuSnapshotDto;
      notify();
      continue;
    }
    if (event.kind === "transientMenuClosed") {
      const sessionId = String(event.data?.sessionId ?? "");
      if (runtime.menu?.sessionId === sessionId) runtime.menu = null;
      notify();
      continue;
    }
    if (event.kind === "runtimeDiagnostic") {
      runtime.diagnostic = event.data as unknown as RuntimeDiagnosticDto;
      notify();
      continue;
    }
    if (event.kind === "fileOperationFailed") {
      // Server-rejected file operations (too large, unauthorized, missing)
      // must surface in shell status; the pane session store alone renders
      // nothing for an empty pane.
      runtime.diagnostic = {
        severity: "error",
        code: String(event.data?.code ?? "file.error"),
        message: String(event.data?.message ?? "File operation failed"),
      };
      notify();
      continue;
    }
    if (event.kind === "documentOpened") {
      // A successful open clears stale file-operation errors (the
      // bootstrap placeholder's "unknown workspace document" status
      // lookup and any failed open attempt), then falls through to the
      // normal pane routing below. dialog./server. errors stay.
      const code = runtime.diagnostic?.code ?? "";
      if (!code.startsWith("dialog.") && !code.startsWith("server.")) {
        runtime.diagnostic = null;
      }
      notify();
    }
    if (event.kind === "serverError") {
      runtime.diagnostic = {
        severity: "error",
        code: String(event.data?.code ?? "server.error"),
        message: String(event.data?.message ?? "Server request failed"),
      };
      notify();
      continue;
    }
    if (event.kind === "shellClientCommandRequest") {
      const commandId = event.data?.commandId;
      if (typeof commandId === "string")
        dispatchClientCommand(runtime, commandId);
      continue;
    }
    const documentId = eventDocumentId(event);
    const matching =
      documentId == null
        ? []
        : [...runtime.panes.values()].filter(
            (pane) => pane.session.store.get()?.documentId === documentId,
          );
    // An unclaimed open reply belongs to whichever pane's OpenDocument is
    // still in flight (restores open several panes at once, and restored
    // placeholders can share a document id with early real ids); the
    // documentId match is next, then the active pane fallback.
    const openReplyPath =
      event.kind === "documentOpened"
        ? String(
            (event.data as { metadata?: { path?: unknown } } | undefined)
              ?.metadata?.path ?? "",
          )
        : null;
    const awaiting =
      openReplyPath == null || openReplyPath === ""
        ? undefined
        : [...runtime.panes.values()].find(
            (pane) => pane.session.inFlightOpenPath() === openReplyPath,
          );
    const targets =
      event.kind === "documentOpened"
        ? [
            awaiting ??
              matching[0] ??
              ensurePane(runtime, runtime.tree.activePaneId),
          ]
        : matching.length > 0
          ? matching
          : [...runtime.panes.values()];
    for (const pane of targets) pane.session.handleEnvelope(eventEnvelope);
  }
}

function eventDocumentId(event: {
  kind?: string;
  data?: Record<string, unknown>;
}): number | null {
  const data = event.data;
  if (!data) return null;
  if (typeof data.documentId === "number") return data.documentId;
  const metadata = data.metadata;
  if (
    metadata &&
    typeof metadata === "object" &&
    "documentId" in metadata &&
    typeof metadata.documentId === "number"
  ) {
    return metadata.documentId;
  }
  if (event.kind === "decorationBatch" && Array.isArray(data)) {
    const first = data[0] as { documentId?: unknown } | undefined;
    return typeof first?.documentId === "number" ? first.documentId : null;
  }
  return null;
}
