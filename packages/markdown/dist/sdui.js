import { markdownPolicyForDocument } from "./index.js";

const DEFAULT_STATUS = Object.freeze({
  mode: "markdown",
  parse: "markdown-it registered",
  decorations: "published",
  preview: "decorated-editor"
});

const HIGHLIGHTING_STATUS = Object.freeze({
  full: Object.freeze({
    parse: "full document syntax current",
    decorations: "full document decorations current"
  }),
  windowed: Object.freeze({
    parse: "windowed visible syntax current",
    decorations: "visible and near-viewport chunks current"
  }),
  degraded: Object.freeze({
    parse: "degraded; visible syntax refresh delayed",
    decorations: "partial decorations; off-viewport chunks evicted"
  }),
  "plain-text-fallback": Object.freeze({
    parse: "plain text fallback; Markdown parser paused",
    decorations: "plain text fallback; syntax decorations cleared"
  })
});

function sanitizeStatusText(value, fallback) {
  const text = String(value ?? fallback)
    .replace(/[\u0000-\u001f\u007f]+/g, " ")
    .replace(/[A-Za-z]:[\\/][^\s]+/g, "[path]")
    .replace(/(?:~|\.\.?)[\\/][^\s]+/g, "[path]")
    .replace(/\b[\w.-]+(?:[\\/][\w.-]+)+\b/g, "[path]")
    .trim();
  return text.length > 0 ? text.slice(0, 160) : fallback;
}

function sanitizeDocumentLabel(value) {
  const raw = String(value ?? "sample.md").replace(/[\u0000-\u001f\u007f]+/g, " ").trim();
  const base = raw.split(/[\\/]/).filter(Boolean).pop() ?? "sample.md";
  return base.slice(0, 80) || "sample.md";
}

function hasPolicyInput(options = {}) {
  return options.documentByteLength !== undefined
    || options.fileSizeBytes !== undefined
    || options.byteLength !== undefined
    || options.budgetExceeded === true
    || options.syntaxBudgetExceeded === true
    || options.memoryBudgetExceeded === true
    || options.parserTimedOut === true
    || options.parseTimedOut === true;
}

export function markdownStatusForPolicy(options = {}) {
  const policy = markdownPolicyForDocument(options);
  if (!hasPolicyInput(options)) {
    return {
      ...DEFAULT_STATUS,
      highlightingState: "full",
      fileTier: "small",
      memoryBudgetBytes: policy.memoryBudgetBytes
    };
  }
  const status = HIGHLIGHTING_STATUS[policy.highlightingState] ?? DEFAULT_STATUS;
  return {
    ...DEFAULT_STATUS,
    parse: status.parse,
    decorations: status.decorations,
    highlightingState: policy.highlightingState,
    fileTier: policy.tier,
    memoryBudgetBytes: policy.memoryBudgetBytes
  };
}

export function markdownPreviewStatusModel(options = {}) {
  const policyStatus = markdownStatusForPolicy(options);
  const statusOptions = options.status ?? {};
  const status = {
    ...policyStatus,
    mode: sanitizeStatusText(statusOptions.mode, policyStatus.mode),
    parse: sanitizeStatusText(statusOptions.parse, policyStatus.parse),
    decorations: sanitizeStatusText(statusOptions.decorations, policyStatus.decorations),
    preview: sanitizeStatusText(statusOptions.preview, policyStatus.preview)
  };
  return {
    documentId: Number(options.documentId ?? 1),
    documentVersion: Number(options.documentVersion ?? 1),
    documentPath: sanitizeDocumentLabel(options.documentPath ?? options.path ?? "sample.md"),
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
    defineLabel({ id: "markdown.policy-status", text: `Highlighting: ${model.status.highlightingState}` }),
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
                    detail: `Markdown syntax policy: ${model.status.fileTier} file, ${model.status.highlightingState}`
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
