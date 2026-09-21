// Plan 132 module 07 T24 negative: unknown wrap policy must be rejected and
// the previous layout kept (the lane publishes nothing).
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "galley" });
