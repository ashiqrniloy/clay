// Design system runtime state: holds the latest resolved ActiveDesignSystem snapshot
// and installs CSS custom properties into the document root once per generation.
// Dependency-free observable store.

import {
  designSystemCssVariables,
  installDesignSystemVariables,
} from "../theme/design-system-adapter";
import type { DesignSystemSnapshot } from "../theme/types";

export interface DesignSystemState {
  designSystem: DesignSystemSnapshot | null;
  installedKeys: Set<string>;
}

export interface DesignSystemStore {
  get(): DesignSystemState;
  setDesignSystem(designSystem: DesignSystemSnapshot | null): void;
  resetToFallback(): void;
  subscribe(listener: () => void): () => void;
}

export function createDesignSystemStore(
  root: { style: CSSStyleDeclaration } = document.documentElement,
): DesignSystemStore {
  let state: DesignSystemState = {
    designSystem: null,
    installedKeys: new Set(),
  };
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of [...listeners]) listener();
  };

  return {
    get: () => state,
    setDesignSystem(ds) {
      if (
        state.designSystem?.specifier === ds?.specifier &&
        state.designSystem?.generation === ds?.generation &&
        state.designSystem?.schemaVersion === ds?.schemaVersion
      ) {
        // Idempotent: identical revision causes no DOM writes or subscriber churn.
        return;
      }

      if (!ds) {
        for (const key of state.installedKeys) {
          root.style.removeProperty(key);
        }
        state = { designSystem: null, installedKeys: new Set() };
        notify();
        return;
      }

      // Validate and convert variables before any DOM writes.
      // If validation fails, error throws and state/DOM remain intact.
      const variables = designSystemCssVariables(ds);
      const nextKeys = installDesignSystemVariables(
        root.style,
        variables,
        state.installedKeys,
      );

      state = { designSystem: ds, installedKeys: nextKeys };
      notify();
    },
    resetToFallback() {
      for (const key of state.installedKeys) {
        root.style.removeProperty(key);
      }
      state = { designSystem: null, installedKeys: new Set() };
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
