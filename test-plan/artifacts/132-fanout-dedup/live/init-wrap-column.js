// Plan 132 module 07 T23/T24 fixture: column wrap override through the
// editor_layouts StateFanout lane.
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "column", columnCap: 100 });
