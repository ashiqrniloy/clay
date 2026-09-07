import { describe, expect, it, vi } from "vitest";

import {
  createIconStore,
  isValidIconPackSnapshot,
  type IconPackSnapshot,
} from "../state/icon-store";

function pack(overrides: Partial<IconPackSnapshot> = {}): IconPackSnapshot {
  return {
    specifier: "@clay/icons-phosphor-regular",
    schemaVersion: 1,
    generation: 5,
    provenance: {
      packageName: "@clay/icons-phosphor-regular",
      packageVersion: "2.0.8",
      apiPrefix: "icons-phosphor-regular",
      trustDomain: "trusted",
    },
    icons: {
      "action.close": {
        viewBox: [0, 0, 24, 24],
        paths: [{ d: "M 4,4 L 20,20 L 4,20 Z" }],
      },
      "document.save": {
        viewBox: [0, 0, 256, 256],
        paths: [
          { d: "M 8,8 L 8,2 L 18,2 L 18,8 Z", opacity: 0.2 },
          { d: "M 6,2 L 2,6 L 2,22 L 22,22 L 22,6 L 18,2 Z" },
        ],
      },
    },
    ...overrides,
  };
}

describe("icon store", () => {
  it("installs an authorized pack and serves per-key geometry", () => {
    const store = createIconStore();
    expect(store.get().pack).toBeNull();
    expect(store.getIcon("action.close")).toBeUndefined();

    const listener = vi.fn();
    store.subscribe(listener);
    store.setIconPack(pack());
    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.getIcon("action.close")?.paths[0]?.d).toBe("M 4,4 L 20,20 L 4,20 Z");
    // Missing key: per-key fallback is the consumer's job (host subset).
    expect(store.getIcon("git.branch")).toBeUndefined();
  });

  it("is idempotent for identical revisions (no subscriber churn)", () => {
    const store = createIconStore();
    const listener = vi.fn();
    store.subscribe(listener);
    store.setIconPack(pack());
    store.setIconPack(pack());
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("rejects stale generations and keeps the last authorized state", () => {
    const store = createIconStore();
    store.setIconPack(pack({ specifier: "@clay/icons-phosphor-duotone", generation: 9 }));
    const listener = vi.fn();
    store.subscribe(listener);

    store.setIconPack(pack({ generation: 8 }));
    expect(listener).not.toHaveBeenCalled();
    expect(store.get().pack?.specifier).toBe("@clay/icons-phosphor-duotone");
    expect(store.get().pack?.generation).toBe(9);

    // Same generation + same identity: idempotent no-op.
    store.setIconPack(pack({ specifier: "@clay/icons-phosphor-duotone", generation: 9 }));
    expect(listener).not.toHaveBeenCalled();
  });

  it("accepts a same-generation identity swap (explicit user switch)", () => {
    const store = createIconStore();
    store.setIconPack(pack({ generation: 9 }));
    store.setIconPack(pack({ specifier: "@clay/icons-phosphor-duotone", generation: 9 }));
    expect(store.get().pack?.specifier).toBe("@clay/icons-phosphor-duotone");
  });

  it("rejects malformed snapshots without touching state", () => {
    const store = createIconStore();
    store.setIconPack(pack());
    const listener = vi.fn();
    store.subscribe(listener);

    for (const malformed of [
      42,
      "icons",
      {},
      { ...pack(), icons: {} },
      { ...pack(), specifier: "  " },
      { ...pack(), icons: { "action.close": { viewBox: [0, 0, 24], paths: [{ d: "M 0,0" }] } } },
      {
        ...pack(),
        icons: {
          "action.close": { viewBox: [0, 0, 900, 24], paths: [{ d: "M 0,0" }] },
        },
      },
      {
        ...pack(),
        icons: { "action.close": { viewBox: [0, 0, 24, 24], paths: [] } },
      },
      {
        ...pack(),
        icons: {
          "action.close": { viewBox: [0, 0, 24, 24], paths: [{ d: "" }] },
        },
      },
      {
        ...pack(),
        icons: {
          "action.close": { viewBox: [0, 0, 24, 24], paths: [{ d: "M 0,0", opacity: 3 }] },
        },
      },
    ]) {
      expect(isValidIconPackSnapshot(malformed)).toBe(false);
      store.setIconPack(malformed as unknown as IconPackSnapshot | null);
    }
    expect(listener).not.toHaveBeenCalled();
    expect(store.get().pack?.specifier).toBe("@clay/icons-phosphor-regular");
  });

  it("treats null as the legitimate host fallback and notifies", () => {
    const store = createIconStore();
    store.setIconPack(pack());
    store.setIconPack(null);
    expect(store.get().pack).toBeNull();
    // Idempotent reset.
    const listener = vi.fn();
    store.subscribe(listener);
    store.resetToFallback();
    expect(listener).not.toHaveBeenCalled();
  });
});
