// Module-backed completion provider (Plan 127 manual fixture).
//
// Imported by the third-party *latency* lane through the registered
// moduleSpecifier, so it answers while the general lane is held by
// parser.js::parseLaneDocument.
export async function provideCompletion(_request, _window) {
  return {
    status: "ok",
    items: [
      {
        label: "lane-latency",
        insertText: "lane-latency",
        detail: "module-backed provider (latency lane)",
      },
    ],
  };
}

export default provideCompletion;