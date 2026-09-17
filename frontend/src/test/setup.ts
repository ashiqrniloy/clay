import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Component tests must not leak mounted trees. `@testing-library/react` only
// auto-registers this cleanup under `globals: true`, which this project does
// not set: a leaked render keeps its window listeners alive (e.g. the shell
// chord matcher), so the next test in the file silently double-handles its
// keydowns.
afterEach(() => {
  cleanup();
});

// jsdom lacks ResizeObserver, which react-resizable-panels requires.
class ResizeObserverStub {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}
globalThis.ResizeObserver ??=
  ResizeObserverStub as unknown as typeof ResizeObserver;
