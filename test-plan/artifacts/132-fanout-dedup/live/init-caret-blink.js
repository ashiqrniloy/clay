// Plan 132 module 07 fixture: user typography pin plus an explicit caret style.
// Both ride migrated lanes (typography = ActiveTypographyState fanout,
// caret = caret_styles StateFanout).
import { clientSetCursorStyle } from "clay:editor";
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["FiraCode Nerd Font Mono", "monospace"], size: 16 },
  proportional: { families: ["sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
  hierarchy: {
    display: 1.5, title: 14 / 12, section: 13 / 12, body: 1,
    status: 1, detail: 10 / 12, caption: 0.75,
  },
});

clientSetCursorStyle({ shape: "block", blink: "blink", widthPx: 2.5, hollow: true });
