import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { themeStore } from "../state/stores";
import { SettingsPanel } from "./SettingsPanel";

afterEach(cleanup);

beforeEach(() => {
  themeStore.setTheme({
    specifier: "@clay/theme-modus-vivendi",
    tokens: {},
    densityScale: 1,
  });
  themeStore.setTypography({
    revision: 1,
    monospace: {
      families: ["Mono"],
      size: 16,
      ligatures: { enableStandard: true },
    },
    proportional: {
      families: ["Sans"],
      size: 16,
      ligatures: { enableStandard: true },
    },
    ui: { families: ["UI"], size: 12, ligatures: { enableStandard: true } },
    hierarchy: {
      display: 1.5,
      title: 1.16,
      section: 1.08,
      body: 1,
      status: 1,
      detail: 0.83,
      caption: 0.75,
    },
  });
});

describe("SettingsPanel", () => {
  it("sends one complete bounded typography transaction", async () => {
    const sent: string[] = [];
    const user = userEvent.setup();
    render(
      <SettingsPanel
        uiVersion={9}
        send={async (payload) => {
          sent.push(payload);
        }}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Typography" }));
    await user.clear(screen.getByRole("textbox", { name: "UI size" }));
    await user.type(screen.getByRole("textbox", { name: "UI size" }), "14");
    await user.click(screen.getByRole("button", { name: "Apply typography" }));
    const message = JSON.parse(sent.at(-1) ?? "{}");
    expect(message.payload.intent.commandId).toBe("settings.setTypography");
    const raw = message.payload.intent.arguments.find(
      (argument: { name: string }) => argument.name === "typography",
    ).value.string;
    expect(JSON.parse(raw).ui).toEqual({ families: ["UI"], size: 14 });
    expect(document.body.innerHTML).not.toContain("secret");
  });

  it("disables apply for invalid profile bounds", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel uiVersion={9} send={async () => undefined} />);
    await user.click(screen.getByRole("button", { name: "Typography" }));
    await user.clear(screen.getByRole("textbox", { name: "UI size" }));
    await user.type(screen.getByRole("textbox", { name: "UI size" }), "200");
    expect(
      screen.getByRole("button", { name: "Apply typography" }),
    ).toBeDisabled();
  });
});
