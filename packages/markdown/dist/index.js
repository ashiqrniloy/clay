import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

export const packageName = "@clay/markdown";
export const apiPrefix = "markdown";
export const modeId = "markdown";

export const supportedExtensions = ["md", "markdown", "mdown"];
export const supportedMimeTypes = ["text/markdown"];

export const markdownSyntaxGrammar = Object.freeze({
  languageId: "markdown",
  filePatterns: { extensions: supportedExtensions },
  grammar: { kind: "native", source: "tree-sitter-md-025" },
  queries: { highlights: "./queries/highlights.scm" },
  styleMap: {
    punctuation: { type: "Operator" },
    text: { type: "Paragraph" },
    code: { type: "CodeBlock", fontRole: "monospace" },
    "code-span": { type: "CodeSpan", fontRole: "monospace" },
    "heading-1": { type: "Heading1" },
    "heading-2": { type: "Heading2" },
    "heading-3": { type: "Heading3" },
    "heading-4": { type: "Heading4" },
    "heading-5": { type: "Heading5" },
    "heading-6": { type: "Heading6" },
    strong: { type: "Paragraph", modifiers: ["Bold"] },
    emphasis: { type: "Paragraph", modifiers: ["Italic"] },
    "list-marker": { type: "ListItem" },
    link: { type: "Link" },
    quote: { type: "Quote" }
  },
  budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
});

export const markdownLargeFilePolicy = Object.freeze({
  smallFileMaxBytes: 1 * 1024 * 1024,
  mediumFileMaxBytes: 5 * 1024 * 1024,
  largeFileThresholdBytes: 5 * 1024 * 1024,
  parseWindowBytes: 64 * 1024,
  guardBytes: 4 * 1024,
  memoryBudgetBytes: 30 * 1024 * 1024,
  timeoutMs: 5000,
  fallbackMode: "plain-text-fallback",
  highlightingStates: Object.freeze(["full", "windowed", "degraded", "plain-text-fallback"])
});

function finitePolicyNumber(value, fallback) {
  const number = Number(value ?? fallback);
  return Number.isFinite(number) && number >= 0 ? Math.trunc(number) : fallback;
}

export function markdownPolicyForDocument(options = {}) {
  const policy = { ...markdownLargeFilePolicy, ...(options.policy ?? {}) };
  const byteLength = finitePolicyNumber(
    options.documentByteLength ?? options.fileSizeBytes ?? options.byteLength,
    0
  );
  const smallFileMaxBytes = finitePolicyNumber(policy.smallFileMaxBytes, markdownLargeFilePolicy.smallFileMaxBytes);
  const mediumFileMaxBytes = finitePolicyNumber(policy.mediumFileMaxBytes, markdownLargeFilePolicy.mediumFileMaxBytes);
  const memoryBudgetBytes = finitePolicyNumber(policy.memoryBudgetBytes, markdownLargeFilePolicy.memoryBudgetBytes);
  const budgetExceeded = Boolean(
    options.budgetExceeded ?? options.syntaxBudgetExceeded ?? options.memoryBudgetExceeded
  );
  const parserTimedOut = Boolean(options.parserTimedOut ?? options.parseTimedOut);

  let tier = "small";
  let highlightingState = "full";
  if (byteLength > mediumFileMaxBytes) {
    tier = "large";
    highlightingState = "windowed";
  } else if (byteLength > smallFileMaxBytes) {
    tier = "medium";
    highlightingState = "windowed";
  }
  if (parserTimedOut) highlightingState = "degraded";
  if (budgetExceeded || memoryBudgetBytes === 0) highlightingState = "plain-text-fallback";

  return Object.freeze({
    tier,
    byteLength,
    highlightingState,
    parseWindowBytes: finitePolicyNumber(policy.parseWindowBytes, markdownLargeFilePolicy.parseWindowBytes),
    guardBytes: finitePolicyNumber(policy.guardBytes, markdownLargeFilePolicy.guardBytes),
    memoryBudgetBytes,
    timeoutMs: finitePolicyNumber(policy.timeoutMs, markdownLargeFilePolicy.timeoutMs)
  });
}

export const markdownEditorRules = Object.freeze(
  buildCodeEditingManifest({
    indentSize: 2,
    enter: {
      kind: "continueLineMarkers",
      markers: ["-", "*", "+", "ordered-dot"],
      exitOnEmptyItem: true
    },
    pairs: [
      { open: "(", close: ")" },
      { open: "[", close: "]" },
      { open: "**", close: "**" },
      { open: "__", close: "__" },
      { open: "`", close: "`" }
    ],
    autocompleteTriggers: ["#", "[", "`"],
    // Plan 071 task 11: prose movement. Underscore and camelCase carry no
    // meaning in prose, so word motion splits on them; everything else defers
    // to the code-editing movement defaults server-side.
    movement: {
      wordSeparators: "prose",
      treatUnderscoreAsWord: false,
      camelCaseSubWord: false
    }
  })
);

export const markdownCompletionProvider = Object.freeze({
  id: "markdown.keywords",
  priority: 0,
  triggerCharacters: completionTriggerCharactersFromEditorRules(markdownEditorRules),
  wordBoundaryChars: ["#", "[", "]", "(", ")", "`", "*", "_"],
  items: [
    "# ", "## ", "### ", "#### ", "##### ", "###### ", "- ", "* ",
    "1. ", "> ", "```", "~~~", "---", "**", "__", "`"
  ],
  budgets: { timeoutMs: 300, maxItems: 32 }
});

export const commands = Object.freeze([
  {
    id: "markdown.toggleComment",
    userFacingName: "Toggle Markdown Comment",
    routingPolicy: "ServerFirst",
    permissions: []
  },
  {
    id: "markdown.togglePreview",
    userFacingName: "Toggle Markdown Preview",
    routingPolicy: "ServerFirst",
    permissions: []
  },
  {
    id: "markdown.insertHeading",
    userFacingName: "Insert Markdown Heading",
    routingPolicy: "ServerFirst",
    permissions: []
  },
  {
    id: "markdown.toggleList",
    userFacingName: "Toggle Markdown List",
    routingPolicy: "ServerFirst",
    permissions: []
  }
]);

export const markdownStatusItem = Object.freeze({
  kind: "statusItem",
  id: "markdown.status.mode",
  style: { variant: "muted" },
  children: [{ kind: "label", id: "markdown.status.mode.label" }]
});

export const behaviorTransforms = Object.freeze([
  {
    transformId: "markdown.list-continuation",
    ruleKind: "markdown_list_continuation",
    routingPolicy: "ClientFirstPredictable",
    markers: ["-", "*", "+", "ordered-dot"],
    exitOnEmptyItem: true
  },
  {
    transformId: "markdown.fenced-code-indent",
    ruleKind: "markdown_fenced_code_indent",
    routingPolicy: "ClientFirstPredictable",
    fenceMarkers: ["```", "~~~"],
    copyBodyIndent: true
  },
  {
    transformId: "markdown.inline-pair-handling",
    ruleKind: "markdown_inline_pair_handling",
    routingPolicy: "ClientFirstPredictable",
    pairs: ["**", "__", "`"]
  }
]);

export function markdownPackageManifest() {
  return {
    name: packageName,
    version: "0.1.0",
    type: "module",
    exports: {
      ".": "./dist/index.js",
      "./load": "./dist/load.js",
      "./parser": "./dist/parser.js",
      "./sdui": "./dist/sdui.js"
    },
    clay: {
      apiPrefix,
      entry: "./dist/index.js",
      loadEntry: "./dist/load.js",
      permissions: [
        "mode-registration",
        "mode-activation",
        "command-registration",
        "completion-provider",
        "parse-document",
        "render-decorations"
      ],
      modes: [modeId],
      docs: "./docs/index.md",
      apiDependencies: [
        "syntax.serverRegisterSyntaxGrammar",
        "modes.serverRegisterModePattern",
        "modes.serverActivateMajorMode",
        "behavior.buildCodeEditingManifest",
        "commands.serverRegisterCommand",
        "completion.serverRegisterCompletionProvider",
        "completion.completionTriggerCharactersFromEditorRules",
        "parse.serverRegisterParseHandler",
        "decorations.serverPublishDecorations",
        "ui.serverRegisterComponentContribution"
      ],
      performance: {
        estimatedManifestBytes: 1900,
        hotPathPolicy: "no hot-path JS on keypress/paint"
      },
      extensionPoints: [{"id": "markdown.completionProviders", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["completionProvider"], "scopes": ["markdown.keywords"], "summary": "Add or replace completion providers for Markdown mode."}, {"id": "markdown.languageLayers", "version": 1, "operations": ["append"], "contributionKinds": ["decorationLayer", "diagnosticSource", "analyzer", "intelligenceProvider"], "scopes": ["markdown.syntaxDecorations", "markdown.parseDecorations"], "summary": "Append package-owned parse, decoration, diagnostic, and intelligence layers for Markdown documents."}, {"id": "markdown.commands", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["command", "keyRoute", "textTransform"], "scopes": ["markdown.toggleComment", "markdown.toggleList", "markdown.togglePreview", "markdown.insertHeading"], "summary": "Add or replace Markdown commands, key routes, and text transforms."}, {"id": "markdown.ui", "version": 1, "operations": ["append", "replace"], "contributionKinds": ["panelContribution", "componentContribution", "sduiRegion", "statusItem"], "scopes": ["markdown.preview", "markdown.preview.root", "markdown.previewStatus"], "summary": "Add or replace Markdown preview and status UI contributions."}, {"id": "markdown.grammar", "version": 1, "operations": ["replace"], "contributionKinds": ["grammar"], "scopes": ["markdown.markdown"], "summary": "Replace the Markdown grammar, highlights query, and style map."}, {"id": "markdown.modePattern", "version": 1, "operations": ["append"], "contributionKinds": ["modePattern"], "summary": "Extend Markdown file and mode patterns."}],
      contributions: {
        syntaxGrammars: [markdownSyntaxGrammar],
        modePatterns: [
          {
            mode: modeId,
            displayName: "Markdown",
            extensions: supportedExtensions,
            mimeTypes: supportedMimeTypes
          }
        ],
        commands: commands.map((command) => ({
          id: command.id,
          displayName: command.userFacingName,
          routingPolicy: "server-first"
        })),
        completionProviders: [markdownCompletionProvider],
        keyRouting: [
          { commandId: "markdown.togglePreview", key: "Ctrl+Shift+M", routingPolicy: "server-first" },
          { commandId: "markdown.insertHeading", key: "Ctrl+Alt+1", routingPolicy: "server-first" },
          { commandId: "markdown.toggleList", key: "Ctrl+Shift+8", routingPolicy: "server-first" }
        ],
        textTransforms: behaviorTransforms.map((transform) => ({
          transformId: transform.transformId,
          kind: transform.transformId === "markdown.inline-pair-handling" ? "pair-rule" : "enter-rule"
        })),
        ui: {
          components: [markdownStatusItem]
        },
        sdui: [
          {
            regionId: "markdown.previewStatus",
            displayName: "Markdown Preview Status",
            adapter: "./dist/sdui.js",
            estimatedSnapshotBytes: 2048,
            estimatedUpdateBytes: 512
          }
        ],
        decorations: []
      }
    }
  };
}

// ponytail: re-export the package-owned default load entry at the package root
// so the documented fallback `import { markdownLoadMode } from "@clay/markdown"`
// resolves. Appended after all declarations above are initialized so the
// circular index <-> load re-export stays TDZ-safe (load.js only references
// these bindings inside functions it does not call at module-eval time).
// See decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md.
export { loadMarkdownPackage, markdownLoadMode, registerMarkdownPreview } from "./load.js";
