// Plan 127 lane-scheduling manual fixture (inline-provider half of the A/B).
//
// Same slow parse handler as `fixture-package`, but the completion provider is
// registered as a module object (no moduleSpecifier), so it lives in the
// third-party *general* lane's isolate behind a token-backed closure: it cannot
// answer until the parse handler above releases that lane.
import { serverRegisterCompletionProvider } from "clay:completion";
import { serverRegisterParseHandler } from "clay:parse";
import * as parser from "./parser.js";

export async function loadLaneBlockedFixture() {
  serverRegisterParseHandler({
    mode: "laneblocked",
    module: parser,
    exportName: "parseLaneDocument",
    parseUnit: "line-group",
    viewportPriority: true,
    timeoutMs: 5000,
  });
  serverRegisterCompletionProvider({
    module: {
      provideCompletion: async (_request, _window) => ({
        status: "ok",
        items: [
          {
            label: "lane-inline",
            insertText: "lane-inline",
            detail: "inline module-object provider (general lane)",
          },
        ],
      }),
    },
  });
  return {
    packageName: "@fixture/laneblocked",
    packageVersion: "0.1.0",
    packagePrefix: "laneblocked",
  };
}

export default loadLaneBlockedFixture;
