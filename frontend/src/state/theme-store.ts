// Theme runtime state: holds the latest resolved snapshots and installs them
// into the document once per revision. Dependency-free observable store.

import {
  installVariables,
  themeCssVariables,
  typographyCssVariables,
} from "../theme/adapter";
import type { ThemeSnapshot, TypographySnapshot } from "../theme/types";
import type { UiChoicesSnapshot } from "../sdui/types";

export interface ThemeState {
  theme: ThemeSnapshot | null;
  typography: TypographySnapshot | null;
  uiChoices: UiChoicesSnapshot | null;
}

export interface ThemeStore {
  get(): ThemeState;
  setTheme(theme: ThemeSnapshot): void;
  setTypography(typography: TypographySnapshot): void;
  setUiChoices(choices: UiChoicesSnapshot | null): void;
  subscribe(listener: () => void): () => void;
}

export function createThemeStore(
  root: { style: CSSStyleDeclaration } = document.documentElement,
): ThemeStore {
  let state: ThemeState = { theme: null, typography: null, uiChoices: null };
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of [...listeners]) listener();
  };

  return {
    get: () => state,
    setTheme(theme) {
      installVariables(root.style, themeCssVariables(theme));
      state = { ...state, theme };
      notify();
    },
    setTypography(typography) {
      installVariables(root.style, typographyCssVariables(typography));
      state = { ...state, typography };
      notify();
    },
    setUiChoices(uiChoices) {
      state = { ...state, uiChoices };
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
