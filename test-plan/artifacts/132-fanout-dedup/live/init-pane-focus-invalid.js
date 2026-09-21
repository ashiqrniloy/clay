// Plan 132 module 13 S17 negative: invalid policy must be rejected and the
// previous working configuration preserved.
import { setPaneFocusPolicy } from "clay:shell";

setPaneFocusPolicy({ paneFocusPolicy: "hover" });
