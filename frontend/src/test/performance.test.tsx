import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { useEffect } from "react";

import { ClayText } from "../components";
import {
  applyEnvelope,
  createConnectionStore,
} from "../state/connection-store";
import type { BootstrapDto } from "../bridge/types";

describe("render-count discipline", () => {
  it("re-renders a subscribed component once per store change, not per event family", () => {
    const store = createConnectionStore();
    let renders = 0;
    function Probe() {
      useEffect(() => store.subscribe(() => (renders += 1)));
      return <div>{store.get().phase}</div>;
    }
    render(<Probe />);
    const before = renders;
    store.set({ phase: "bootstrapping" });
    store.set({ phase: "bootstrapping" }); // same state: still notifies (cheap), consumers dedupe
    store.set({
      phase: "ready",
      bootstrap: {} as unknown as BootstrapDto,
    });
    expect(renders - before).toBe(3);
  });

  it("ignores non-lifecycle envelopes in the connection reducer", () => {
    const store = createConnectionStore();
    store.set({ phase: "ready", bootstrap: {} as unknown as BootstrapDto });
    const state = store.get();
    // Decoration-style opaque events must not churn connection state.
    for (let i = 0; i < 100; i += 1) {
      expect(
        applyEnvelope(state, {
          kind: "event",
          data: { kind: "editAck", data: {} },
        }),
      ).toBe(state);
    }
  });
});

describe("text component", () => {
  it("maps variants to token-driven classes without inline sizes", () => {
    render(
      <ClayText variant="title" muted>
        Heading
      </ClayText>,
    );
    const node = screen.getByText("Heading");
    expect(node.className).toMatch(/title/);
    expect(node.getAttribute("style")).toBeNull();
  });
});
