const DEFAULT_STATUS = Object.freeze({
  mode: "markdown",
  parse: "markdown-it registered",
  decorations: "published",
  preview: "decorated-editor"
});

export function markdownPreviewStatusModel(options = {}) {
  const status = { ...DEFAULT_STATUS, ...(options.status ?? {}) };
  return {
    documentId: Number(options.documentId ?? 1),
    documentVersion: Number(options.documentVersion ?? 1),
    documentPath: String(options.documentPath ?? "sample.md"),
    previewEnabled: Boolean(options.previewEnabled ?? true),
    status
  };
}

export function buildMarkdownPreviewStatusTree(claySdui, options = {}) {
  const model = markdownPreviewStatusModel(options);
  const {
    defineButton,
    defineEditorView,
    defineFlex,
    defineLabel,
    defineList,
    definePanel,
    defineStack
  } = claySdui;

  const statusLines = [
    defineLabel({ id: "markdown.mode-status", text: `Mode: ${model.status.mode}` }),
    defineLabel({ id: "markdown.parse-status", text: `Parse: ${model.status.parse}` }),
    defineLabel({ id: "markdown.decoration-status", text: `Decorations: ${model.status.decorations}` }),
    defineLabel({
      id: "markdown.preview-status",
      text: model.previewEnabled ? "Preview: decorated editor" : "Preview: hidden"
    })
  ];

  return defineFlex({
    id: "markdown.root",
    direction: "row",
    children: [
      definePanel({
        id: "markdown.preview-panel",
        title: "Markdown Preview",
        children: [
          defineStack({
            id: "markdown.preview-stack",
            children: [
              defineLabel({ id: "markdown.document", text: `Document: ${model.documentPath}` }),
              ...statusLines,
              defineButton({
                id: "markdown.toggle-preview",
                label: "Toggle Preview",
                action: { commandId: "markdown.togglePreview" }
              }),
              defineList({
                id: "markdown.preview-items",
                items: [
                  {
                    id: "markdown.preview-mode",
                    label: "Decorated editor preview",
                    detail: "Rendered by native SDUI + decoration primitives"
                  }
                ]
              })
            ]
          })
        ]
      }),
      defineEditorView({
        id: "markdown.editor",
        documentId: model.documentId,
        expectedVersion: model.documentVersion
      })
    ]
  });
}

export async function publishMarkdownPreviewStatus(clay, options = {}) {
  const tree = buildMarkdownPreviewStatusTree(clay.sdui, options);
  await clay.sdui.publishTree(tree);
  return markdownPreviewStatusModel(options);
}
