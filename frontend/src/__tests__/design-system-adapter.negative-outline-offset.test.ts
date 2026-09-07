import { describe, it, expect } from "vitest";

import { variableToCssValue } from "../theme/design-system-adapter";

describe("design-system adapter dimension bounds", () => {
  it("allows negative outline-offset (inset focus outline, glass recipes)", () => {
    expect(
      variableToCssValue(
        { type: "dimension", value: -2 },
        "tab.default.item.focus.outlineOffset",
      ),
    ).toBe("-2px");
  });
  it("still rejects negative sizes on other dimensions", () => {
    expect(
      variableToCssValue(
        { type: "dimension", value: -2 },
        "button.default.root.rest.gap",
      ),
    ).toBeNull();
  });
});
