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
});
