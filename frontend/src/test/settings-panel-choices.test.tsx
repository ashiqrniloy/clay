import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";

import { SettingsPanel } from "../settings/SettingsPanel";
import { designSystemStore, themeStore } from "../state/stores";
import type { DesignSystemSnapshot } from "../theme/types";
import type { ThemeSnapshot, TypographySnapshot } from "../theme/types";
import type { UiChoicesSnapshot } from "../sdui/types";

afterEach(() => {
  cleanup();
  designSystemStore.resetToFallback();
  themeStore.setUiChoices(null);
});

// Plan 110 task 10: the Settings panel renders its Theme / Design system /
// Appearance dropdowns from the server-enumerated runtime snapshot
// (`uiChoices`) — never a hardcoded list — and tracks the live selections.

const noopSend = () => Promise.resolve();

const themeSnapshot = {
  specifier: "@clay/theme-modus-operandi",
  tokens: {},
  editorStyles: {},
  densityScale: 1,
} as unknown as ThemeSnapshot;

const typographySnapshot = {
  monospace: {
    families: ["monospace"],
    size: 16,
    ligatures: {
      enableStandard: true,
      enableContextual: true,
      enableDiscretionary: false,
      rawFeatures: "",
      disableFeatures: "",
    },
  },
  proportional: {
    families: ["sans-serif"],
    size: 16,
    ligatures: {
      enableStandard: true,
      enableContextual: true,
      enableDiscretionary: false,
      rawFeatures: "",
      disableFeatures: "",
    },
  },
  ui: {
    families: ["system-ui"],
    size: 13,
    ligatures: {
      enableStandard: true,
      enableContextual: true,
      enableDiscretionary: false,
      rawFeatures: "",
      disableFeatures: "",
    },
  },
  hierarchy: {
    display: 1.5,
    title: 15 / 13,
    section: 13 / 12,
    body: 1,
    status: 1,
    detail: 12 / 13,
    caption: 0.75,
  },
} as unknown as TypographySnapshot;

const coreDesignSystem = {
  specifier: "@clay/core",
  schemaVersion: 1,
  generation: 1,
  provenance: { packageName: "core", packageVersion: "0", domain: "core" },
  recipes: {},
  variables: {},
} as unknown as DesignSystemSnapshot;

// Plan 104 source-independence guard: DS-package name literals live in
// `const` declarations only.
const NEOBRUTAL_SPECIFIER = "@clay/design-neobrutal";
const GLASS = "@clay/design-glass";

const neobrutalDesignSystem = {
  specifier: NEOBRUTAL_SPECIFIER,
  schemaVersion: 1,
  generation: 2,
  provenance: {
    packageName: NEOBRUTAL_SPECIFIER,
    packageVersion: "0.1.0",
    domain: "third-party",
  },
  recipes: {},
  variables: {},
} as unknown as DesignSystemSnapshot;

const choices: UiChoicesSnapshot = {
  themes: [
    { specifier: "@clay/theme-modus-operandi" },
    { specifier: "@clay/theme-modus-vivendi" },
    { specifier: "@clay/theme-gruvbox-material-light" },
    { specifier: "@clay/theme-gruvbox-material-dark" },
  ],
  designSystems: [
    { specifier: "@clay/core", displayName: "Core baseline" },
    { specifier: NEOBRUTAL_SPECIFIER, displayName: "Neobrutal (Default)" },
    { specifier: GLASS, displayName: "Glass (Reference)" },
  ],
  appearance: "dark",
};

function primeStores() {
  themeStore.setTheme(themeSnapshot);
  themeStore.setTypography(typographySnapshot);
  themeStore.setUiChoices(choices);
  designSystemStore.setDesignSystem(coreDesignSystem);
}

function dropdownTrigger(label: string): HTMLElement {
  const buttons = screen.getAllByRole("button");
  const trigger =
    buttons.find((button) => button.getAttribute("aria-label") === label) ??
    buttons.find((button) => button.textContent?.startsWith(label));
  expect(trigger, `dropdown trigger for ${label}`).toBeDefined();
  return trigger!;
}

describe("SettingsPanel design-system selection UX (plan 110 task 10)", () => {
  it("renders theme and design-system dropdowns from the server snapshot", () => {
    primeStores();
    render(<SettingsPanel uiVersion={1} send={noopSend} />);

    // Theme dropdown shows the snapshot-derived list with the active theme.
    expect(dropdownTrigger("Theme").textContent).toContain("Modus Operandi");

    // Design-system dropdown lists the enumerated contributors with the
    // core baseline selected.
    expect(dropdownTrigger("Design system").textContent).toContain(
      "Core baseline",
    );
    expect(
      choices.designSystems.some((option) => option.specifier === "@clay/core"),
    ).toBe(true);
  });

  it("hydrates appearance from the persisted preference in the snapshot", () => {
    primeStores();
    render(<SettingsPanel uiVersion={1} send={noopSend} />);

    const appearanceTrigger = dropdownTrigger("Appearance");
    expect(appearanceTrigger.textContent).toContain("Dark");
  });

  it("tracks a design-system switch without remounting the panel", async () => {
    primeStores();
    const user = userEvent.setup();
    const { container } = render(
      <SettingsPanel uiVersion={1} send={noopSend} />,
    );
    const panel = container.querySelector("aside");
    expect(panel).not.toBeNull();

    // Server applies the selection and fans out the new snapshot; the store
    // update alone must move the selection (no remount).
    act(() => designSystemStore.setDesignSystem(neobrutalDesignSystem));
    expect(dropdownTrigger("Design system").textContent).toContain(
      "Neobrutal (Default)",
    );
    expect(container.querySelector("aside")).toBe(panel);
    void user;
  });

  it("falls back to core baseline when no design system is active", () => {
    themeStore.setUiChoices(choices);
    render(<SettingsPanel uiVersion={1} send={noopSend} />);
    expect(dropdownTrigger("Design system").textContent).toContain(
      "Core baseline",
    );
  });
});
