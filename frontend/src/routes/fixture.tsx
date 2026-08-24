import { useEffect, useMemo, useState } from "react";
import { useParams, useSearchParams } from "react-router";

import {
  ClayBadge,
  ClayButton,
  ClayCollapse,
  ClayDivider,
  ClayDropdown,
  ClayKbd,
  ClayList,
  ClayModal,
  ClayText,
  ClayTextField,
} from "../components";
import { lazy, Suspense } from "react";
import { CommandCentre } from "../command-centre/CommandCentre";
import { createDocumentSession } from "../editor/sync/session";
import { createWorkspace } from "../shell/workspace-controller";
import { PackageWorkspace } from "../packages/PackageWorkspace";
import { SettingsPanel } from "../settings/SettingsPanel";
import { installSduiTree } from "../sdui/state";
import type { PackageUiSnapshot } from "../sdui/types";

const WorkspacePanes = lazy(async () => {
  const module = await import("../shell/WorkspacePanes");
  return { default: module.WorkspacePanes };
});

const ClayEditor = lazy(async () => {
  const module = await import("../editor/ClayEditor");
  return { default: module.ClayEditor };
});

const ChatPanel = lazy(async () => {
  const module = await import("../chat/ChatPanel");
  return { default: module.ChatPanel };
});
import type { BootstrapDto } from "../bridge/types";
import styles from "./fixture.module.css";

/**
 * Deterministic visual states for the UI review harness and component tests.
 * Development/test builds only (`import.meta.env.DEV` guard at the router).
 */
export function FixtureRoute() {
  const { fixtureId = "controls" } = useParams();
  const [modalOpen, setModalOpen] = useState(false);
  const [text, setText] = useState("");
  const [selected, setSelected] = useState<string | null>("compact");

  if (fixtureId === "editor") {
    return <EditorFixture />;
  }
  if (fixtureId === "splits") {
    return <SplitsFixture />;
  }
  if (fixtureId === "intelligence") {
    return <IntelligenceFixture />;
  }
  if (fixtureId === "package-ui") {
    return <PackageUiFixture />;
  }
  if (fixtureId === "command-centre") {
    return <CommandCentreFixture />;
  }
  if (fixtureId === "command-centre-empty") {
    return <CommandCentreFixture empty />;
  }
  if (fixtureId === "path-browser") {
    return <CommandCentreFixture pathMode />;
  }
  if (fixtureId === "chat") {
    return <ChatFixture />;
  }
  if (fixtureId === "settings") {
    return (
      <div className={styles.fixture} data-fixture="settings">
        <SettingsPanel uiVersion={9} send={async () => undefined} />
      </div>
    );
  }

  if (fixtureId === "states") {
    return (
      <div className={styles.fixture} data-fixture="states">
        <ClayText variant="title">Loading state</ClayText>
        <div className={styles.emptyState} role="status">
          <ClayText variant="body" muted>
            Loading…
          </ClayText>
        </div>
        <ClayDivider />
        <ClayText variant="title">Empty state</ClayText>
        <div className={styles.emptyState}>
          <div className={styles.stack}>
            <ClayText variant="body" muted>
              Nothing here yet
            </ClayText>
            <ClayButton variant="primary">Open file</ClayButton>
          </div>
        </div>
        <ClayDivider />
        <ClayText variant="title">Error state</ClayText>
        <div className={styles.panel} role="alert">
          <ClayText variant="body">Connection refused.</ClayText>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.fixture} data-fixture="controls">
      <ClayText variant="title">Controls fixture</ClayText>
      <div className={styles.row}>
        <ClayButton>Default</ClayButton>
        <ClayButton variant="muted">Muted</ClayButton>
        <ClayButton variant="primary">Primary</ClayButton>
        <ClayButton variant="danger" isDisabled>
          Disabled
        </ClayButton>
        <ClayBadge>beta</ClayBadge>
        <ClayKbd>Ctrl+S</ClayKbd>
      </div>
      <ClayTextField
        label="Example"
        value={text}
        onChange={setText}
        placeholder="Type here"
        description="Token-driven validation states"
      />
      <ClayDropdown
        label="Density"
        options={[
          { id: "compact", label: "Compact" },
          { id: "default", label: "Default" },
          { id: "spacious", label: "Spacious" },
        ]}
        selectedId={selected}
        onSelect={setSelected}
      />
      <ClayList
        ariaLabel="Fixture list"
        items={[
          { id: "1", title: "Row one", detail: "detail text" },
          { id: "2", title: "Row two" },
        ]}
      />
      <ClayCollapse title="Section" defaultExpanded>
        <ClayText variant="body">Collapsed body content</ClayText>
      </ClayCollapse>
      <ClayButton onPress={() => setModalOpen(true)}>Open modal</ClayButton>
      <ClayModal
        title="Confirm"
        open={modalOpen}
        onClose={() => setModalOpen(false)}
      >
        <ClayText variant="body">
          Modal body with focus trap and Escape handling.
        </ClayText>
      </ClayModal>
    </div>
  );
}

function SplitsFixture() {
  const workspace = useMemo(() => {
    const created = createWorkspace({ send: async () => undefined });
    created.installBootstrap(fixtureBootstrap);
    created.split("horizontal");
    return created;
  }, []);
  return (
    <div
      className={styles.fixture}
      data-fixture="splits"
      style={{ height: "100%" }}
    >
      <Suspense fallback={<ClayText variant="status">Loading panes…</ClayText>}>
        <WorkspacePanes workspace={workspace} />
      </Suspense>
    </div>
  );
}

function CommandCentreFixture({
  empty = false,
  pathMode = false,
}: {
  empty?: boolean;
  pathMode?: boolean;
}) {
  const workspace = useMemo(() => {
    const created = createWorkspace({ send: async () => undefined });
    created.installBootstrap(fixtureBootstrap);
    created.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 1,
        event: {
          kind: "transientMenuSnapshot",
          data: {
            sessionId: "9223372036854775809" as never,
            prompt: pathMode ? "Browse workspace" : "Command Centre",
            query: pathMode ? "workspace/" : "git",
            selectedIndex: 0,
            status: empty
              ? { empty: { message: "No commands match this query" } }
              : "active",
            focusPolicy: "modal",
            origin: "centered",
            items: empty
              ? []
              : pathMode
                ? [
                    {
                      id: "src",
                      label: "src/",
                      detail: "directory",
                      accessibilityLabel: "src directory",
                    },
                    {
                      id: "readme",
                      label: "README.md",
                      detail: "4 KB",
                      accessibilityLabel: "README.md file",
                    },
                  ]
                : [
                    {
                      id: "git.refresh",
                      label: "Refresh Git status",
                      detail: "@clay/git - Ctrl+G",
                      accessibilityLabel: "Refresh Git status",
                    },
                    {
                      id: "runtime.reloadConfiguration",
                      label: "Reload configuration",
                      detail: "Clay - Ctrl+Shift+R",
                      accessibilityLabel: "Reload configuration",
                    },
                    {
                      id: "settings.open",
                      label: "Open settings",
                      detail: "@clay/settings",
                      accessibilityLabel: "Open settings",
                    },
                  ],
          },
        },
      },
    });
    return created;
  }, [empty, pathMode]);
  return (
    <div className={styles.fixture} data-fixture="command-centre">
      <CommandCentre workspace={workspace} />
    </div>
  );
}

function IntelligenceFixture() {
  const session = useMemo(() => {
    const text =
      'fn main() {\n  let message = "hello";\n  println!("{}", message);\n}\n';
    const created = createDocumentSession({ send: async () => undefined });
    created.installInitial({
      ...fixtureBootstrap,
      initialDocument: { ...fixtureBootstrap.initialDocument, text },
      behaviorManifest: {
        ...fixtureBootstrap.behaviorManifest,
        documentFontRole: "monospace",
        editorRules: {
          chrome: {
            gutter: true,
            activeLine: true,
            indentGuides: true,
            bracketMatch: true,
            inlayHints: true,
          },
        },
      },
    } as unknown as BootstrapDto);
    created.store.update({ workspaceRootId: 1, path: "main.rs" });
    const provenance = {
      packageName: "@clay/rust",
      packageVersion: "1",
      packagePrefix: "rust",
    };
    created.handleEnvelope({
      kind: "event",
      data: {
        kind: "decorationSet",
        data: {
          documentId: 1,
          documentVersion: 1,
          packagePrefix: "rust",
          kind: "syntax",
          viewportByteStart: 0,
          viewportByteEnd: text.length,
          spans: [
            {
              byteStart: 0,
              byteEnd: 2,
              kind: "syntax",
              tokenType: "keyword",
              modifiers: 0,
              scope: null,
              fontRole: null,
              priority: 1,
              provenance,
              target: null,
              inlay: null,
            },
            {
              byteStart: 18,
              byteEnd: 25,
              kind: "syntax",
              tokenType: "variable",
              modifiers: 0,
              scope: null,
              fontRole: null,
              priority: 1,
              provenance,
              target: null,
              inlay: null,
            },
            {
              byteStart: 28,
              byteEnd: 35,
              kind: "syntax",
              tokenType: "string",
              modifiers: 0,
              scope: null,
              fontRole: null,
              priority: 1,
              provenance,
              target: null,
              inlay: null,
            },
            {
              byteStart: 18,
              byteEnd: 25,
              kind: "link",
              tokenType: "link",
              modifiers: 4096,
              scope: null,
              fontRole: null,
              priority: 2,
              provenance,
              target: { displayOnly: { text: "message: &str" } },
              inlay: null,
            },
            {
              byteStart: 25,
              byteEnd: 25,
              kind: "inlayHint",
              tokenType: "type",
              modifiers: 0,
              scope: null,
              fontRole: null,
              priority: 1,
              provenance,
              target: null,
              inlay: { label: ": &str", placement: "after" },
            },
          ],
        },
      },
    });
    created.handleEnvelope({
      kind: "event",
      data: {
        kind: "diagnosticSet",
        data: {
          documentId: 1,
          documentVersion: 1,
          viewportByteStart: 0,
          viewportByteEnd: text.length,
          source: "rust-analyzer",
          provenance,
          spans: [
            {
              byteStart: 18,
              byteEnd: 25,
              severity: "warning",
              code: "unused",
              message: "Example warning",
              source: "rust-analyzer",
              provenance,
            },
          ],
        },
      },
    });
    created.handleEnvelope({
      kind: "event",
      data: {
        kind: "foldingRangeSet",
        data: {
          documentId: 1,
          documentVersion: 1,
          packagePrefix: "rust",
          ranges: [
            {
              byteStart: 0,
              byteEnd: text.length - 1,
              label: "function",
              provenance,
            },
          ],
        },
      },
    });
    return created;
  }, []);
  return (
    <div className={styles.fixture} data-fixture="intelligence">
      <Suspense
        fallback={<ClayText variant="status">Loading intelligence…</ClayText>}
      >
        <ClayEditor session={session} />
      </Suspense>
    </div>
  );
}

function PackageUiFixture() {
  const session = useMemo(() => {
    const created = createDocumentSession({ send: async () => undefined });
    created.installInitial(fixtureBootstrap);
    created.store.update({ workspaceRootId: 1, path: "notes.md" });
    return created;
  }, []);
  const sdui = useMemo(
    () =>
      installSduiTree({
        uiVersion: 4,
        rootId: 1,
        nodes: [
          { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
          { id: 2, kind: { panel: { title: "Workspace", children: [3] } } },
          { id: 3, kind: { label: { text: "notes.md" } } },
          {
            id: 4,
            kind: {
              editorView: { binding: { documentId: 1, expectedVersion: 1 } },
            },
          },
        ],
      }),
    [],
  );
  return (
    <div className={styles.packageFixture} data-fixture="package-ui">
      <PackageWorkspace
        sdui={sdui}
        packageUi={packageFixtureSnapshot}
        send={async () => undefined}
        editorSlot={
          <Suspense
            fallback={<ClayText variant="status">Loading editor…</ClayText>}
          >
            <ClayEditor session={session} />
          </Suspense>
        }
      />
    </div>
  );
}

function EditorFixture() {
  const session = useMemo(() => {
    const created = createDocumentSession({
      send: async () => undefined,
    });
    created.installInitial(fixtureBootstrap);
    created.store.update({ workspaceRootId: 1, path: "notes.md" });
    return created;
  }, []);
  return (
    <div className={styles.fixture} data-fixture="editor">
      <Suspense
        fallback={<ClayText variant="status">Loading editor…</ClayText>}
      >
        <ClayEditor session={session} />
      </Suspense>
    </div>
  );
}

const packageFixtureSnapshot: PackageUiSnapshot = {
  version: 4,
  emptyTab: null,
  overlays: [],
  inputRoutes: [],
  components: [
    {
      id: "markdown.status.mode",
      actionTargets: [],
      provenance: {
        packageName: "@clay/markdown",
        packageVersion: "0.1.0",
        apiPrefix: "markdown",
        trustDomain: "trusted",
      },
      component: {
        id: "markdown.status.mode",
        kind: "statusItem",
        text: "Markdown mode",
      },
    },
  ],
  panels: [
    {
      id: "settings.surface",
      slot: "right",
      visibility: "visible",
      actionTargets: ["settings.setTheme", "settings.close"],
      provenance: {
        packageName: "@clay/settings",
        packageVersion: "0.1.0",
        apiPrefix: "settings",
        trustDomain: "trusted",
      },
      component: {
        id: "settings.root",
        kind: "panel",
        title: "Settings",
        children: [
          { id: "settings.label.theme", kind: "label", text: "Theme" },
          {
            id: "settings.dropdown.theme",
            kind: "dropdown",
            title: "Theme",
            items: [
              {
                id: "light",
                label: "Modus Operandi",
                action: { commandId: "settings.setTheme" },
              },
              {
                id: "dark",
                label: "Modus Vivendi",
                action: { commandId: "settings.setTheme" },
              },
            ],
          },
          {
            id: "settings.section.typography",
            kind: "collapse",
            title: "Typography",
            children: [
              {
                id: "settings.input.ui",
                kind: "textInput",
                title: "UI families",
              },
            ],
          },
          {
            id: "settings.close",
            kind: "button",
            label: "Close",
            action: { commandId: "settings.close" },
          },
        ],
      },
    },
  ],
};

const fixtureBootstrap = {
  clientId: 1,
  protocolVersion: 26,
  endpoint: "fixture",
  generation: 1,
  initialDocument: {
    documentId: 1,
    version: 1,
    text: "fixture document\n",
    access: { editable: { leaseId: 1 } },
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: {
    manifestId: "fixture",
    behaviorVersion: 1,
    commands: [],
    keymaps: [],
  },
  activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
  activeTypography: {
    revision: 1,
    monospace: {
      families: ["monospace"],
      size: 13,
      ligatures: { enableStandard: true },
    },
    proportional: {
      families: ["serif"],
      size: 13,
      ligatures: { enableStandard: true },
    },
    ui: {
      families: ["system-ui"],
      size: 13,
      ligatures: { enableStandard: true },
    },
    hierarchy: {
      display: 1.5,
      title: 1,
      section: 1,
      body: 1,
      status: 1,
      detail: 0.8,
      caption: 0.75,
    },
  },
} as unknown as BootstrapDto;

function ChatFixture() {
  const [params] = useSearchParams();
  const state = params.get("state") ?? "landing";
  useEffect(() => {
    let cancelled = false;
    void import("../agent/state").then(({ chatAgent }) => {
      if (cancelled) return;
      seedChatFixture(chatAgent, state);
    });
    return () => {
      cancelled = true;
    };
  }, [state]);
  return (
    <div className={styles.packageFixture} data-fixture={`chat-${state}`}>
      <Suspense fallback={<ClayText variant="status">Loading chat…</ClayText>}>
        <ChatPanel surface={chatFixtureSurface} uiVersion={4} />
      </Suspense>
    </div>
  );
}

function seedChatFixture(
  chatAgent: {
    seedForDev(input: {
      messages?: unknown;
      state?: Record<string, unknown>;
      streaming?: boolean;
      statusText?: string | null;
    }): void;
  },
  state: string,
) {
  if (state === "landing") {
    chatAgent.seedForDev({
      messages: [],
      state: {},
      streaming: false,
      statusText: null,
    });
    return;
  }
  if (state === "conversation") {
    chatAgent.seedForDev({
      messages: [
        { id: "f0", role: "user", content: "Summarize notes.md" },
        {
          id: "f1",
          role: "reasoning",
          content: "The user wants the key points.",
        },
        {
          id: "f2",
          role: "assistant",
          content: "Three key points stand out.",
        },
        {
          id: "f3",
          role: "assistant",
          content: "42 tokens",
          metadata: { clayKind: "usage" },
        },
      ],
      state: { provider: "mock", model: "mock-mini", profile: "chat" },
      streaming: false,
      statusText: null,
    });
    return;
  }
  if (state === "streaming") {
    chatAgent.seedForDev({
      messages: [
        {
          id: "s0",
          role: "user",
          content: "Write a haiku about editors",
        },
        {
          id: "s1",
          role: "assistant",
          content: "Cursor blinks softly \u2014",
        },
      ],
      state: { provider: "mock", model: "mock-mini" },
      streaming: true,
    });
    return;
  }
  if (state === "error") {
    chatAgent.seedForDev({
      messages: [
        { id: "e0", role: "user", content: "List files" },
        {
          id: "e1",
          role: "assistant",
          content: "provider unreachable",
          metadata: { clayKind: "error" },
        },
      ],
      state: { provider: "mock", model: "mock-mini" },
      streaming: false,
      statusText: null,
    });
  }
}

/** Mirrors the @clay/chat declared landing (packages/chat/package.json). */
const chatFixtureSurface = {
  id: "chat.entry",
  actionTargets: [
    "agent.clientOpenAgentPicker",
    "agent.clientOpenProviderPicker",
    "agent.clientOpenModelPicker",
    "chat.submit",
    "chat.cancel",
    "documents.clientOpenFileDialog",
    "workspace.clientOpenFolderDialog",
  ],
  provenance: {
    packageName: "@clay/chat",
    packageVersion: "0.1.0",
    apiPrefix: "chat",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "chat.root",
    title: "Chat",
    children: [
      {
        kind: "label" as const,
        id: "chat.greeting",
        text: "What do you want to do today?",
      },
      {
        kind: "label" as const,
        id: "chat.providerHint",
        text: "Configure a provider to start chatting.",
      },
      {
        kind: "button" as const,
        id: "chat.button.agent",
        label: "Agent",
        action: { commandId: "agent.clientOpenAgentPicker" },
      },
      {
        kind: "button" as const,
        id: "chat.button.provider",
        label: "Provider",
        action: { commandId: "agent.clientOpenProviderPicker" },
      },
      {
        kind: "button" as const,
        id: "chat.button.model",
        label: "Model",
        action: { commandId: "agent.clientOpenModelPicker" },
      },
      {
        kind: "button" as const,
        id: "chat.button.openFile",
        label: "Open File",
        action: { commandId: "documents.clientOpenFileDialog" },
      },
      {
        kind: "button" as const,
        id: "chat.button.openFolder",
        label: "Open Folder",
        action: { commandId: "workspace.clientOpenFolderDialog" },
      },
    ],
  },
};
