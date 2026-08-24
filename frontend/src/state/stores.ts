// App-level store singletons. Created lazily so tests can import types
// without touching `document`.

import { createThemeStore } from "./theme-store";

function safeCreate<T>(create: () => T, fallback: T): T {
  try {
    return typeof document === "undefined" ? fallback : create();
  } catch {
    return fallback;
  }
}

/** Theme runtime singleton; installs snapshots into the document root. */
export const themeStore = safeCreate(
  () => createThemeStore(),
  // jsdom-less contexts (pure reducers) get a no-op style target.
  createThemeStore({
    style: { setProperty: () => {} } as unknown as CSSStyleDeclaration,
  }),
);
