// Phase 18.14 language package expansion smoke fixture.
// ~/.config/clay/init.js equivalent: opt in to Rust/TypeScript/JavaScript
// major modes, commands, completion providers, and status items.
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

// Opening a .rs, .ts, or .js file from a configured workspace root will
// classify and activate the corresponding package-declared major mode.
// The fixture stops at the one-line loads so the same init.js works both as
// a manual smoke entry point and as an automated test configuration root.
