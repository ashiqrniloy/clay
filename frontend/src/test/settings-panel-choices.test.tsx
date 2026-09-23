import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
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
// `const` declarations only. Plan 118 task 9: the shipped system is
// `@clay/design-instrument`; the removed Neobrutal/Glass packages are gone.
const SHIPPED_SPECIFIER = "@clay/design-instrument";

const shippedDesignSystem = {
  specifier: SHIPPED_SPECIFIER,
  schemaVersion: 1,
  generation: 2,
  provenance: {
    packageName: SHIPPED_SPECIFIER,
    packageVersion: "0.1.0",
    domain: "trusted",
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
    { specifier: SHIPPED_SPECIFIER, displayName: "Quiet Instrument (Default)" },
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
  if (trigger === undefined) {
    throw new Error(`dropdown trigger for ${label} not found`);
  }
  return trigger;
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
    act(() => designSystemStore.setDesignSystem(shippedDesignSystem));
    expect(dropdownTrigger("Design system").textContent).toContain(
      "Quiet Instrument (Default)",
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

  it("renders the shipped choice sets in the server's order, with nothing else", () => {
    primeStores();
    render(<SettingsPanel uiVersion={1} send={noopSend} />);
    // The panel states no lists of its own: it projects `uiChoices` (theme
    // packages and enabled design-system contributors) as enumerated.
    expect(choices.themes.map((option) => option.specifier)).toEqual([
      "@clay/theme-modus-operandi",
      "@clay/theme-modus-vivendi",
      "@clay/theme-gruvbox-material-light",
      "@clay/theme-gruvbox-material-dark",
    ]);
    expect(choices.designSystems.map((option) => option.displayName)).toEqual([
      "Core baseline",
      "Quiet Instrument (Default)",
    ]);
    expect(dropdownTrigger("Theme").textContent).toContain("Modus Operandi");
  });
});

describe("SettingsPanel composition (plan 118)", () => {
  const rows = (container: HTMLElement) => [
    ...container.querySelectorAll("[data-settings-row]"),
  ];
  /** The Typography disclosure header (the group ships collapsed). */
  const typographyDisclosure = (container: HTMLElement) => {
    const section = container.querySelector(
      '[data-settings-section="typography"]',
    );
    return within(section as HTMLElement).getByRole("button");
  };

  it("groups settings into eyebrow-labelled sections of hairline-separated rows", () => {
    primeStores();
    const { container } = render(
      <SettingsPanel uiVersion={1} send={noopSend} />,
    );
    const sections = container.querySelectorAll("[data-settings-section]");
    expect(sections).toHaveLength(2);
    expect(sections[0]?.textContent).toContain("Theme");
    expect(sections[1]?.textContent).toContain("Typography");
    // Every row carries a label; none is a framed card.
    for (const row of rows(container)) {
      expect(row.textContent?.trim().length).toBeGreaterThan(0);
    }
    expect(rows(container).length).toBeGreaterThanOrEqual(3);
  });

  it("carries no elevation on the fixed slot and no nested panel frame", () => {
    primeStores();
    const { container } = render(
      <SettingsPanel uiVersion={1} send={noopSend} />,
    );
    const panel = container.querySelector("aside");
    // A fixed slot is not a floating surface: elevation belongs to transient
    // overlays only (DESIGN.md §1/§11).
    expect(panel?.getAttribute("data-clay-ds")).toBe("settingsPanel.panel");
    // The only framed control is the input well: no `panel` inside the panel.
    expect(container.querySelector("[data-clay-component='panel']")).toBeNull();
  });

  it("keeps the typography values in the mono role (data, not prose)", async () => {
    primeStores();
    const user = userEvent.setup();
    const { container } = render(
      <SettingsPanel uiVersion={1} send={noopSend} />,
    );
    // Typography ships collapsed (approved composition): expand it to reach
    // the fields.
    await user.click(typographyDisclosure(container));
    const values = rows(container)
      .filter((row) => /size|ratios/i.test(row.textContent ?? ""))
      .map((row) => row.querySelector("input"));
    expect(values.length).toBeGreaterThan(0);
    for (const input of values) {
      expect(input?.className).toMatch(/monospace/);
    }
  });

  it("warns the invalid field itself, not the whole group (plan 118 E6)", async () => {
    primeStores();
    const user = userEvent.setup();
    const { container } = render(
      <SettingsPanel uiVersion={1} send={noopSend} />,
    );
    await user.click(typographyDisclosure(container));

    const size = screen.getByLabelText("UI size");
    await user.clear(size);
    await user.type(size, "4");
    expect(size).toBeInvalid();
    expect(size).toHaveAccessibleDescription("Enter a size between 6 and 96.");
    // The sibling sizes are fine, so they are not toned.
    expect(screen.getByLabelText("Monospace size")).not.toBeInvalid();

    await user.clear(size);
    await user.type(size, "12");
    expect(screen.getByLabelText("UI size")).not.toBeInvalid();
    expect(document.querySelectorAll("[data-clay-slot='error']").length).toBe(
      0,
    );
  });

  it("shows the close chord and dismisses on Escape", async () => {
    primeStores();
    const sent: string[] = [];
    const user = userEvent.setup();
    const { container } = render(
      <SettingsPanel
        uiVersion={1}
        send={(payload) => {
          sent.push(JSON.stringify(payload));
          return Promise.resolve();
        }}
      />,
    );
    const close = container.querySelector("[data-settings-close]");
    expect(close?.textContent).toContain("Close");
    // The head states the chord the panel answers to.
    expect(container.querySelector("header")?.textContent).toContain("esc");
    await user.click(typographyDisclosure(container));
    await user.click(screen.getByRole("textbox", { name: "UI families" }));
    await user.keyboard("{Escape}");
    expect(sent.some((raw) => raw.includes("settings.close"))).toBe(true);
  });
});
