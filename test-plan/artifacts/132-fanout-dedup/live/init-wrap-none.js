// Plan 132 module 07 T22 A-side: explicit no-wrap override through the
// editor_layouts lane (long line must extend past the pane).
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "none" });
