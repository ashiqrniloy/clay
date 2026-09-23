import {
  defineEditorView,
  defineFlex,
  defineLabel,
  definePanel,
  defineStack,
  publishTree,
} from "clay:sdui";

await publishTree(defineFlex({
  id: "review-loading-root",
  direction: "column",
  children: [
    definePanel({
      id: "review-loading-panel",
      title: "Loading review",
      children: [
        defineStack({
          id: "review-loading-stack",
          children: [
            defineLabel({ id: "review-loading-label", text: "Loading workspace…" }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-loading-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
