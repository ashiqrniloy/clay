import { describe, expect, it } from "vitest";

import { normalizeBridgeError } from "../bridge/errors";
import {
  asDocumentId,
  type BridgeEnvelope,
  type BootstrapDto,
} from "../bridge/types";
import type {
  ThemeSnapshot,
  TypographySnapshot,
  DesignSystemSnapshot,
} from "../theme/types";
import {
  applyEnvelope,
  createConnectionStore,
} from "../state/connection-store";
import { behaviorManifestFixture } from "../test/contract-fixtures";

const themeSnapshot: ThemeSnapshot = {
  specifier: "",
  tokens: { "surface.main": { type: "color", value: "#100f17" } },
  editorStyles: {},
  densityScale: 1,
};
const typographySnapshot: TypographySnapshot = {
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
    title: 1.07,
    section: 1,
    body: 1,
    status: 1,
    detail: 0.77,
    caption: 0.75,
  },
};
const designSystemSnapshot: DesignSystemSnapshot = {
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
  variables: {
    "button.primary.root.rest.backgroundColor": {
      type: "theme-color-role",
      value: "accent.primary",
    },
    "button.primary.root.rest.borderRadius": {
      type: "radius",
      value: 0,
    },
  },
};

const bootstrap: BootstrapDto = {
  clientId: 1,
  protocolVersion: 28,
  endpoint: "/tmp/x.sock",
  generation: 1,
  initialDocument: {
    documentId: asDocumentId(1),
    version: 1,
    head: { totalBytes: 0, firstChunk: "" },
    access: "readOnly",
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: behaviorManifestFixture({ behaviorVersion: 1 }),
  tabId: null,
  performanceProfile: false,
  activeTheme: themeSnapshot,
  activeTypography: typographySnapshot,
  activeDesignSystem: designSystemSnapshot,
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

describe("design system DTO projection", () => {
  it("carries activeDesignSystem in bootstrap state with typed variables", () => {
    expect(bootstrap.activeDesignSystem.specifier).toBe("@clay/core");
    expect(bootstrap.activeDesignSystem.schemaVersion).toBe(1);
    expect(bootstrap.activeDesignSystem.generation).toBe(1);
    expect(bootstrap.activeDesignSystem.provenance.packageName).toBe("core");
    expect(bootstrap.activeDesignSystem.provenance.trustDomain).toBe("trusted");

    const colorVar =
      bootstrap.activeDesignSystem.variables[
        "button.primary.root.rest.backgroundColor"
      ];
    expect(colorVar).toEqual({
      type: "theme-color-role",
      value: "accent.primary",
    });

    const radiusVar =
      bootstrap.activeDesignSystem.variables[
        "button.primary.root.rest.borderRadius"
      ];
    expect(radiusVar).toEqual({
      type: "radius",
      value: 0,
    });
  });

  it("handles runtimeSnapshot envelopes carrying activeDesignSystem", () => {
    const envelope: BridgeEnvelope = {
      kind: "runtimeSnapshot",
      data: {
        clientId: 1,
        tabId: null,
        snapshot: {
          runtimeGenerationId: 2,
          behaviorManifest: behaviorManifestFixture(),
          activeTheme: themeSnapshot,
          activeTypography: typographySnapshot,
          activeDesignSystem: {
            specifier: "@thirdparty/design-sample",
            schemaVersion: 1,
            generation: 2,
            provenance: {
              packageName: "@thirdparty/design-sample",
              packageVersion: "0.2.0",
              apiPrefix: "design-sample",
              trustDomain: "thirdParty",
            },
            recipes: {
              "button.primary.root.rest": {
                backgroundColor: "surface.overlay",
                backgroundOpacity: 0.9,
                textColor: "text.primary",
                borderColor: "border.subtle",
                borderWidth: 1,
                borderStyle: "solid",
                borderRadius: 8,
                padding: null,
                gap: null,
                innerHighlight: null,
                shadow: [],
                backdropBlur: 12,
                backdropSaturate: 1.2,
                opacity: 1,
                outlineColor: "focus.ring",
                outlineWidth: 2,
                outlineOffset: 1,
                outlineStyle: "solid",
                transitionDuration: 100,
                transitionTiming: "linear",
                transformPreset: "none",
              },
            },
            variables: {
              "button.primary.root.rest.backgroundColor": {
                type: "theme-color-role",
                value: "surface.overlay",
              },
              "button.primary.root.rest.backdropBlur": {
                type: "backdrop-blur",
                value: 12,
              },
            },
          },
          sduiTree: { uiVersion: 2, rootId: 1, nodes: [] },
          packageUi: {
            version: 2,
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
    };

    if (envelope.kind === "runtimeSnapshot") {
      expect(envelope.data.snapshot.activeDesignSystem.specifier).toBe(
        "@thirdparty/design-sample",
      );
      expect(envelope.data.snapshot.activeDesignSystem.generation).toBe(2);
      expect(
        envelope.data.snapshot.activeDesignSystem.recipes[
          "button.primary.root.rest"
        ]?.backdropBlur,
      ).toBe(12);
      expect(
        envelope.data.snapshot.activeDesignSystem.variables[
          "button.primary.root.rest.backdropBlur"
        ],
      ).toEqual({
        type: "backdrop-blur",
        value: 12,
      });
    }
  });
});
