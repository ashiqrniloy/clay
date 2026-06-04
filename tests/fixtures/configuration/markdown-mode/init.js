import { serverRegisterCommand } from "clay:commands";
import { serverPublishDecorations } from "clay:decorations";
import { serverOpenDocument } from "clay:documents";
import { serverActivateMajorMode, serverRegisterModePattern } from "clay:modes";
import { serverLoadPackage } from "clay:packages";
import { serverRegisterParseHandler } from "clay:parse";
import {
  defineButton,
  defineEditorView,
  defineFlex,
  defineLabel,
  defineList,
  definePanel,
  defineStack,
  publishTree,
} from "clay:sdui";
import { serverListWorkspaceRoots } from "clay:workspace";

const markdownPackage = {
  name: "@clay/markdown",
  version: "0.1.0",
  type: "module",
  exports: {
    ".": "./dist/index.js",
    "./load": "./dist/load.js",
    "./parser": "./dist/parser.js",
    "./sdui": "./dist/sdui.js",
  },
  clay: {
    apiPrefix: "markdown",
    entry: "./dist/index.js",
    loadEntry: "./dist/load.js",
    permissions: [
      "mode-registration",
      "mode-activation",
      "command-registration",
      "parse-document",
      "render-decorations",
    ],
    modes: ["markdown"],
    docs: "./docs/index.md",
    apiDependencies: [
      "clay.modes.serverRegisterModePattern",
      "clay.modes.serverActivateMajorMode",
      "clay.commands.serverRegisterCommand",
      "clay.parse.serverRegisterParseHandler",
      "clay.decorations.serverPublishDecorations",
    ],
    contributions: {
      commands: [
        {
          id: "markdown.togglePreview",
          displayName: "Toggle Markdown Preview",
          routingPolicy: "server-first",
        },
        {
          id: "markdown.insertHeading",
          displayName: "Insert Markdown Heading",
          routingPolicy: "server-first",
        },
        {
          id: "markdown.toggleList",
          displayName: "Toggle Markdown List",
          routingPolicy: "server-first",
        },
      ],
      sdui: [
        {
          regionId: "markdown.previewStatus",
          displayName: "Markdown Preview Status",
          estimatedSnapshotBytes: 2048,
          estimatedUpdateBytes: 512,
        },
      ],
      decorations: [
        {
          primitiveId: "markdown.syntaxDecorations",
          kind: "markdown.syntax",
        },
      ],
    },
  },
};

const roots = await serverListWorkspaceRoots();
let opened = null;
if (roots.length > 0) {
  opened = await serverOpenDocument({ workspaceRootId: roots[0].workspaceRootId, path: "sample.md" });
}

const documentId = Number(opened?.metadata?.documentId ?? 1);
const documentVersion = Number(opened?.metadata?.version ?? 1);
const documentPath = opened?.metadata?.path ?? "sample.md";

serverLoadPackage(markdownPackage);
serverRegisterModePattern(markdownPackage, {
  modeId: "markdown",
  displayName: "Markdown",
  extensions: ["md", "markdown", "mdown"],
  mimeTypes: ["text/markdown"],
});

const markdownEditorRules = {
  enter: {
    kind: "continueLineMarkers",
    markers: ["-", "*", "+", "ordered-dot"],
    exitOnEmptyItem: true,
  },
  pairs: [
    { open: "(", close: ")" },
    { open: "[", close: "]" },
    { open: "**", close: "**" },
    { open: "__", close: "__" },
    { open: "`", close: "`" },
  ],
  comments: [],
  tabSpaces: 4,
};
const activationInput = {
  documentId,
  path: documentPath,
  editorRules: markdownEditorRules,
  commands: markdownPackage.clay.contributions.commands,
  keymaps: [
    { commandId: "markdown.togglePreview", key: "Ctrl+Shift+M", routingPolicy: "server-first" },
    { commandId: "markdown.insertHeading", key: "Ctrl+Alt+1", routingPolicy: "server-first" },
    { commandId: "markdown.toggleList", key: "Ctrl+Shift+8", routingPolicy: "server-first" },
  ],
};
serverActivateMajorMode(markdownPackage, activationInput);
const activation = serverActivateMajorMode(markdownPackage, activationInput);

for (const command of markdownPackage.clay.contributions.commands) {
  serverRegisterCommand(markdownPackage, {
    commandId: command.id,
    displayName: command.displayName,
    routingPolicy: command.routingPolicy,
    permissions: [],
  });
}

serverRegisterParseHandler({
  packageManifest: markdownPackage,
  mode: "markdown",
  parseUnit: "line-group",
  viewportPriority: true,
});

serverPublishDecorations({
  packageManifest: markdownPackage,
  documentId,
  documentVersion,
  currentDocumentVersion: documentVersion,
  viewport: { byteStart: 0, byteEnd: 256 },
  spans: [
    { byteStart: 0, byteEnd: 1, kind: "syntax", styleToken: "markup.heading.1", priority: 10 },
    { byteStart: 2, byteEnd: 23, kind: "syntax", styleToken: "markup.heading.1", priority: 9 },
  ],
});

const root = defineFlex({
  id: "markdown-root",
  direction: "row",
  children: [
    definePanel({
      id: "markdown-panel",
      title: "Markdown Preview",
      children: [
        defineStack({
          id: "markdown-stack",
          children: [
            defineLabel({ id: "markdown-document", text: `Document: ${documentPath}` }),
            defineLabel({ id: "markdown-mode", text: `Mode: ${activation.modeId}` }),
            defineLabel({ id: "markdown-parse", text: "Parse: markdown-it registered" }),
            defineLabel({ id: "markdown-decorations", text: "Decorations: published" }),
            defineLabel({ id: "markdown-preview", text: "Preview: decorated editor" }),
            defineButton({
              id: "markdown-toggle-preview",
              label: "Toggle Preview",
              action: { commandId: "markdown.togglePreview" },
            }),
            defineList({
              id: "markdown-preview-list",
              items: [
                {
                  id: "markdown-preview-mode",
                  label: "Decorated editor preview",
                  detail: "Inert package SDUI, server-routed command only",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "markdown-editor", documentId, expectedVersion: documentVersion }),
  ],
});

await publishTree(root);
