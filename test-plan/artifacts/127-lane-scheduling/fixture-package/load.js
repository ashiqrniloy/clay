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
import { serverRegisterParseHandler } from "clay:parse";
import * as parser from "./parser.js";

export async function loadLaneFixture() {
  serverRegisterParseHandler({
    mode: "lane",
    module: parser,
    exportName: "parseLaneDocument",
    parseUnit: "line-group",
    viewportPriority: true,
    // The harness budget only has to exceed the deliberate hold below.
    timeoutMs: 5000,
  });
  serverRegisterCompletionProvider({
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