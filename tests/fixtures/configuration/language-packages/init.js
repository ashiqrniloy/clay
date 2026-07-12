// First-party language package smoke fixture.
// ~/.config/clay/init.js equivalent: opt in to Rust/TypeScript/JavaScript
// major modes, commands, completion providers, and status items, plus the
// Markdown mode, commands, and decoration/preview parse handler — all through
// one-line `loadPackage("@clay/<lang>")` calls with no per-facade plumbing.
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");

// Opening a .rs, .ts, or .js file from a configured workspace root will
// classify and activate the corresponding package-declared major mode; a .md
// file activates the Markdown mode with Tier 1 native highlighting plus the
// package-JS preview SDUI panel. The fixture stops at the one-line loads so
// the same init.js works both as a manual smoke entry point and as an
// automated test configuration root.
