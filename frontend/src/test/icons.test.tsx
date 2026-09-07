import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { act, type RenderResult } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

afterEach(() => {
  cleanup();
  iconStore.resetToFallback();
});

import { iconStore } from "../state/stores";
import type { IconPackSnapshot } from "../state/icon-store";
import { ClayButton, ClayIconButton } from "../components/button";
import { ClayDropdown } from "../components/controls";
import { PackageComponent } from "../sdui/registry";
import { ClayIcon } from "../components/icon";

function getIconByName(
  view: RenderResult,
  name: string,
): SVGElement | Element | null {
  return view.container.querySelector(`svg[data-icon-name="${name}"]`);
}

function pack(overrides: Partial<IconPackSnapshot> = {}): IconPackSnapshot {
  return {
    specifier: "@clay/icons-phosphor-regular",
    schemaVersion: 1,
    generation: 5,
    provenance: {
      packageName: "@clay/icons-phosphor-regular",
      packageVersion: "2.0.8",
      apiPrefix: "icons-phosphor-regular",
      trustDomain: "trusted",
    },
    icons: {
      "action.close": {
        viewBox: [0, 0, 256, 256],
        paths: [{ d: "M 4,4 L 20,20 L 4,20 Z" }],
      },
      "disclosure.down": {
        viewBox: [0, 0, 256, 256],
        paths: [{ d: "M 213.66,101.66 L 128,187.31 42.34,101.66 Z" }],
      },
      "disclosure.right": {
        viewBox: [0, 0, 256, 256],
        paths: [{ d: "M 154.34,128 L 68.69,213.66 91.31,236.28 199.66,128 91.31,19.72 68.69,42.34 Z" }],
      },
      "document.save": {
        viewBox: [0, 0, 256, 256],
        paths: [
          { d: "M 8,8 L 8,2 L 18,2 L 18,8 Z", opacity: 0.2 },
          { d: "M 6,2 L 2,6 L 2,22 L 22,22 L 22,6 L 18,2 Z" },
        ],
      },
    },
    ...overrides,
  };
}

describe("ClayIcon geometry rendering", () => {
  it("renders bounded paths in order with viewBox and currentColor fill", () => {
    act(() => iconStore.setIconPack(pack()));
    const view = render(<ClayIcon name="document.save" />);
    const svg = getIconByName(view, "document.save");
    if (!(svg instanceof SVGElement)) throw new Error("icon svg missing");
    expect(svg).toHaveAttribute("viewBox", "0 0 256 256");
    expect(svg).toHaveAttribute("fill", "currentColor");
    const paths = svg.querySelectorAll("path");
    expect(paths).toHaveLength(2);
    // Path order is preserved: duotone shade layer first, outline second.
    expect(paths[0]?.getAttribute("d")).toBe("M 8,8 L 8,2 L 18,2 L 18,8 Z");
    expect(paths[0]?.getAttribute("opacity")).toBe("0.2");
    expect(paths[1]?.getAttribute("d")).toBe("M 6,2 L 2,6 L 2,22 L 22,22 L 22,6 L 18,2 Z");
    expect(paths[1]?.getAttribute("opacity")).toBeNull();
  });

  it("falls back to the bundled Regular subset when no pack is active", () => {
    // No setIconPack call: host fallback subset is the zero-config source.
    const view = render(<ClayIcon name="action.close" />);
    const svg = getIconByName(view, "action.close");
    // Real Phosphor close geometry carries the arc-heavy fallback d-string.
    expect(svg?.querySelector("path")?.getAttribute("d")).toContain(
      "205.66,194.34",
    );
  });

  it("renders duotone packs identically: opacity rides each path", () => {
    act(() =>
      iconStore.setIconPack(pack({ specifier: "@clay/icons-phosphor-duotone" })),
    );
    const view = render(<ClayIcon name="document.save" />);
    expect(getIconByName(view, "document.save")?.querySelectorAll("path")).toHaveLength(2);
    iconStore.resetToFallback();
  });

  it("keeps decorative icons out of the accessibility tree", () => {
    const view = render(<ClayIcon name="action.close" />);
    expect(getIconByName(view, "action.close")).toHaveAttribute("aria-hidden", "true");
  });

  it("exposes informative icons as role=img with one name", () => {
    render(<ClayIcon name="action.close" label="Close" />);
    expect(screen.getByRole("img", { name: "Close" })).toBeTruthy();
  });

  it("renders an empty decorative slot for unknown keys (no broken glyph)", () => {
    const view = render(<ClayIcon name="vendor.nonexistent" />);
    const slot = view.container.querySelector("span[data-clay-component=\"iconSlot\"]");
    expect(slot).not.toBeNull();
    expect(slot?.querySelector("svg")).toBeNull();
    expect(slot).toHaveAttribute("aria-hidden", "true");
  });
});

describe("ClayIconButton accessibility contract", () => {
  it("activates on Enter and Space with keyboard focus", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<ClayIconButton icon="document.save" label="Save" onPress={onPress} />);
    const button = screen.getByRole("button", { name: "Save" });
    button.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onPress).toHaveBeenCalledTimes(2);
  });

  it("exposes exactly one accessible name: the label, not the glyph", () => {
    render(<ClayIconButton icon="document.save" label="Save" />);
    const button = screen.getByRole("button", { name: "Save" });
    // The svg inside is decorative (no competing img name).
    expect(button.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryAllByRole("img")).toHaveLength(0);
  });

  it("gates activation while disabled and conveys the state", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<ClayIconButton icon="document.save" label="Save" isDisabled onPress={onPress} />);
    const button = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    await user.click(button);
    expect(onPress).not.toHaveBeenCalled();
  });

  it("falls back to the visible text label when the icon key is unknown", () => {
    render(<ClayIconButton icon="vendor.nonexistent" label="Archive" />);
    const button = screen.getByRole("button", { name: "Archive" });
    // No blank control: the accessible name is visible text.
    expect(button.textContent).toContain("Archive");
  });

  it("keeps DOM identity and focus across icon-pack switches", () => {
    act(() => iconStore.setIconPack(pack()));
    const onPress = vi.fn();
    render(<ClayIconButton icon="document.save" label="Save" onPress={onPress} />);
    const button = screen.getByRole("button", { name: "Save" });
    button.focus();
    expect(document.activeElement).toBe(button);

    act(() => {
      iconStore.setIconPack(
        pack({ specifier: "@clay/icons-phosphor-duotone", generation: 6 }),
      );
    });
    const after = screen.getByRole("button", { name: "Save" });
    // Same DOM node, still focused, still functional (no remount).
    expect(after).toBe(button);
    expect(document.activeElement).toBe(after);
  });

  it("describes the action through the tooltip on hover and focus", async () => {
    const user = userEvent.setup();
    render(
      <ClayIconButton icon="document.save" label="Save" shortcut="Mod+S" />,
    );
    // Keyboard focus opens the tooltip (programmatic focus is not
    // focus-visible, so simulate Tab).
    await user.tab();
    expect(await screen.findByRole("tooltip", {}, { timeout: 3000 })).toHaveTextContent(
      "Save (Mod+S)",
    );

    // Escape dismisses (WCAG 1.4.13).
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

describe("icon-capable shared controls", () => {
  it("dropdown trigger uses the shared disclosure.down glyph decoratively", () => {
    render(
      <ClayDropdown
        label="Model"
        options={[{ id: "a", label: "Opus" }]}
        selectedId="a"
        onSelect={() => {}}
      />,
    );
    const trigger = screen.getByRole("button", { name: /Model/ });
    const svg = trigger.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute("aria-hidden", "true");
    // No text-glyph remnant.
    expect(trigger.textContent).not.toContain("▾");
  });

  it("icon-only usage inside a labeled button stays decorative", () => {
    render(
      <ClayButton>
        <ClayIcon name="document.save" />
        Save
      </ClayButton>,
    );
    const button = screen.getByRole("button", { name: "Save" });
    expect(button).toBeTruthy();
    expect(screen.queryAllByRole("img")).toHaveLength(0);
  });

  it("projects SDUI button, label, and list icon references", () => {
    const send = vi.fn();
    const view = render(
      <>
        <PackageComponent
          node={{
            id: "b1",
            kind: "button",
            label: "Save",
            icon: "document.save",
            action: { commandId: "doc.save" },
          }}
          uiVersion={1}
          send={send}
        />
        <PackageComponent
          node={{ id: "l1", kind: "label", text: "Ready", icon: "status.success" }}
          uiVersion={1}
          send={send}
        />
        <PackageComponent
          node={{
            id: "ls1",
            kind: "list",
            title: "Files",
            items: [{ id: "f1", label: "main.rs", icon: "file.file" }],
          }}
          uiVersion={1}
          send={send}
        />
      </>,
    );
    // Each declared reference renders its geometry decoratively.
    for (const name of ["document.save", "status.success", "file.file"]) {
      expect(
        view.container.querySelector(`svg[data-icon-name="${name}"]`),
      ).not.toBeNull();
    }
    // Accessible names stay the text labels; glyphs add none.
    expect(
      screen.getByRole("button", { name: "Save" }).querySelector("svg"),
    ).toHaveAttribute("aria-hidden", "true");
    expect(screen.queryAllByRole("img")).toHaveLength(0);
  });
});

describe("plan 112 task 10 cross-layer matrix", () => {
  it("embeds no colors in icon output so themes stay the sole color authority", () => {
    // Duotone pack carries a 0.2-opacity shade layer; even then the rendered
    // output must carry no fill/stroke/style of its own: colorization comes
    // only from the --clay-text-icon core token resolved by the active theme.
    iconStore.setIconPack({
      specifier: "@clay/icons-phosphor-duotone",
      schemaVersion: 1,
      generation: 1,
      provenance: {
        packageName: "icons-phosphor-duotone",
        packageVersion: "0.1.0",
        apiPrefix: "icons-phosphor-duotone",
        trustDomain: "Trusted",
      },
      icons: {
        "action.close": {
          viewBox: [0, 0, 256, 256],
          paths: [
            { d: "M 4 4 L 20 20" },
            { d: "M 40 40 L 56 56", opacity: 0.2 },
          ],
        },
      },
    });
    const view = render(<ClayIcon name="action.close" />);
    const svg = view.container.querySelector("svg");
    if (!(svg instanceof SVGElement)) throw new Error("icon svg missing");
    expect(svg.getAttribute("fill")).toBe("currentColor");
    expect(svg.style.length).toBe(0);
    for (const path of Array.from(svg.querySelectorAll("path"))) {
      expect(path.getAttribute("fill")).toBeNull();
      expect(path.getAttribute("stroke")).toBeNull();
      expect(path.getAttribute("style")).toBeNull();
      expect(path.getAttribute("class")).toBeNull();
    }
    // Opacity is geometry (duotone shade layer), not colorization.
    const shaded = svg.querySelectorAll("path")[1];
    if (!(shaded instanceof SVGElement)) throw new Error("shade path missing");
    expect(shaded.getAttribute("opacity")).toBe("0.2");
  });

  it("keeps action and label semantics identical across the pack matrix", () => {
    // Regular -> duotone -> fallback: the accessible name and press semantics
    // of a migrated control do not vary with the selected pack.
    const onPress = vi.fn();
    const packs: (IconPackSnapshot | null)[] = [
      null,
      {
        specifier: "@clay/icons-phosphor-regular",
        schemaVersion: 1,
        generation: 2,
        provenance: {
          packageName: "icons-phosphor-regular",
          packageVersion: "0.1.0",
          apiPrefix: "icons-phosphor-regular",
          trustDomain: "Trusted",
        },
        icons: {
          "document.save": {
            viewBox: [0, 0, 256, 256],
            paths: [{ d: "M 4 4 L 20 20 Z" }],
          },
        },
      },
      {
        specifier: "@clay/icons-phosphor-duotone",
        schemaVersion: 1,
        generation: 3,
        provenance: {
          packageName: "icons-phosphor-duotone",
          packageVersion: "0.1.0",
          apiPrefix: "icons-phosphor-duotone",
          trustDomain: "Trusted",
        },
        icons: {
          "document.save": {
            viewBox: [0, 0, 256, 256],
            paths: [
              { d: "M 4 4 L 20 20 Z" },
              { d: "M 40 40 L 56 56", opacity: 0.2 },
            ],
          },
        },
      },
    ];
    for (const pack of packs) {
      if (pack) {
        iconStore.setIconPack(pack);
      } else {
        iconStore.resetToFallback();
      }
      const view = render(<ClayIconButton icon="document.save" label="Save" onPress={onPress} />);
      const button = screen.getByRole("button", { name: "Save" });
      fireEvent.click(button);
      expect(onPress).toHaveBeenCalledTimes(1);
      onPress.mockClear();
      view.unmount();
    }
    expect(onPress).not.toHaveBeenCalled();
  });
});

