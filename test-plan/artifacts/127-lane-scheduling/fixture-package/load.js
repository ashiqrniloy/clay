// Plan 127 lane-scheduling manual fixture: one third-party package with a slow
// parse handler (general lane) and a module-backed completion provider (latency
// lane).
//
// The parse handler holds the third-party *general* lane for BUSY_MS per parse
// request. The completion provider resolves through `moduleSpecifier`, so it is
// materialized in the third-party *latency* lane and must keep answering while
// the general lane is held. `fixture-package-inline` is the same package with a
// module-object provider instead: that one shares the general lane and visibly
// waits.
import { serverRegisterCompletionProvider } from "clay:completion";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterParseHandler } from "clay:parse";
import * as parser from "./parser.js";
import * as provider from "./provider.js";

export async function loadLaneFixture() {
  // A third-party package's manifest contributions are not applied by the host
  // (only trusted/bundled records take that path), so the mode pattern the
  // document needs to classify as `lane` is registered explicitly here — the
  // `mode-registration` grant is what makes this call legal.
  serverRegisterModePattern({
    modeId: "lane",
    displayName: "Lane Fixture",
    extensions: ["lane"],
    editorRules: { autocompleteTriggers: [{ trigger: "." }] },
  });
  serverRegisterParseHandler({
    mode: "lane",
    module: parser,
    exportName: "parseLaneDocument",
    parseUnit: "line-group",
    viewportPriority: true,
    // The harness budget only has to exceed the deliberate hold below.
    timeoutMs: 5000,
  });
  // `module` binds the handler in this isolate and marks the registration as
  // runtime-bridged (the op only creates a JS registration when the facade
  // sees a `module`); `moduleSpecifier` is what lets the latency lane
  // materialize the same handler by import. Both are required: a
  // specifier-only call registers nothing, and a module-only call stays on
  // the general lane.
  serverRegisterCompletionProvider({
    module: provider,
    moduleSpecifier: import.meta.resolve("./provider.js"),
    exportName: "provideCompletion",
  });
  return {
    packageName: "@fixture/lane",
    packageVersion: "0.1.0",
    packagePrefix: "lane",
  };
}

export default loadLaneFixture;