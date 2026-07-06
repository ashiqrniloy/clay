// Clay JavaScript facade module entry point.
//
// Future import-map entries will expose these domain modules as `clay:*`
// specifiers. This aggregate module is for source-tree organization and tests.

export * as editor from "./editor.ts";
export * as keybindings from "./keybindings.ts";
export * as configuration from "./configuration.ts";
export * as documents from "./documents.ts";
export * as workspace from "./workspace.ts";
export * as git from "./git.ts";
export * as behavior from "./behavior.ts";
export * as application from "./application.ts";
export * as sdui from "./sdui.ts";
export * as ui from "./ui.ts";
export * as packages from "./packages.ts";
export * as modes from "./modes.ts";
export * as commands from "./commands.ts";
export * as decorations from "./decorations.ts";
export * as parse from "./parse.ts";
export * as completion from "./completion.ts";
