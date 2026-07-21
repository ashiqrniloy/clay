import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider } from "clay:completion";
import { serverActivateMajorMode, serverRegisterModePattern } from "clay:modes";
import { serverLoadPackage } from "clay:packages";
import { serverRegisterParseHandler } from "clay:parse";
import { serverRegisterComponentContribution, serverRegisterPanelContribution } from "clay:ui";

import {
  apiPrefix,
  behaviorTransforms,
  commands,
  markdownCompletionProvider,
  markdownEditorRules,
  markdownLargeFilePolicy,
  markdownPackageManifest,
  markdownStatusItem,
  modeId,
  packageName,
  supportedExtensions,
  supportedMimeTypes
} from "./index.js";

export function markdownPackageContract() {
  return {
    packageName,
    apiPrefix,
    modeId,
    supportedExtensions,
    supportedMimeTypes,
    commands,
    behaviorTransforms,
    parse: {
      id: "markdown.parseDecorations",
      role: "tier3-javascript-fallback",
      adapter: "./dist/parser.js",
      parseUnit: "line-group",
      viewportPriority: true,
      parseWindowBytes: markdownLargeFilePolicy.parseWindowBytes,
      guardBytes: markdownLargeFilePolicy.guardBytes,
      memoryBudgetBytes: markdownLargeFilePolicy.memoryBudgetBytes,
      timeoutMs: markdownLargeFilePolicy.timeoutMs,
      smallFileMaxBytes: markdownLargeFilePolicy.smallFileMaxBytes,
      mediumFileMaxBytes: markdownLargeFilePolicy.mediumFileMaxBytes,
      largeFileThresholdBytes: markdownLargeFilePolicy.largeFileThresholdBytes,
      fallbackMode: markdownLargeFilePolicy.fallbackMode
    },
    decorations: {
      primitiveId: "markdown.syntaxDecorations",
      engine: "tier1-native",
      grammar: "tree-sitter-md-025",
      fallbackAdapter: "./dist/parser.js",
      vocabularyTokens: [
        "Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6",
        "Paragraph+Bold", "Paragraph+Italic", "CodeSpan", "CodeBlock",
        "ListItem", "Link", "Quote"
      ]
    },
    sdui: {
      regionId: "markdown.previewStatus",
      displayName: "Markdown Preview and Parse Status",
      adapter: "./dist/sdui.js"
    },
    packageManifest: markdownPackageManifest()
  };
}

// Package-declared activation metadata stays local to avoid evaluating imported
// index bindings during the package's intentional index <-> load re-export cycle.
const MARKDOWN_COMMANDS = Object.freeze([
  { id: "markdown.toggleComment", displayName: "Toggle Markdown Comment", routingPolicy: "server-first" },
  { id: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" },
  { id: "markdown.insertHeading", displayName: "Insert Markdown Heading", routingPolicy: "server-first" },
  { id: "markdown.toggleList", displayName: "Toggle Markdown List", routingPolicy: "server-first" }
]);

// Package-declared key bindings as generic key descriptor objects.
// The editor stores these as KeyBindingRule entries in the behavior manifest.
const MARKDOWN_KEYMAPS = Object.freeze([
  { commandId: "markdown.togglePreview", key: "Ctrl+Shift+M", routingPolicy: "server-first" },
  { commandId: "markdown.insertHeading", key: "Ctrl+Alt+1",   routingPolicy: "server-first" },
  { commandId: "markdown.toggleList",    key: "Ctrl+Shift+8", routingPolicy: "server-first" }
]);

export async function loadMarkdownPackage(clay, options = {}) {
  const contract = markdownPackageContract();
  const packageManifest = contract.packageManifest;
  const documentId = Number(options.documentId ?? 1);
  const documentPath = String(options.path ?? options.documentPath ?? "sample.md");
  const mimeType = options.mimeType ?? undefined;

  await clay.packages?.serverLoadPackage?.(packageManifest);

  // Register the mode's file-extension and MIME patterns.
  // The editor classifies documents by these static patterns; no JS runs on open.
  await clay.modes.serverRegisterModePattern({
    modeId,
    displayName: "Markdown",
    defaultFontRole: "proportional",
    extensions: supportedExtensions,
    mimeTypes: supportedMimeTypes,
    editorRules: markdownEditorRules,
    commands: MARKDOWN_COMMANDS,
    keymaps: MARKDOWN_KEYMAPS
  });

  // Activate the major mode for this package.  The editorRules, commands, and
  // keymaps fields are all Markdown-specific knowledge declared by the package.
  // The editor op deserializes them into generic protocol types (EnterRule,
  // PairRule, KeyBindingRule) — no Markdown logic lives in Rust.
  await clay.modes.serverActivateMajorMode({
    documentId,
    path: documentPath,
    mimeType,
    editorRules: markdownEditorRules,
    commands: MARKDOWN_COMMANDS,
    keymaps: MARKDOWN_KEYMAPS
  });

  // Register each command so it is discoverable in help/search.
  for (const command of commands) {
    await clay.commands.serverRegisterCommand({
      commandId: command.id,
      displayName: command.userFacingName,
      routingPolicy: command.routingPolicy,
      permissions: command.permissions
    });
  }

  await clay.completion?.serverRegisterCompletionProvider?.({
    completionProvider: markdownCompletionProvider,
    providerId: markdownCompletionProvider.id,
    priority: markdownCompletionProvider.priority,
    triggerCharacters: markdownCompletionProvider.triggerCharacters,
    wordBoundaryChars: markdownCompletionProvider.wordBoundaryChars,
    items: markdownCompletionProvider.items,
    budgets: markdownCompletionProvider.budgets
  });

  // Register inert status metadata; native client code owns rendering.
  await clay.ui?.serverRegisterComponentContribution?.(markdownStatusItem);

  // Keep parser.js registered as Tier 3 fallback metadata. On open, Clay's
  // generic syntax selector installs the matching Tier 1 native handler first;
  // this same package/mode fallback is used only when no native handler wins.
  let parserModule;
  try {
    parserModule = await import("./parser.js");
  } catch {
    // ponytail: copied fixture load roots may omit parser.js; they still verify
    // metadata registration. Real @clay/markdown package load includes parser.js.
  }
  await clay.parse.serverRegisterParseHandler({
    mode: modeId,
    parseUnit: contract.parse.parseUnit,
    viewportPriority: contract.parse.viewportPriority,
    adapter: contract.parse.adapter,
    ...(parserModule ? { module: parserModule, exportName: "parseMarkdownDecorationUpdate" } : {}),
    maxWindowBytes: contract.parse.parseWindowBytes,
    guardBytes: contract.parse.guardBytes,
    memoryBudgetBytes: contract.parse.memoryBudgetBytes,
    timeoutMs: contract.parse.timeoutMs
  });

  return contract;
}

// ponytail: package-owned alias for advanced/per-load options. The default
// user path is now `loadPackage("@clay/markdown")`, which invokes this same
// setup through the first-party resolver and persistent runtime.
export async function markdownLoadMode(options = {}) {
  const clay = {
    packages: { serverLoadPackage },
    modes: { serverActivateMajorMode, serverRegisterModePattern },
    commands: { serverRegisterCommand },
    completion: { serverRegisterCompletionProvider },
    parse: { serverRegisterParseHandler },
    ui: { serverRegisterComponentContribution }
  };
  return loadMarkdownPackage(clay, options);
}

// Optional Markdown preview panel. NOT called by the default load entry above
// — `loadPackage("@clay/markdown")` and `markdownLoadMode()` publish NO
// PanelContribution by default, so the right slot stays empty unless the host
// opts in. Call this helper explicitly AFTER the package is loaded (so its
// `markdown.togglePreview` command is registered), or drive it through Clay's
// package-option / layout-override configuration. The PackageUiRegistry
// validates the action target, slot, and payload before publication.
export function registerMarkdownPreview() {
  return serverRegisterPanelContribution({
    id: "markdown.preview",
    slot: "right",
    kind: "fixed",
    defaultVisibility: "hidden",
    actionTargets: ["markdown.togglePreview"],
    component: {
      kind: "panel",
      id: "markdown.preview.root",
      title: "Markdown Preview",
      children: []
    }
  });
}

// Default activation entry. `loadPackage("@clay/markdown")` imports this module
// (the declared `clay.loadEntry`) and invokes this default export so the
// package's mode/commands/parse handler register under Clay's authority without
// any per-primitive plumbing in user config.
export default markdownLoadMode;
