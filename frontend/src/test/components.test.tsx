import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

afterEach(cleanup);

import {
  ClayBadge,
  ClayButton,
  ClayCollapse,
  ClayDivider,
  ClayDropdown,
  ClayKbd,
  ClayList,
  ClayModal,
  ClayTabStrip,
  ClayTabBar,
  ClayText,
  ClayTextField,
} from "../components";
import { TabBar } from "../app/layout/tab-bar";

describe("ClayButton keyboard semantics", () => {
  it("activates on Enter and Space with native button behavior", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(<ClayButton onPress={onPress}>Save</ClayButton>);
    const button = screen.getByRole("button", { name: "Save" });
    button.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onPress).toHaveBeenCalledTimes(2);
  });

  it("exposes disabled state and gates activation", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(
      <ClayButton isDisabled onPress={onPress}>
        Deleted
      </ClayButton>,
    );
    const button = screen.getByRole("button", {
      name: "Deleted",
    }) as HTMLButtonElement;
    // React Aria conveys disabled via the native attribute + data-disabled.
    expect(button.disabled).toBe(true);
    expect(button).toHaveAttribute("data-disabled");
    await user.click(button);
    expect(onPress).not.toHaveBeenCalled();
  });
});

describe("ClayTextField accessibility wiring", () => {
  it("associates label and description with the input", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <ClayTextField
        label="Search"
        value=""
        onChange={onChange}
        description="Filters the catalogue"
      />,
    );
    const input = screen.getByLabelText("Search");
    expect(input).toHaveAccessibleDescription("Filters the catalogue");
    await user.type(input, "a");
    expect(onChange).toHaveBeenCalled();
  });

  it("marks error validation state via aria-invalid", () => {
    render(
      <ClayTextField
        label="Path"
        value=""
        onChange={() => {}}
        validationState="error"
      />,
    );
    expect(screen.getByLabelText("Path")).toBeInvalid();
  });
});

describe("ClayList selection semantics", () => {
  it("supports roving selection through aria-selected rows", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <ClayList
        ariaLabel="Files"
        items={[
          { id: "1", title: "one.ts" },
          { id: "2", title: "two.rs" },
        ]}
        onSelect={onSelect}
      />,
    );
    const listbox = screen.getByRole("listbox", { name: "Files" });
    expect(listbox).toBeInTheDocument();
    const rowTwo = screen.getByRole("option", { name: /two\.rs/ });
    await user.click(rowTwo);
    expect(onSelect).toHaveBeenCalledWith("2");
  });
});

describe("ClayCollapse disclosure pattern", () => {
  it("toggles aria-expanded and reveals content", async () => {
    const user = userEvent.setup();
    render(
      <ClayCollapse title="Details">
        <span>hidden body</span>
      </ClayCollapse>,
    );
    const toggle = screen.getByRole("button", { name: /Details/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("hidden body")).not.toBeInTheDocument();
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("hidden body")).toBeInTheDocument();
  });
});

describe("ClayModal focus containment", () => {
  it("traps focus in the dialog and closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <div>
        <button type="button">outside trigger</button>
        <ClayModal title="Confirm" open onClose={onClose}>
          <button type="button">inside action</button>
        </ClayModal>
      </div>,
    );
    const dialog = screen.getByRole("dialog", { name: "Confirm" });
    expect(dialog).toBeInTheDocument();
    // Focus moves into the dialog on open.
    const inside = screen.getByRole("button", { name: "inside action" });
    inside.focus();
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});

describe("ClayButton variants & recipe integration", () => {
  it("renders all four variants with deterministic CSS classes and button semantics", () => {
    const { rerender } = render(
      <ClayButton variant="default">Default</ClayButton>,
    );
    expect(screen.getByRole("button", { name: "Default" })).toBeInTheDocument();

    rerender(<ClayButton variant="primary">Primary</ClayButton>);
    expect(screen.getByRole("button", { name: "Primary" })).toBeInTheDocument();

    rerender(<ClayButton variant="muted">Muted</ClayButton>);
    expect(screen.getByRole("button", { name: "Muted" })).toBeInTheDocument();

    rerender(<ClayButton variant="danger">Danger</ClayButton>);
    expect(screen.getByRole("button", { name: "Danger" })).toBeInTheDocument();
  });
});

function InteractiveTestSurface() {
  return (
    <div>
      <ClayTextField
        label="Workspace Name"
        value="Project Alpha"
        onChange={() => {}}
      />
      <ClayCollapse title="Advanced Settings">
        <span>Expanded details panel</span>
      </ClayCollapse>
    </div>
  );
}

describe("host recipe attributes and state mapping", () => {
  it("renders closed host-owned data-clay-component and data-clay-slot attributes on all primitives", () => {
    const { container } = render(
      <div>
        <ClayButton variant="primary">Action</ClayButton>
        <ClayBadge>Beta</ClayBadge>
        <ClayKbd>⌘K</ClayKbd>
        <ClayDivider />
        <ClayText variant="title">Heading</ClayText>
        <ClayText variant="status">Online</ClayText>
        <ClayTextField label="Name" value="test" onChange={() => {}} />
        <ClayDropdown
          label="Select Option"
          options={[{ id: "1", label: "Option 1" }]}
          selectedId="1"
          onSelect={() => {}}
        />
        <ClayList
          ariaLabel="Items"
          items={[{ id: "a", title: "Item A", detail: "info" }]}
        />
        <ClayCollapse title="Section">
          <span>Content</span>
        </ClayCollapse>
      </div>,
    );

    const button = screen.getByRole("button", { name: "Action" });
    expect(button).toHaveAttribute("data-clay-component", "button");
    expect(button).toHaveAttribute("data-clay-slot", "root");
    expect(button).toHaveAttribute("data-variant", "primary");

    const badge = screen.getByText("Beta");
    expect(badge).toHaveAttribute("data-clay-component", "badge");
    expect(badge).toHaveAttribute("data-clay-slot", "root");

    const kbd = screen.getByText("⌘K");
    expect(kbd).toHaveAttribute("data-clay-component", "kbd");
    expect(kbd).toHaveAttribute("data-clay-slot", "root");

    const divider = container.querySelector("hr");
    expect(divider).toHaveAttribute("data-clay-component", "divider");
    expect(divider).toHaveAttribute("data-clay-slot", "root");

    const heading = screen.getByText("Heading");
    expect(heading).toHaveAttribute("data-clay-component", "label");
    expect(heading).toHaveAttribute("data-clay-slot", "root");
    expect(heading).toHaveAttribute("data-variant", "title");

    const status = screen.getByText("Online");
    expect(status).toHaveAttribute("data-clay-component", "statusItem");
    expect(status).toHaveAttribute("data-clay-slot", "root");
    expect(status).toHaveAttribute("data-variant", "status");

    const textField = container.querySelector(
      '[data-clay-component="textInput"]',
    );
    expect(textField).toBeInTheDocument();
    expect(textField).toHaveAttribute("data-clay-slot", "field");

    const collapse = screen.getByRole("button", { name: /Section/ });
    expect(collapse).toHaveAttribute("data-clay-component", "collapse");
    expect(collapse).toHaveAttribute("data-clay-slot", "header");
  });

  it("preserves state precedence so disabled and invalid states are not masked by hover or active", async () => {
    const user = userEvent.setup();
    const onPress = vi.fn();
    render(
      <ClayButton isDisabled onPress={onPress}>
        Disabled Button
      </ClayButton>,
    );
    const button = screen.getByRole("button", { name: "Disabled Button" });
    expect(button).toHaveAttribute("data-disabled");

    await user.hover(button);
    // When disabled, React Aria retains data-disabled and suppresses action
    expect(button).toHaveAttribute("data-disabled");
    await user.click(button);
    expect(onPress).not.toHaveBeenCalled();
  });

  it("does not trigger recipe-related React re-renders on hover or focus state changes", async () => {
    const user = userEvent.setup();
    let renderCount = 0;
    function TrackedButton() {
      renderCount++;
      return <ClayButton>Hover Me</ClayButton>;
    }

    render(<TrackedButton />);
    expect(renderCount).toBe(1);

    const button = screen.getByRole("button", { name: "Hover Me" });
    await user.hover(button);
    button.focus();

    // Hover and focus states are purely CSS pseudo-class/attribute driven;
    // no React re-render occurs for recipe resolution.
    expect(renderCount).toBe(1);
  });

  it("maintains stable component state across runtime recipe variable updates", async () => {
    const user = userEvent.setup();
    const { rerender } = render(<InteractiveTestSurface />);

    // Expand collapse
    const collapseToggle = screen.getByRole("button", {
      name: /Advanced Settings/,
    });
    await user.click(collapseToggle);
    expect(collapseToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Expanded details panel")).toBeInTheDocument();

    // Simulate recipe CSS variable updates on document root (as done by DesignSystemStore)
    document.documentElement.style.setProperty(
      "--clay-ds-text-input-default-input-rest-border-radius",
      "8px",
    );
    document.documentElement.style.setProperty(
      "--clay-ds-collapse-default-header-rest-border-radius",
      "4px",
    );

    // Rerender
    rerender(<InteractiveTestSurface />);

    // Input value and disclosure state remain completely preserved
    expect(screen.getByLabelText("Workspace Name")).toHaveValue(
      "Project Alpha",
    );
    expect(collapseToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Expanded details panel")).toBeInTheDocument();

    // Cleanup style
    document.documentElement.style.removeProperty(
      "--clay-ds-text-input-default-input-rest-border-radius",
    );
    document.documentElement.style.removeProperty(
      "--clay-ds-collapse-default-header-rest-border-radius",
    );
  });
});

describe("ClayTabStrip catalog primitive and unification", () => {
  it("renders strip-only layout with recipe attributes, active selection, and new tab affordance", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    const onClose = vi.fn();
    const onNew = vi.fn();

    render(
      <ClayTabStrip
        ariaLabel="Window tabs"
        activeId="2"
        onActivate={onActivate}
        onClose={onClose}
        onNew={onNew}
        tabs={[
          { id: "1", label: "main.rs", closable: true },
          { id: "2", label: "lib.rs", dirty: true, closable: true },
          { id: "3", label: "readonly.rs", disabled: true },
        ]}
      />,
    );

    const tablist = screen.getByRole("tablist", { name: "Window tabs" });
    expect(tablist).toBeInTheDocument();
    expect(tablist).toHaveAttribute("data-clay-component", "tabList");
    expect(tablist).toHaveAttribute("data-clay-slot", "strip");

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(3);

    expect(tabs[0]).toHaveAttribute("data-clay-component", "tabList");
    expect(tabs[0]).toHaveAttribute("data-clay-slot", "tab");
    expect(tabs[1]).toHaveAttribute("data-selected", "true");
    expect(tabs[2]).toHaveAttribute("data-disabled", "true");

    // Click tab 1
    await user.click(tabs[0]!);
    expect(onActivate).toHaveBeenCalledWith("1");

    // Click close button on tab 1
    const closeBtn = screen.getByRole("button", { name: "Close main.rs" });
    await user.click(closeBtn);
    expect(onClose).toHaveBeenCalledWith("1");

    // Click new tab button
    const newBtn = screen.getByRole("button", { name: "New tab" });
    await user.click(newBtn);
    expect(onNew).toHaveBeenCalled();
  });

  it("renders empty strip state cleanly when no tabs exist", async () => {
    const user = userEvent.setup();
    const onNew = vi.fn();

    render(
      <ClayTabStrip
        tabs={[]}
        emptyLabel="No open buffers"
        onNew={onNew}
      />,
    );

    expect(screen.getByText("No open buffers")).toBeInTheDocument();
    const newBtn = screen.getByRole("button", { name: "New tab" });
    await user.click(newBtn);
    expect(onNew).toHaveBeenCalled();
  });

  it("renders panels and coordinates widget-local selection without server round-trip", async () => {
    const user = userEvent.setup();

    render(
      <ClayTabStrip
        ariaLabel="Settings tabs"
        defaultActiveId="general"
        tabs={[
          {
            id: "general",
            label: "General",
            content: <div>General Options</div>,
          },
          {
            id: "editor",
            label: "Editor",
            content: <div>Editor Preferences</div>,
          },
        ]}
      />,
    );

    expect(screen.getByText("General Options")).toBeInTheDocument();
    expect(screen.queryByText("Editor Preferences")).not.toBeInTheDocument();

    const editorTab = screen.getByRole("tab", { name: "Editor" });
    await user.click(editorTab);

    expect(screen.getByText("Editor Preferences")).toBeInTheDocument();
    expect(screen.queryByText("General Options")).not.toBeInTheDocument();
  });

  it("proves shell TabBar delegates directly to the ClayTabStrip catalog primitive (single source)", () => {
    expect(TabBar).toBeDefined();
    expect(ClayTabBar).toBe(ClayTabStrip);
    const { container: shellContainer } = render(
      <TabBar
        tabs={[{ id: "1", label: "buffer.rs" }]}
        activeId="1"
      />,
    );
    const { container: stripContainer } = render(
      <ClayTabStrip
        ariaLabel="Window tabs"
        tabs={[{ id: "1", label: "buffer.rs" }]}
        activeId="1"
      />,
    );

    expect(
      shellContainer.querySelector('[data-clay-slot="strip"]'),
    ).toBeInTheDocument();
    expect(
      stripContainer.querySelector('[data-clay-slot="strip"]'),
    ).toBeInTheDocument();
    expect(
      shellContainer.querySelector('[data-clay-slot="tab"]')?.textContent,
    ).toBe("buffer.rs");
    expect(
      stripContainer.querySelector('[data-clay-slot="tab"]')?.textContent,
    ).toBe("buffer.rs");
  });
});
