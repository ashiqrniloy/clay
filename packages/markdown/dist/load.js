import { serverRegisterCommand } from "clay:commands";
import { serverActivateMajorMode, serverRegisterModePattern } from "clay:modes";
import { serverLoadPackage } from "clay:packages";
import { serverRegisterParseHandler } from "clay:parse";

import {
  apiPrefix,
  behaviorTransforms,
  commands,
  markdownLargeFilePolicy,
  markdownPackageManifest,
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
      adapter: "./dist/parser.js",
      styleTokens: [
        "markup.heading.1",
        "markup.heading.2",
        "markup.heading.3",
        "markup.heading.4",
        "markup.heading.5",
        "markup.heading.6",
        "markup.strong",
        "markup.emphasis",
        "markup.inline-code",
        "markup.code-block",
        "markup.list-marker"
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

// All Markdown-specific editing knowledge lives here in the package.
// The editor side exposes generic primitives (EnterRule kinds, PairRule,
// CommentContinuationRule); the package decides which rule instances to use.
const MARKDOWN_EDITOR_RULES = Object.freeze({
  // Unordered and ordered list markers trigger ContinueLineMarkers.
  // Any mode with list-like continuation declares its own marker strings here.
  enter: {
    kind: "continueLineMarkers",
    markers: ["-", "*", "+", "ordered-dot"],
    exitOnEmptyItem: true
  },
  // Generic pair rules: the editor auto-closes these delimiter pairs.
  // Markdown adds ** and __ (bold), ` (inline code) on top of the base pairs.
  pairs: [
    { open: "(", close: ")" },
    { open: "[", close: "]" },
    { open: "**", close: "**" },
    { open: "__", close: "__" },
    { open: "`",  close: "`"  }
  ],
  // Markdown has no line-comment continuation.
  comments: [],
  tabSpaces: 4
});

// Package-declared commands with routing policy.
// The editor registers these as ServerFirst intents with no built-in authority.
const MARKDOWN_COMMANDS = Object.freeze([
  { id: "markdown.togglePreview",  displayName: "Toggle Markdown Preview",  routingPolicy: "server-first" },
  { id: "markdown.insertHeading",  displayName: "Insert Markdown Heading",  routingPolicy: "server-first" },
  { id: "markdown.toggleList",     displayName: "Toggle Markdown List",     routingPolicy: "server-first" }
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
  await clay.modes.serverRegisterModePattern(packageManifest, {
    modeId,
    displayName: "Markdown",
    extensions: supportedExtensions,
    mimeTypes: supportedMimeTypes
  });

  // Activate the major mode for this package.  The editorRules, commands, and
  // keymaps fields are all Markdown-specific knowledge declared by the package.
  // The editor op deserializes them into generic protocol types (EnterRule,
  // PairRule, KeyBindingRule) — no Markdown logic lives in Rust.
  await clay.modes.serverActivateMajorMode(packageManifest, {
    documentId,
    path: documentPath,
    mimeType,
    editorRules: MARKDOWN_EDITOR_RULES,
    commands: MARKDOWN_COMMANDS,
    keymaps: MARKDOWN_KEYMAPS
  });

  // Register each command so it is discoverable in help/search.
  for (const command of commands) {
    await clay.commands.serverRegisterCommand(packageManifest, {
      commandId: command.id,
      displayName: command.userFacingName,
      routingPolicy: command.routingPolicy,
      permissions: command.permissions
    });
  }

  // Register the background parse handler (server-side only, no hot-path JS).
  await clay.parse.serverRegisterParseHandler({
    packageManifest,
    mode: modeId,
    parseUnit: contract.parse.parseUnit,
    viewportPriority: contract.parse.viewportPriority,
    adapter: contract.parse.adapter,
    maxWindowBytes: contract.parse.parseWindowBytes,
    guardBytes: contract.parse.guardBytes,
    memoryBudgetBytes: contract.parse.memoryBudgetBytes,
    timeoutMs: contract.parse.timeoutMs
  });

  return contract;
}

// ponytail: package-owned one-line fallback entry. Imports the Clay facades
// directly (no caller-supplied `clay` object, no inline manifest) and reuses
// loadMarkdownPackage. This is the documented temporary fallback while the
// generic loadPackage("@clay/*") resolver + first-party module-loader bridge
// remain deferred (see decision-logs/2026-06-15-1015-...). Once that bridge
// ships, loadPackage("@clay/markdown") will invoke this same default setup.
export async function markdownLoadMode(options = {}) {
  const clay = {
    packages: { serverLoadPackage },
    modes: { serverActivateMajorMode, serverRegisterModePattern },
    commands: { serverRegisterCommand },
    parse: { serverRegisterParseHandler }
  };
  return loadMarkdownPackage(clay, options);
}

// Default activation entry. `loadPackage("@clay/markdown")` imports this module
// (the declared `clay.loadEntry`) and invokes this default export so the
// package's mode/commands/parse handler register under Clay's authority without
// any per-primitive plumbing in user config.
export default markdownLoadMode;
