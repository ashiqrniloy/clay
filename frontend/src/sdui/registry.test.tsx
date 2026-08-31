// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PackageSurfaceView } from "./registry";
import type { PackageSurface } from "./types";

afterEach(cleanup);

function surface(greeting: string): PackageSurface {
  return {
    id: "chat.entry",
    actionTargets: ["chat.submit"],
    provenance: {
      packageName: "@clay/chat",
      packageVersion: "0.1.0",
      apiPrefix: "chat",
      trustDomain: "trusted",
    },
    component: {
      id: "chat.root",
      kind: "panel",
      title: "Chat",
      children: [
        { id: "chat.greeting", kind: "label", text: greeting },
        {
          id: "chat.section",
          kind: "collapse",
          title: "Details",
          children: [{ id: "chat.detail", kind: "label", text: "Body" }],
        },
        {
          id: "chat.composer",
          kind: "textInput",
          title: "Message",
          action: { commandId: "chat.submit" },
        },
      ],
    },
  };
}

describe("package component registry", () => {
  it("preserves keyed input and disclosure state across server property updates", () => {
    const { rerender } = render(
      <PackageSurfaceView
        surface={surface("First")}
        uiVersion={4}
        send={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText("Message"), {
      target: { value: "draft" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText("Body")).toBeVisible();

    rerender(
      <PackageSurfaceView
        surface={surface("Updated")}
        uiVersion={5}
        send={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Message")).toHaveValue("draft");
    expect(screen.getByText("Body")).toBeVisible();
    expect(screen.getByText("Updated")).toBeVisible();
  });

  it("routes text input through one inert typed action", () => {
    const send = vi.fn<(payload: string) => Promise<void>>(
      async () => undefined,
    );
    render(
      <PackageSurfaceView
        surface={surface("Ready")}
        uiVersion={9}
        send={send}
      />,
    );
    const input = screen.getByLabelText("Message");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const payload = JSON.parse(String(send.mock.calls[0]?.[0]));
    expect(payload).toMatchObject({
      family: "sduiAction",
      payload: {
        uiVersion: 9,
        intent: {
          commandId: "chat.submit",
          arguments: [
            { name: "value", value: { string: "hello" } },
            { name: "text", value: { string: "hello" } },
          ],
        },
      },
    });
  });

  it("renders provenance as text and never interprets package text as HTML", () => {
    const hostile = surface("<script>window.pwned = true</script>");
    render(
      <PackageSurfaceView surface={hostile} uiVersion={1} send={vi.fn()} />,
    );
    expect(
      screen.getByText("Provided by @clay/chat (trusted package)"),
    ).toBeVisible();
    expect(
      screen.getByText("<script>window.pwned = true</script>"),
    ).toBeVisible();
    expect(document.querySelector("script")).toBeNull();
  });

  it("applies host data attributes for known kinds and slots while denying unvalidated variants", () => {
    const withVariants: PackageSurface = {
      id: "pkg.surface",
      actionTargets: [],
      provenance: {
        packageName: "@clay/custom",
        packageVersion: "0.1.0",
        apiPrefix: "custom",
        trustDomain: "trusted",
      },
      component: {
        id: "root.panel",
        kind: "panel",
        title: "Test Panel",
        style: {
          variant: "primary" as unknown as undefined,
        },
        children: [
          {
            id: "btn.valid",
            kind: "button",
            label: "Valid Variant",
            action: { commandId: "custom.action" },
            style: {
              variant: "danger" as unknown as undefined,
            },
          },
          {
            id: "btn.hostile",
            kind: "button",
            label: "Hostile Variant",
            action: { commandId: "custom.action" },
            style: {
              variant: "malicious-unregistered-variant" as unknown as undefined,
            },
          },
        ],
      },
    };

    render(
      <PackageSurfaceView
        surface={withVariants}
        uiVersion={1}
        send={vi.fn()}
      />,
    );

    const panel = screen.getByRole("region", { name: "Test Panel" });
    expect(panel).toHaveAttribute("data-clay-component", "panel");
    expect(panel).toHaveAttribute("data-clay-slot", "root");
    expect(panel).toHaveAttribute("data-variant", "primary");

    const validBtn = screen.getByRole("button", { name: "Valid Variant" });
    expect(validBtn).toHaveAttribute("data-clay-component", "button");
    expect(validBtn).toHaveAttribute("data-clay-slot", "root");
    expect(validBtn).toHaveAttribute("data-variant", "danger");

    const hostileBtn = screen.getByRole("button", { name: "Hostile Variant" });
    expect(hostileBtn).toHaveAttribute("data-clay-component", "button");
    expect(hostileBtn).toHaveAttribute("data-clay-slot", "root");
    // Unregistered / hostile variant is denied and stripped:
    expect(hostileBtn).not.toHaveAttribute("data-variant");
  });

  it("preserves SDUI tree state across runtime recipe variable updates", () => {
    const { rerender } = render(
      <PackageSurfaceView
        surface={surface("Interactive Tree")}
        uiVersion={1}
        send={vi.fn()}
      />,
    );

    const input = screen.getByLabelText("Message");
    fireEvent.change(input, { target: { value: "my-draft-message" } });
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText("Body")).toBeVisible();

    // Simulate recipe CSS variable updates on document root
    document.documentElement.style.setProperty(
      "--clay-ds-panel-default-root-rest-border-radius",
      "8px",
    );
    document.documentElement.style.setProperty(
      "--clay-ds-text-input-default-input-rest-border-width",
      "2px",
    );

    rerender(
      <PackageSurfaceView
        surface={surface("Interactive Tree")}
        uiVersion={1}
        send={vi.fn()}
      />,
    );

    // State is fully preserved
    expect(screen.getByLabelText("Message")).toHaveValue("my-draft-message");
    expect(screen.getByText("Body")).toBeVisible();

    document.documentElement.style.removeProperty(
      "--clay-ds-panel-default-root-rest-border-radius",
    );
    document.documentElement.style.removeProperty(
      "--clay-ds-text-input-default-input-rest-border-width",
    );
  });
});
