import { describe, expect, it } from "vitest";

import { normalizeBridgeError } from "../bridge/errors";
import {
  asDocumentId,
  type BridgeEnvelope,
  type BootstrapDto,
  type ThemeSnapshot,
  type TypographySnapshot,
} from "../bridge/types";
import {
  applyEnvelope,
  createConnectionStore,
} from "../state/connection-store";

const themeSnapshot: ThemeSnapshot = {
  specifier: "",
  tokens: { "surface.main": { type: "color", value: "#100f17" } },
  densityScale: 1,
};
const typographySnapshot: TypographySnapshot = {
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
    title: 1.07,
    section: 1,
    body: 1,
    status: 1,
    detail: 0.77,
    caption: 0.75,
  },
};

const bootstrap: BootstrapDto = {
  clientId: 1,
  protocolVersion: 26,
  endpoint: "/tmp/x.sock",
  generation: 1,
  initialDocument: {
    documentId: asDocumentId(1),
    version: 1,
    text: "",
    access: {},
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: {
    manifestId: "m",
    behaviorVersion: 1,
    commands: [],
    keymaps: [],
  },
  activeTheme: themeSnapshot,
  activeTypography: typographySnapshot,
};

describe("connection store", () => {
  it("transitions idle → bootstrapping → ready", () => {
    const store = createConnectionStore();
    expect(store.get().phase).toBe("idle");
    store.set({ phase: "bootstrapping" });
    store.set({ phase: "ready", bootstrap });
    expect(store.get()).toEqual({ phase: "ready", bootstrap });
  });

  it("disconnected envelopes replace ready state exactly once", () => {
    const state = createConnectionStore();
    state.set({ phase: "ready", bootstrap });
    const envelope: BridgeEnvelope = {
      kind: "disconnected",
      data: { reason: "connection closed" },
    };
    state.set(applyEnvelope(state.get(), envelope));
    expect(state.get().phase).toBe("disconnected");
    // Idempotent: a second identical notice must not clobber the reason.
    state.set(applyEnvelope(state.get(), envelope));
    expect(state.get()).toEqual({
      phase: "disconnected",
      reason: "connection closed",
    });
  });

  it("notifies subscribers on change", () => {
    const store = createConnectionStore();
    let notifications = 0;
    const unsubscribe = store.subscribe(() => {
      notifications += 1;
    });
    store.set({ phase: "bootstrapping" });
    unsubscribe();
    store.set({ phase: "bootstrapping" });
    expect(notifications).toBe(1);
  });
});

describe("bridge error normalization", () => {
  it("passes through structured errors", () => {
    expect(
      normalizeBridgeError({ code: "queueFull", message: "slow down" }),
    ).toEqual({ code: "queueFull", message: "slow down" });
  });

  it("wraps opaque throwables with sanitized messages", () => {
    const normalized = normalizeBridgeError(new Error("boom"));
    expect(normalized.code).toBe("invalidRequest");
    expect(normalized.message).toBe("boom");
    expect(normalizeBridgeError(42).message).toContain("42");
  });
});
