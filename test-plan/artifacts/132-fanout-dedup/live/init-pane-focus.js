// Plan 132 module 13 S14 fixture: focus-follows-cursor through the
// shell_preferences StateFanout lane (default is click-to-focus).
import { setPaneFocusPolicy } from "clay:shell";

setPaneFocusPolicy({ paneFocusPolicy: "cursor" });
