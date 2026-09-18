import { useEffect, useMemo, useRef, useState } from "react";
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
import { createDocumentSession } from "../editor/sync/session";
import { WorkspaceView } from "./workspace";
import { createWorkspace } from "../shell/workspace-controller";
import { PackageWorkspace } from "../packages/PackageWorkspace";
import { AgentSettingsPanel } from "../agent-settings/AgentSettingsPanel";
import { createAgentSession, type AgentSessionModule } from "../agent/state";
import type { ComposerPalette } from "../coding-agent/Composer";
import type { AgentSettingsFileInfo } from "../agent-settings/AgentSettingsPanel";
import { themeStore } from "../state/stores";
import { installSduiTree } from "../sdui/state";
import type { ThemeSnapshot } from "../theme/types";
import type { PackageUiSnapshot, UiChoicesSnapshot } from "../sdui/types";
import { behaviorManifestFixture } from "../test/contract-fixtures";

// Plan 104 source-independence guard: package specifiers live in `const`
// declarations only. Plan 118: `@clay/core` + the shipped design system are the
// whole choice set (the removed Neobrutal/Glass packages are gone).
const SHIPPED_THEME = "@clay/theme-modus-operandi";
const SHIPPED_DESIGN_SYSTEM = "@clay/design-instrument";
const SHIPPED_UI_CHOICES: UiChoicesSnapshot = {
  themes: [
    { specifier: SHIPPED_THEME },
    { specifier: "@clay/theme-modus-vivendi" },
    { specifier: "@clay/theme-gruvbox-material-light" },
    { specifier: "@clay/theme-gruvbox-material-dark" },
  ],
  designSystems: [
    { specifier: "@clay/core", displayName: "Core baseline" },
    { specifier: SHIPPED_DESIGN_SYSTEM, displayName: "Quiet Instrument" },
  ],
  appearance: "dark",
};

const WorkspacePanes = lazy(async () => {
  const module = await import("../shell/WorkspacePanes");
  return { default: module.WorkspacePanes };
});

const ClayEditor = lazy(async () => {
  const module = await import("../editor/ClayEditor");
  return { default: module.ClayEditor };
});

import type { BootstrapDto, TransientMenuItemDto } from "../bridge/types";
import styles from "./fixture.module.css";
import panesStyles from "../shell/workspace-panes.module.css";

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
  if (fixtureId === "document-loading") {
    return <DocumentLoadingFixture />;
  }
  if (fixtureId === "document-budget-error") {
    return <DocumentErrorFixture kind="budget" />;
  }
  if (fixtureId === "document-binary-error") {
    return <DocumentErrorFixture kind="binary" />;
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
  if (fixtureId === "coding-agent") {
    return <CodingAgentFixture />;
  }
  if (fixtureId === "settings") {
    return <SettingsFixture />;
  }
  if (fixtureId === "agent-settings") {
    return <AgentSettingsFixture />;
  }

  if (fixtureId === "workspace-sidebar") {
    return <WorkspaceSidebarFixture />;
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
      <ClayTextField
        label="Workspace"
        value={text}
        onChange={setText}
        validationState="error"
        errorMessage="Value is not a valid workspace path"
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
        footer={
          <>
            <ClayButton variant="muted" onPress={() => setModalOpen(false)}>
              Cancel
            </ClayButton>
            <ClayButton variant="primary" onPress={() => setModalOpen(false)}>
              Confirm
            </ClayButton>
          </>
        }
      >
        <ClayText variant="body">
          Modal body with focus trap and Escape handling.
        </ClayText>
      </ClayModal>
    </div>
  );
}

/** The host-rendered sidebar region as the server delivers it (DEV harness):
 * a flush stack of title + filterable list beside the editor. The real tree is
 * built by `src/shell/file_browser.rs` and asserted there; this fixture exists
 * so the browser path (renderer → ClayList → filter → geometry) can be
 * measured (plan 118 task E1). */
function WorkspaceSidebarFixture() {
  const workspace = useMemo(() => {
    const created = createWorkspace({ send: async () => undefined });
    created.installBootstrap(fixtureBootstrap);
    created.handleEnvelope({
      kind: "runtimeSnapshot",
      data: {
        clientId: 1,
        tabId: 1,
        snapshot: {
          runtimeGenerationId: 1,
          behaviorManifest: fixtureBootstrap.behaviorManifest,
          activeTheme: fixtureBootstrap.activeTheme,
          activeTypography: fixtureBootstrap.activeTypography,
          activeDesignSystem: (
            fixtureBootstrap as unknown as { activeDesignSystem: never }
          ).activeDesignSystem,
          sduiTree: {
            uiVersion: 9,
            rootId: 1,
            nodes: [
              {
                id: 1,
                kind: { flex: { direction: "row", children: [2, 4] } },
              },
              {
                id: 2,
                kind: { stack: { children: [7, 5] } },
                size: "dimension.sidebar.default",
              },
              {
                id: 3,
                kind: { label: { text: "Workspace · clay", icon: null } },
              },
              {
                id: 6,
                kind: {
                  button: {
                    label: "Hide file browser",
                    icon: "disclosure.right",
                    action: {
                      commandId: "workspace.toggleFileBrowser",
                      source: { button: { nodeId: 6 } },
                      arguments: [],
                    },
                  },
                },
              },
              {
                id: 7,
                kind: { flex: { direction: "row", children: [3, 6] } },
              },
              {
                id: 5,
                kind: {
                  list: {
                    items: [
                      {
                        id: "src/alpha.md",
                        label: "alpha.md",
                        detail: "src",
                        icon: null,
                        action: null,
                      },
                      {
                        id: "src/beta.md",
                        label: "beta.md",
                        detail: "src",
                        icon: null,
                        action: null,
                      },
                      {
                        id: "docs/gamma.md",
                        label: "gamma.md",
                        detail: "docs",
                        icon: null,
                        action: null,
                      },
                    ],
                    filter: { placeholder: "Filter files", shortcut: "/" },
                  },
                },
              },
              {
                id: 4,
                kind: {
                  editorView: {
                    binding: { documentId: 1, expectedVersion: 1 },
                  },
                },
              },
            ],
          },
          packageUi: {
            version: 1,
            emptyTab: null,
            surfaces: [],
            panels: [],
            overlays: [],
            components: [],
            inputRoutes: [],
          },
          uiChoices: { themes: [], designSystems: [] },
          documents: [],
          diagnostics: [],
        },
      },
    });
    return created;
  }, []);
  return (
    <div className={styles.packageFixture} data-fixture="workspace-sidebar">
      {/* The fixture renders the app's own working-area composition, not a
          second shell: `WorkspaceView` draws the grid and `WorkspacePanes`
          places the sidebar rail, the views, the lane and the veil in it. */}
      <WorkspaceView session={null}>
        <Suspense
          fallback={<ClayText variant="status">Loading panes…</ClayText>}
        >
          <WorkspacePanes workspace={workspace} />
        </Suspense>
      </WorkspaceView>
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

/** A palette for fixtures with no workspace of their own (the agent surfaces):
 *  no session, inert intents. */
const IDLE_PALETTE: ComposerPalette = {
  menu: null,
  request: () => undefined,
  query: () => undefined,
  move: () => undefined,
  activate: () => undefined,
  back: () => undefined,
  cancel: () => undefined,
};

/**
 * The picker stages the server's Agent Picker produces (plan 125): each one is
 * a `commandPalette` session whose *mode* decides how the sheet renders
 * (protocol v32). The rows, prompts, and modes below mirror
 * `src/server/agent_picker.rs` verbatim, so the fixture draws the shipped
 * `CommandPalette` for the stage the live daemon would send.
 */
type PaletteStageId =
  "providers" | "auth" | "secret" | "url" | "oauth" | "models" | "sessions";

interface PaletteStageSeed {
  prompt: string;
  mode?: string;
  items: TransientMenuItemDto[];
  query?: string;
}

/** One `Model` picker with more rows than the sheet can hold, so the fixture
 *  pins the height cap and internal scrolling against a real catalogue size
 *  (`TRANSIENT_MENU_MAX_ITEMS` is 256; 40 rows already overflow the sheet). */
function modelStageRows(count: number): TransientMenuItemDto[] {
  return Array.from({ length: count }, (_unused, index) => ({
    id: `model:mock/model-${index + 1}`,
    label: `Mock Model ${index + 1}`,
    detail: "mock",
    accessibilityLabel: `Mock Model ${index + 1} mock`,
  }));
}

function paletteStage(stage: PaletteStageId): PaletteStageSeed {
  if (stage === "providers") {
    return {
      prompt: "Configure provider",
      mode: "picker",
      items: [
        {
          id: "provider:mock",
          label: "Mock Provider",
          detail: "mock",
          accessibilityLabel: "Mock Provider mock",
        },
        {
          id: "provider:anthropic",
          label: "Anthropic",
          detail: "anthropic",
          accessibilityLabel: "Anthropic anthropic",
        },
        {
          id: "configure",
          label: "Configure provider…",
          detail: "API key, OAuth, or base URL",
          accessibilityLabel: "Configure provider… API key, OAuth, or base URL",
        },
      ],
    };
  }
  if (stage === "auth") {
    return {
      prompt: "Choose sign-in method",
      mode: "picker",
      items: [
        {
          id: "auth:api_key",
          label: "API key",
          detail: "api_key",
          accessibilityLabel: "API key api_key",
        },
        {
          id: "auth:oauth",
          label: "OAuth",
          detail: "oauth",
          accessibilityLabel: "OAuth oauth",
        },
        {
          id: "auth:url",
          label: "API base URL",
          detail: "url",
          accessibilityLabel: "API base URL url",
        },
      ],
    };
  }
  if (stage === "secret") {
    return {
      prompt: "API key (hidden)",
      mode: "secret",
      items: [
        {
          id: "store_secret",
          label: "Store API key",
          detail: "Value is hidden. Enter stores it.",
          accessibilityLabel: "Store API key Value is hidden. Enter stores it.",
        },
      ],
    };
  }
  if (stage === "url") {
    return {
      prompt: "API base URL",
      mode: "url",
      items: [
        {
          id: "store_url",
          label: "Save base URL",
          detail: "OpenAI-compatible endpoint",
          accessibilityLabel: "Save base URL OpenAI-compatible endpoint",
        },
      ],
    };
  }
  if (stage === "oauth") {
    return {
      prompt: "Authorize provider",
      mode: "oauth",
      items: [
        {
          id: "poll_oauth",
          label: "Device code WDJB-MJHT",
          detail: "https://example.test/device",
          accessibilityLabel:
            "Device code WDJB-MJHT https://example.test/device",
        },
        {
          id: "open_oauth_url",
          label: "Open in browser",
          detail: "https://example.test/device",
          accessibilityLabel:
            "Open the authorization URL in the default browser",
        },
        {
          id: "copy_oauth_url",
          label: "Copy URL",
          detail: "https://example.test/device",
          accessibilityLabel: "Copy the authorization URL to the clipboard",
        },
      ],
    };
  }
  if (stage === "models") {
    return { prompt: "Models", mode: "picker", items: modelStageRows(40) };
  }
  return {
    prompt: "Sessions",
    mode: "picker",
    items: [
      {
        id: "session:1",
        label: "How does the resume list",
        detail: "2026-09-10 22:17",
        // The row states what only its own kind does: `Alt+↵` deletes this
        // session instead of resuming it (the sheet's secondary activation).
        bindings: ["Alt+↵"],
        accessibilityLabel: "How does the resume list 2026-09-10 22:17",
      },
      {
        id: "session:2",
        label: "Untitled session",
        detail: "2026-09-10 22:15",
        bindings: ["Alt+↵"],
        accessibilityLabel: "Untitled session 2026-09-10 22:15",
      },
    ],
  };
}

function CommandCentreFixture({
  empty = false,
  pathMode = false,
}: {
  empty?: boolean;
  pathMode?: boolean;
}) {
  const [params] = useSearchParams();
  const stageParam = params.get("stage") as PaletteStageId | null;
  const stage = stageParam ? paletteStage(stageParam) : null;
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
            prompt:
              stage?.prompt ?? (pathMode ? "Browse workspace" : "Commands"),
            query: stage?.query ?? (pathMode ? "workspace/" : ""),
            selectedIndex: 0,
            status: empty
              ? { empty: { message: "No commands match this query" } }
              : "active",
            focusPolicy: "modal",
            // Plan 125: the catalogue and the path browser are the composer's
            // palette, and so is every picker stage — this fixture draws the
            // lane's own sheet, never a window-centred one.
            origin: "commandPalette",
            ...(stage?.mode ? { mode: stage.mode } : {}),
            items: stage
              ? stage.items
              : empty
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
                        id: "coding-agent.compact",
                        label: "/compact",
                        detail: "server-first — @clay/coding-agent@0.1.0",
                        scope: "session",
                        bindings: [],
                        accessibilityLabel: "/compact @clay/coding-agent@0.1.0",
                      },
                      {
                        id: "shell.toggleAgentLane",
                        label: "Toggle Agent Lane",
                        detail: "client — built-in",
                        scope: "shell",
                        bindings: ["Ctrl+X Ctrl+P"],
                        accessibilityLabel: "Toggle Agent Lane built-in",
                      },
                      {
                        id: "controlCenter.openPath",
                        label: "Browse Filesystem",
                        detail: "client — built-in",
                        scope: "files",
                        bindings: ["Ctrl+X Ctrl+F"],
                        accessibilityLabel: "Browse Filesystem built-in",
                      },
                    ],
          },
        },
      },
    });
    return created;
  }, [empty, pathMode, stage]);
  // The palette's intents are the fixture workspace's own (the sheet is the
  // field's menu, so the lane drives the session): the fixture opens the
  // session itself and lets query/move/activate/cancel reach the stub send.
  const palette = useMemo<ComposerPalette>(
    () => ({
      menu:
        workspace.active()?.menu?.origin === "commandPalette"
          ? (workspace.active()?.menu ?? null)
          : null,
      request: () => undefined,
      query: (filter, scope) => workspace.menuQuery(filter, scope),
      move: (delta) => workspace.menuMove(delta),
      activate: (secondary) => workspace.menuActivate(secondary),
      back: () => workspace.menuBackspace(),
      cancel: () => workspace.menuCancel(),
    }),
    [workspace],
  );
  const storeRef = useRef<AgentSessionModule | null>(null);
  storeRef.current ??= createAgentSession({});
  return (
    <div
      className={`${styles.fixture} ${styles.paletteFixture}`}
      data-fixture="command-centre"
    >
      <AgentLaneLazy
        store={storeRef.current}
        uiVersion={4}
        workspaceRoot="/tmp/project"
        agentType="coding-agent"
        palette={palette}
      />
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

const WORKSPACE_FIXTURE_DOC = `# Notes

Fixture document for the workspace review surface.

## 26-08-12 01:15 — outline-entry-one

The rail lists this heading with its time and title.

## 26-08-13 09:40 — outline-entry-two

A second entry, so the rail shows a list rather than a single row.
`;

/**
 * Settings fixture: the panel in its real host (`PackageWorkspace`'s right
 * slot), with the selection sets the runtime enumerates stated locally — the
 * fixture has no server, so the panel would otherwise render empty dropdowns.
 * Specifier literals stay in `const` declarations (plan 104 guard).
 */
function SettingsFixture() {
  useEffect(() => {
    themeStore.setTheme({
      specifier: SHIPPED_THEME,
      tokens: {},
      editorStyles: {},
      densityScale: 1,
    } as unknown as ThemeSnapshot);
    themeStore.setUiChoices(SHIPPED_UI_CHOICES);
    return () => themeStore.setUiChoices(null);
  }, []);
  return <PackageUiFixture settingsOpen />;
}

/**
 * Agent settings fixture: the coding agent's Settings tab body as a page-level
 * surface (approved `agent-settings.html`), with the delivered-file rows a
 * daemon writes — the layout the client cannot read for itself without a
 * server. `?state=empty` is the documented first-run state.
 */
function AgentSettingsFixture() {
  const [params] = useSearchParams();
  const empty = params.get("state") === "empty";
  return (
    <div className={styles.fixture} data-fixture="agent-settings">
      <AgentSettingsPanel
        files={empty ? [] : DELIVERED_AGENT_FILES}
        loading={false}
        onOpen={() => undefined}
      />
    </div>
  );
}

/** Delivered agent config files in the daemon's fixed layout (`SYSTEM.md` plus
 *  one `SKILL.md` per bundled skill), with built-in/edited provenance. */
const DELIVERED_AGENT_FILES: AgentSettingsFileInfo[] = [
  {
    name: "SYSTEM.md",
    displayPath: "/root/SYSTEM.md",
    sizeBytes: 1830,
    modifiedMs: 1,
    edited: false,
  },
  {
    name: "skills/create-plan/SKILL.md",
    displayPath: "/root/skills/create-plan/SKILL.md",
    sizeBytes: 7600,
    modifiedMs: 2,
    edited: false,
  },
  {
    name: "skills/impeccable/SKILL.md",
    displayPath: "/root/skills/impeccable/SKILL.md",
    sizeBytes: 10800,
    modifiedMs: 3,
    edited: true,
  },
];

function PackageUiFixture({
  settingsOpen = false,
}: {
  settingsOpen?: boolean;
}) {
  const session = useMemo(() => {
    const created = createDocumentSession({ send: async () => undefined });
    created.installInitial({
      ...fixtureBootstrap,
      initialDocument: {
        ...fixtureBootstrap.initialDocument,
        head: {
          totalBytes: WORKSPACE_FIXTURE_DOC.length,
          firstChunk: WORKSPACE_FIXTURE_DOC,
        },
      },
    } as unknown as BootstrapDto);
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
          // The server's file-browser region is a flat stack, not a panel: the
          // host's left slot owns the region's paint (src/shell/file_browser.rs).
          { id: 2, kind: { stack: { children: [7, 5] } } },
          { id: 3, kind: { label: { text: "Workspace · clay", icon: null } } },
          {
            id: 6,
            kind: {
              button: {
                label: "Hide file browser",
                icon: "disclosure.right",
                action: {
                  commandId: "workspace.toggleFileBrowser",
                  source: { button: { nodeId: 6 } },
                  arguments: [],
                },
              },
            },
          },
          {
            id: 7,
            kind: { flex: { direction: "row", children: [3, 6] } },
          },
          {
            id: 4,
            kind: {
              editorView: { binding: { documentId: 1, expectedVersion: 1 } },
            },
          },
          {
            id: 5,
            kind: {
              list: {
                items: [
                  {
                    id: "src",
                    label: "src",
                    detail: null,
                    action: null,
                    icon: "file.folder",
                  },
                  {
                    id: "notes.md",
                    label: "notes.md",
                    detail: null,
                    action: null,
                    icon: "file.file",
                  },
                ],
              },
            },
          },
        ],
      }),
    [],
  );
  return (
    <div
      className={styles.packageFixture}
      data-fixture={settingsOpen ? "settings" : "package-ui"}
    >
      <WorkspaceView session={session}>
        {/* The working-area grid places its items explicitly (routes/workspace
            .module.css), so the panes ride the same view-area slot the real
            shell's `WorkspacePanes` uses; the fixtures render the tree whole,
            so the sidebar region keeps the renderer's fallback width. */}
        <div className={panesStyles.viewArea} data-panes="view-area">
          <PackageWorkspace
            sdui={sdui}
            packageUi={packageFixtureSnapshot}
            settingsOpen={settingsOpen}
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
      </WorkspaceView>
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

function DocumentLoadingFixture() {
  const session = useMemo(() => {
    const created = createDocumentSession({
      send: async () => undefined,
    });
    created.installInitial({
      ...fixtureBootstrap,
      initialDocument: {
        ...fixtureBootstrap.initialDocument,
        head: {
          totalBytes: 1024 * 1024,
          firstChunk:
            "Large document head\n\nThe first viewport is ready while the remaining chunks load.\n",
        },
      },
    } as unknown as BootstrapDto);
    created.store.update({ workspaceRootId: 1, path: "large-document.txt" });
    return created;
  }, []);

  return (
    <div className={styles.documentFixture} data-fixture="document-loading">
      <div className={styles.reviewStatus} role="status" aria-live="polite">
        <ClayText variant="status">Loading document…</ClayText>
      </div>
      <div className={styles.documentSurface}>
        <Suspense
          fallback={<ClayText variant="status">Loading editor…</ClayText>}
        >
          <ClayEditor session={session} />
        </Suspense>
      </div>
    </div>
  );
}

type DocumentErrorKind = "budget" | "binary";

const documentErrorMessages: Record<DocumentErrorKind, string> = {
  budget:
    "opening workspace file large-document.txt would exceed the 268435456 byte resident document budget",
  binary:
    "workspace file sample.bin appears to be binary and is not supported as a text document",
};

function DocumentErrorFixture({ kind }: { kind: DocumentErrorKind }) {
  const message = documentErrorMessages[kind];
  const workspace = useMemo(() => {
    const created = createWorkspace({ send: async () => undefined });
    created.installBootstrap({
      ...fixtureBootstrap,
      initialDocument: {
        ...fixtureBootstrap.initialDocument,
        head: { totalBytes: 0, firstChunk: "" },
      },
    } as unknown as BootstrapDto);
    created.active()?.panes.get(1)?.session.store.update({
      diagnostic: message,
    });
    return created;
  }, [message]);

  return (
    <div
      className={styles.documentFixture}
      data-fixture={`document-${kind}-error`}
    >
      <div className={styles.reviewStatus} role="status" aria-live="polite">
        <ClayText variant="status">{message}</ClayText>
      </div>
      <div className={styles.documentSurface}>
        <Suspense
          fallback={<ClayText variant="status">Loading workspace…</ClayText>}
        >
          <WorkspacePanes workspace={workspace} />
        </Suspense>
      </div>
    </div>
  );
}

const packageFixtureSnapshot: PackageUiSnapshot = {
  version: 4,
  emptyTab: null,
  surfaces: [],
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
  protocolVersion: 28,
  endpoint: "fixture",
  generation: 1,
  initialDocument: {
    documentId: 1,
    version: 1,
    head: { totalBytes: 17, firstChunk: "fixture document\n" },
    access: { editable: { leaseId: 1 } },
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: behaviorManifestFixture({ behaviorVersion: 1 }),
  activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
  activeTypography: {
    revision: 1,
    monospace: {
      families: ["monospace"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
    },
    proportional: {
      families: ["serif"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
    },
    ui: {
      families: ["system-ui"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
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

function CodingAgentFixture() {
  const [params] = useSearchParams();
  const state = params.get("state") ?? "landing";
  // Plan 119 SC-6: the fixture owns its tab store (there is no process-global
  // agent session any more), seeds it, and hands it to the panel.
  const storeRef = useRef<AgentSessionModule | null>(null);
  storeRef.current ??= createAgentSession({});
  const store = storeRef.current;
  useEffect(() => {
    seedAgentFixture(store, state === "landing" ? "landing" : state);
  }, [store, state]);
  return (
    <div
      className={`${styles.packageFixture} ${styles.agentFixture}`}
      data-fixture={`coding-agent-${state}`}
    >
      {/* Plan 124: the composition under review is the agent *view* plus the
          tab's persistent lane beneath it — the panel no longer owns a
          composer, so a fixture that rendered the panel alone would review a
          surface the app never draws. */}
      <Suspense fallback={<ClayText variant="status">Loading agent…</ClayText>}>
        <CodingAgentSurfaceLazy
          surface={codingAgentFixtureSurface}
          uiVersion={4}
          agent={store}
        />
      </Suspense>
      <AgentLaneLazy
        store={store}
        uiVersion={4}
        workspaceRoot="/tmp/project"
        agentType="coding-agent"
        palette={IDLE_PALETTE}
      />
    </div>
  );
}

const AgentLaneLazy = lazy(async () => {
  const module = await import("../shell/AgentLane");
  return { default: module.AgentLane };
});

const CodingAgentSurfaceLazy = lazy(async () => {
  const module = await import("../coding-agent/CodingAgentPanel");
  return { default: module.CodingAgentPanel };
});

const codingAgentFixtureSurface = {
  id: "coding-agent.surface",
  actionTargets: [
    "coding-agent.profile",
    "coding-agent.close",
    "agent.clientOpenModelPicker",
    "documents.clientOpenFileDialog",
  ],
  provenance: {
    packageName: "@clay/coding-agent",
    packageVersion: "0.1.0",
    apiPrefix: "coding-agent",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "coding-agent.root",
    title: "Coding Agent",
    children: [
      {
        kind: "label" as const,
        id: "coding-agent.transcriptTitle",
        text: "Coding Agent",
      },
      {
        kind: "label" as const,
        id: "coding-agent.emptyHint",
        text: "No conversation yet.",
      },
      {
        kind: "textInput" as const,
        id: "coding-agent.composer",
        title: "Message",
        multiline: true,
      },
    ],
  },
} as never;

function seedAgentFixture(store: AgentSessionModule, state: string) {
  if (state === "landing") {
    store.seedForDev({
      messages: [],
      state: {},
      streaming: false,
      statusText: null,
    });
    return;
  }
  if (state === "conversation") {
    store.seedForDev({
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
        // Plan 118 task 36: the rows carry the file records the server derives
        // from a call's own arguments, so the Files tab (session history) has
        // real content in a capture. Paths are real repo files; the roles are
        // this fixture session's own state.
        {
          id: "f2a",
          role: "tool",
          content:
            'read {"path":"plans/118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md"}',
          metadata: {
            clayKind: "tool",
            toolName: "read",
            sessionFile: {
              path: "plans/118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md",
              op: "read",
            },
          },
        },
        {
          id: "f2b",
          role: "tool",
          content:
            'edit {"path":"frontend/src/coding-agent/coding-agent.module.css"}',
          metadata: {
            clayKind: "tool",
            toolName: "edit",
            sessionFile: {
              path: "frontend/src/coding-agent/coding-agent.module.css",
              op: "edit",
            },
          },
        },
        {
          id: "f2c",
          role: "tool",
          content:
            'write {"path":"design-artifacts/approved/quiet-instrument-migration/start.html"}',
          metadata: {
            clayKind: "tool",
            toolName: "write",
            sessionFile: {
              path: "design-artifacts/approved/quiet-instrument-migration/start.html",
              op: "write",
            },
          },
        },
        {
          id: "f3",
          role: "assistant",
          content: "42 tokens",
          metadata: { clayKind: "usage" },
        },
      ] as never,
      // The panel's full inventory surface: branch, extensions, MCP servers,
      // the models/providers pair the picker and the effort levels resolve
      // from, and the catalog skills the Context tab pins. A fixture with only
      // `provider` renders the transcript but none of those inspected tabs.
      state: {
        provider: "mock",
        model: "mock-mini",
        agent: "coding-agent",
        // A bound session id + OM view: Settings and Memory render their real
        // bodies (both fall back to "no session" without one), and the
        // @-mention fetch gate keys off the same field.
        sessionId: "fixture-session",
        omView: {
          sessionId: "fixture-session",
          attached: true,
          observation: { provider: "mock", model: "mock-mini" },
          reflection: { provider: "mock", model: "mock-mini" },
          activity: [
            {
              kind: "observation",
              summary: "Noted the SC-4 module split.",
              at: "09:12",
            },
            {
              kind: "reflection",
              summary: "Consolidated the panel ownership rule.",
              at: "09:40",
            },
          ],
        },
        branch: "main",
        extensions: ["wiki", "graft"],
        contextTokens: 4210,
        effort: "medium",
        mcpServers: [
          { serverId: "prism", connected: true, tools: 7, error: "" },
          {
            serverId: "filesystem",
            connected: false,
            tools: 0,
            error: "connect timed out",
          },
        ],
        models: [
          {
            provider: "mock",
            model: "mock-mini",
            displayName: "Mock Mini",
            contextWindow: 1_000_000,
            thinkingLevels: ["low", "medium", "high"],
          },
          {
            provider: "mock",
            model: "mock-large",
            displayName: "Mock Large",
            contextWindow: 2_000_000,
            thinkingLevels: ["low", "medium", "high"],
          },
        ],
        providers: [
          { id: "mock", configured: true },
          { id: "unconfigured", configured: false },
        ],
        skills: [
          {
            name: "clay-execution",
            description: "Plan, execute, and verify repo work.",
          },
          {
            name: "design-taste-frontend",
            description: "Anti-slop frontend direction.",
          },
        ],
      },
      streaming: false,
      statusText: null,
    });
    return;
  }
  if (state === "streaming") {
    store.seedForDev({
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
  if (state === "approval") {
    store.seedForDev({
      messages: [
        { id: "a0", role: "user", content: "Clean up the build directory" },
        {
          id: "a1",
          role: "tool",
          content: 'bash {"command":"rm -rf target/debug"}',
          metadata: { clayKind: "tool", toolName: "bash" },
        },
      ] as never,
      state: { provider: "mock", model: "mock-mini" },
      streaming: false,
      statusText: "waiting for approval",
      pendingApproval: {
        sessionId: "fixture-session",
        runId: "fixture-run",
        requestId: "fixture-request",
        toolName: "bash",
      },
    });
    return;
  }
  if (state === "error") {
    store.seedForDev({
      messages: [
        { id: "e0", role: "user", content: "List files" },
        {
          id: "e1",
          role: "assistant",
          content: "provider unreachable",
          metadata: { clayKind: "error" },
        },
      ] as never,
      state: { provider: "mock", model: "mock-mini" },
      streaming: false,
      statusText: null,
    });
  }
}
