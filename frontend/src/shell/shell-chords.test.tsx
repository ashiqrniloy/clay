// @vitest-environment jsdom
// Regression (plan 117 follow-up): Ctrl+X Ctrl+P must resolve the Control
// Center chord with the EXACT manifest JSON the real server publishes
// (context "global", routingPolicy "serverFirst", key as {character}) —
// captured live from an example-config server boot
// (tests/example_config_control_center_chord.rs). Covers both keymap
// owners: the shell matcher outside the editor and the CodeMirror chord
// keymap inside it, including the two real-world killers: a Control
// keydown arriving between strokes (held/re-pressed Ctrl) and held-key
// auto-repeat of the first stroke.

import { describe, expect, it, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async () => undefined),
}));

import { workspaceRail } from "./layout-state";
import { createWorkspace } from "./workspace-controller";
import type { BootstrapDto } from "../bridge/types";
import { useShellChords } from "./use-shell-chords";
import { WorkspacePanes } from "./WorkspacePanes";
import { behaviorExtensions } from "../editor/extensions/behavior";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

const stroke = (character: string) => ({
  key: { character },
  modifiers: { shift: false, control: true, alt: false, superKey: false },
});

const CONTROL_CENTER_RULE = {
  commandId: "controlCenter.open",
  sequence: [stroke("x"), stroke("p")],
  context: "global",
  routingPolicy: "serverFirst",
};

const OPEN_PATH_RULE = {
  commandId: "controlCenter.openPath",
  sequence: [stroke("x"), stroke("f")],
  context: "global",
  routingPolicy: "serverFirst",
};

function bootstrap(clientId: number, keymaps: unknown[]): BootstrapDto {
  return {
    protocolVersion: 28,
    endpoint: "test",
    generation: 1,
    clientId,
    initialDocument: {
      documentId: clientId as never,
      version: 1,
      head: { totalBytes: 0, firstChunk: "" },
      access: { none: {} },
      workspaceRoot: `/tmp/ws${clientId}`,
    },
    behaviorManifest: {
      manifestId: "m",
      behaviorVersion: 2,
      commands: [],
      keymaps,
    },
    activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
    activeTypography: {
      revision: 1,
      monospace: { families: ["m"], size: 13 },
      proportional: { families: ["p"], size: 13 },
      ui: { families: ["u"], size: 13 },
    },
    activeDesignSystem: {
      specifier: "@clay/core",
      schemaVersion: 1,
      generation: 1,
      provenance: {
        packageName: "core",
        packageVersion: "1.0.0",
        apiPrefix: "clay",
        trustDomain: "trusted",
      },
      recipes: {},
      variables: {},
    },
  } as unknown as BootstrapDto;
}

function pressChord(user: ReturnType<typeof userEvent.setup>) {
  return act(async () => {
    await user.keyboard("{Control>}x");
    await user.keyboard("{Control>}p");
  });
}

describe("Ctrl+X Ctrl+P opens the Control Center (plan 117 follow-up)", () => {
  it("shell matcher dispatches the command intent with the real wire manifest", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload as string);
      },
    });
    ws.installBootstrap(bootstrap(1, [CONTROL_CENTER_RULE, OPEN_PATH_RULE]));

    function Host() {
      useShellChords(ws, true);
      return <div data-testid="host" />;
    }
    render(<Host />);
    const user = userEvent.setup();
    await pressChord(user);

    const intent = sent
      .map((raw) => JSON.parse(raw))
      .find((payload) => payload.family === "commandIntent");
    expect(intent).toBeDefined();
    expect(intent.payload.commandId).toBe("controlCenter.open");
  });

  it("a Control keydown between strokes does not cancel the pending chord", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload as string);
      },
    });
    ws.installBootstrap(bootstrap(1, [CONTROL_CENTER_RULE]));

    function Host() {
      useShellChords(ws, true);
      return <div data-testid="host" />;
    }
    render(<Host />);
    // Simulate exactly what broke the chord before the fix: the Control
    // key fires its own keydown between the two strokes (held/re-pressed
    // Ctrl), which reset the pending chord and swallowed the second stroke.
    const fire = (key: string, ctrl: boolean, repeat = false) =>
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key,
          ctrlKey: ctrl,
          repeat,
          bubbles: true,
        }),
      );
    act(() => {
      fire("Control", true);
      fire("x", true);
      fire("Control", true); // the killer
      fire("p", true);
      fire("x", true, true); // held-stroke auto-repeat after resolve
      fire("p", true, true); // repeat of the resolving stroke
    });

    const intents = sent
      .map((raw) => JSON.parse(raw))
      .filter((payload) => payload.family === "commandIntent");
    expect(intents).toHaveLength(1);
    expect(intents[0].payload.commandId).toBe("controlCenter.open");
  });

  it("renders the command centre overlay end to end through WorkspacePanes", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload as string);
      },
    });
    ws.installBootstrap(bootstrap(1, [CONTROL_CENTER_RULE, OPEN_PATH_RULE]));

    render(<WorkspacePanes workspace={ws} />);
    const user = userEvent.setup();
    const host = screen.getByTestId("workspace-panes");
    await act(async () => {
      host.focus();
      await user.keyboard("{Control>}x");
      await user.keyboard("{Control>}p");
    });
    // The chord dispatches the intent; the snapshot arrives as an envelope.
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "transientMenuSnapshot",
        data: {
          sessionId: 1,
          generationId: 1,
          prompt: "Command Centre",
          query: "",
          items: [
            {
              id: "runtime.reloadConfiguration",
              label: "Reload Configuration",
              detail: "",
            },
          ],
          selectedIndex: 0,
          status: "active",
          focusPolicy: "modal",
          origin: "centered",
        },
      },
    } as never);
    expect(await screen.findByRole("dialog")).toBeDefined();
  });

  it("editor chord keymap fires inside CodeMirror despite Control keydown noise", async () => {
    const sent: string[] = [];
    const state = EditorState.create({
      doc: "hello",
      extensions: [
        behaviorExtensions(
          {
            manifestId: "m",
            behaviorVersion: 2,
            commands: [],
            keymaps: [CONTROL_CENTER_RULE, OPEN_PATH_RULE],
          } as never,
          (commandId) => {
            sent.push(commandId);
            return true;
          },
        ),
      ],
    });
    const host = document.createElement("div");
    document.body.append(host);
    const view = new EditorView({ state, parent: host });
    view.focus();

    const fire = (key: string, ctrl: boolean, repeat = false) =>
      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", {
          key,
          ctrlKey: ctrl,
          repeat,
          bubbles: true,
        }),
      );
    fire("Control", true);
    fire("x", true);
    fire("Control", true); // modifier noise between strokes
    fire("p", true);
    expect(sent).toEqual(["controlCenter.open"]);
    view.destroy();
    host.remove();
  });
});

describe("Ctrl+I toggles the workspace rail (plan 118: shell and Workspace)", () => {
  it("fires from window focus, wherever the caret sits", async () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap(1, [CONTROL_CENTER_RULE]));

    function Host() {
      useShellChords(ws, true);
      return <div data-testid="host" />;
    }
    render(<Host />);
    workspaceRail.setVisible(true);
    const press = () =>
      act(() => {
        window.dispatchEvent(
          new KeyboardEvent("keydown", { key: "i", ctrlKey: true }),
        );
      });
    press();
    expect(workspaceRail.isVisible()).toBe(false);
    press();
    expect(workspaceRail.isVisible()).toBe(true);
    workspaceRail.setVisible(true);
  });

  it("does not consume a plain `i` (editor text stays text)", async () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap(1, [CONTROL_CENTER_RULE]));

    function Host() {
      useShellChords(ws, true);
      return <div data-testid="host" />;
    }
    render(<Host />);
    workspaceRail.setVisible(true);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "i" }));
    });
    expect(workspaceRail.isVisible()).toBe(true);
  });
});
