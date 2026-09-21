// Plan 132 module 07 probe fixture: a grossly different caret shape (8px
// underline) so the lane's delivery is unambiguous in a screenshot.
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ shape: "underline", blink: "solid", widthPx: 8 });
