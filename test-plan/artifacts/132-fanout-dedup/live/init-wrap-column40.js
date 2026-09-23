// Plan 132 module 07 T23/T24 probe fixture: a small column cap so the wrap
// point is unmistakable in a screenshot.
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "column", columnCap: 40 });
