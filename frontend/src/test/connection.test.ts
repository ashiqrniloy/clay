import { describe, expect, it } from "vitest";

import { connectionView, type ConnectionView } from "../app/connection";
import type { ServerStatus } from "../lib/server";

const connecting: ServerStatus = {
  state: "connecting",
  endpoint: "/run/user/1000/clay.sock",
};
const connected: ServerStatus = {
  state: "connected",
  endpoint: "/run/user/1000/clay.sock",
  pid: 4321,
};
const disconnected: ServerStatus = {
  state: "disconnected",
  reason: "endpoint did not accept within 15000ms",
};

function expectKind<K extends ConnectionView["kind"]>(
  status: ServerStatus,
  kind: K,
): Extract<ConnectionView, { kind: K }> {
  const view = connectionView(status);
  if (view.kind !== kind) throw new Error(`expected ${kind}, got ${view.kind}`);
  return view as Extract<ConnectionView, { kind: K }>;
}

describe("connectionView", () => {
  it("maps connecting to loading", () => {
    expect(expectKind(connecting, "loading").message).toContain("Connecting");
  });

  it("maps connected to ready", () => {
    const view = expectKind(connected, "ready");
    expect(view.message).toContain("4321");
  });

  it("maps disconnected to a retryable error", () => {
    const view = expectKind(disconnected, "error");
    expect(view.retryable).toBe(true);
    expect(view.message).toContain("unavailable");
  });

  it("exhausts every typed status variant", () => {
    // Type-level exhaustiveness aid: keep this list in sync with ServerStatus.
    const all: ServerStatus[] = [connecting, connected, disconnected];
    for (const status of all) {
      expect(connectionView(status).message.length).toBeGreaterThan(0);
    }
  });
});
