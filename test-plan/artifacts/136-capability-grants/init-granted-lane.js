// Plan 136 task 6 fixture config: the harness installs, adopts, grants, and
// enables `@fixture/lane` through the host CLI verbs (plan 136 task 5), so this
// config only loads the package.
//
// The package itself registers its `lane` mode pattern, its slow parse handler,
// and its module-backed completion provider; the live completion trigger is the
// `.` character those declarations carry (`editorRules.autocompleteTriggers` and
// `completionProviders[].triggerCharacters`). Package-owned modes do not route
// the built-in `completion.trigger` command, so a `bindKey` chord is rejected as
// an unknown command there.
//
// The parse handler holds the general lane for the fixture's own default
// (1000 ms): `parser.js` reads `globalThis.clayLaneBusyMs` inside the package
// isolate, which configuration code cannot reach from its own domain.
import { loadPackage } from "clay:packages";

await loadPackage("@fixture/lane");
