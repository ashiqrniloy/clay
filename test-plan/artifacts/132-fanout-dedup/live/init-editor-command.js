// Plan 132 module 04/10 fixture: one advisory editor command through the
// editor_commands Fanout lane (no replay store). Called from init.js, so the
// first run happens before the client subscribes — the drop is the expected
// Advice semantics; a config reload after connect delivers it.
import { clientExecuteEditorCommand } from "clay:editor";

clientExecuteEditorCommand({ commandId: "editor.clientMoveCursor.nextWordStart" });
