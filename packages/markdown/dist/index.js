export const packageName = "@clay/markdown";
export const apiPrefix = "markdown";
export const modeId = "markdown";

export const supportedExtensions = ["md", "markdown", "mdown"];
export const supportedMimeTypes = ["text/markdown"];

export const commands = Object.freeze([
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
        "parse-document",
        "render-decorations"
      ],
      modes: [modeId],
      docs: "./docs/index.md",
      apiDependencies: [
        "clay.modes.serverRegisterModePattern",
        "clay.modes.serverActivateMajorMode",
        "clay.commands.serverRegisterCommand",
        "clay.parse.serverRegisterParseHandler",
        "clay.decorations.serverPublishDecorations"
      ],
      performance: {
        estimatedManifestBytes: 1900,
        hotPathPolicy: "no hot-path JS on keypress/paint"
      },
      contributions: {
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
        keyRouting: [
          { commandId: "markdown.togglePreview", key: "Ctrl+Shift+M", routingPolicy: "server-first" },
          { commandId: "markdown.insertHeading", key: "Ctrl+Alt+1", routingPolicy: "server-first" },
          { commandId: "markdown.toggleList", key: "Ctrl+Shift+8", routingPolicy: "server-first" }
        ],
        textTransforms: behaviorTransforms.map((transform) => ({
          transformId: transform.transformId,
          kind: transform.transformId === "markdown.inline-pair-handling" ? "pair-rule" : "enter-rule"
        })),
        sdui: [
          {
            regionId: "markdown.previewStatus",
            displayName: "Markdown Preview Status",
            adapter: "./dist/sdui.js",
            estimatedSnapshotBytes: 2048,
            estimatedUpdateBytes: 512
          }
        ],
        decorations: [
          {
            primitiveId: "markdown.syntaxDecorations",
            kind: "markdown.syntax",
            adapter: "./dist/parser.js"
          }
        ]
      }
    }
  };
}
