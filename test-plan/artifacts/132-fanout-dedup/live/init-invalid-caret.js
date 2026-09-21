// Plan 132 module 07 negative fixture: deny-by-default caret shape. The
// caret_styles lane must publish nothing and the editor must stay up.
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ shape: "triangle" });
