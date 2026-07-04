// Explicit one-line loading fixture for Phase 18.10 first-party grammar packages.
// ~/.config/clay/init.js equivalent: opt in to syntax highlighting without
// changing the active major mode. Grammar packages are NOT auto-loaded.
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
