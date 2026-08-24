import "@testing-library/jest-dom/vitest";

// jsdom lacks ResizeObserver, which react-resizable-panels requires.
class ResizeObserverStub {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}
globalThis.ResizeObserver ??=
  ResizeObserverStub as unknown as typeof ResizeObserver;
