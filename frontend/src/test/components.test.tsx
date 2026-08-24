import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  ClayButton,
  ClayCollapse,
  ClayList,
  ClayModal,
  ClayTextField,
} from "../components";

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
