import { createDesignSystemStore } from "./design-system-store";
import { createIconStore } from "./icon-store";
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

/** Design system runtime singleton; installs recipe variables into the document root. */
export const designSystemStore = safeCreate(
  () => createDesignSystemStore(),
  createDesignSystemStore({
    style: {
      setProperty: () => {},
      removeProperty: () => {},
    } as unknown as CSSStyleDeclaration,
  }),
);

/** Icon pack runtime singleton; holds bounded active geometry (no DOM writes). */
export const iconStore = createIconStore();
