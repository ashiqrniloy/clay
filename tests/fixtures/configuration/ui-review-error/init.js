import { setTheme } from "clay:theme";

// Deliberately invalid configuration: Clay should retain a usable shell and
// surface a sanitized runtime diagnostic instead of failing the GUI process.
setTheme("@clay/does-not-exist");
