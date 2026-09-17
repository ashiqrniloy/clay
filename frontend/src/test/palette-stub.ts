import { vi } from "vitest";

import type { TransientMenuSnapshotDto } from "../bridge/types";
import type { ComposerPalette } from "../coding-agent/Composer";

/** The composer's palette wiring for tests: no session open, spied intents, so
 *  a test asserts on the intent a keystroke produced rather than on the shell. */
export function paletteStub(
  overrides: Partial<ComposerPalette> = {},
): ComposerPalette {
  return {
    menu: null,
    request: vi.fn(),
    query: vi.fn(),
    move: vi.fn(),
    activate: vi.fn(),
    cancel: vi.fn(),
    ...overrides,
  };
}

/** A palette session snapshot: the composer's `/` palette (`commandPalette`),
 *  prompt `Commands` (what the server names the catalogue session). */
export function paletteMenu(
  items: TransientMenuSnapshotDto["items"] = [],
  overrides: Partial<TransientMenuSnapshotDto> = {},
): TransientMenuSnapshotDto {
  return {
    sessionId: "7" as never,
    prompt: "Commands",
    query: "",
    items,
    selectedIndex: 0,
    status: "active",
    focusPolicy: "modal",
    origin: "commandPalette",
    ...overrides,
  };
}

/** The palette's rows as the catalogue session carries them: the server's one
 *  detail line per item (`routing — provenance`), its scope tag, and its
 *  chords (plan 124 — the sheet's chips come from these fields). */
export const paletteItems: TransientMenuSnapshotDto["items"] = [
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
];
