// Negative fixture: deny-by-default cursor-style enum (module 07 negative check).
import { clientSetCursorStyle } from "clay:editor";

clientSetCursorStyle({ shape: "triangle" });
