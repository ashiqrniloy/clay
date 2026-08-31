import { setTheme } from "clay:theme";

// Valid baseline: the capture harness replaces this file with an invalid
// selection after the connected shell is visible, exercising reload-time
// failure instead of a failed first boot.
setTheme("@clay/theme-gruvbox-material-dark");
