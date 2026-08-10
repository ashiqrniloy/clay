// Clay shell pane management facade (Phase 22.1).
//
// Each export returns a stable Clay command ID string. The command IDs are
// ClientUiCommand-routed: bindKey installs an inert keybinding that the client
// dispatches to ClayShellWidget without a server round-trip. Packages and user
// configuration (`~/.config/clay/init.js`) use these helpers with
// `clay.keybindings.bindKey` to remap the default Phase 22.1 chords.
//
// No function here performs side effects, runs server IPC, or mutates the
// shell directly. The returned string is the bindable command ID only.
//
// `setPaneFocusPolicy` is the exception: it calls a server-side op to validate
// and publish the preference, which is then transported to the client.

function shellOps() {
    const ops = Deno?.core?.ops;
    if (typeof ops?.op_clay_shell_set_pane_focus_policy !== "function") {
        throw new Error("clay.shell runtime ops are unavailable in this context");
    }
    return ops;
}

export function clientSplitPaneVertical() {
    return "clay.shell.clientSplitPaneVertical";
}
export function clientSplitPaneHorizontal() {
    return "clay.shell.clientSplitPaneHorizontal";
}
// Phase 22.7 (F3): direction-named aliases resolving to the canonical
// split handlers (clientSplitPaneRight -> vertical, clientSplitPaneDown ->
// horizontal). No default chords; canonical IDs keep their bindings.
export function clientSplitPaneRight() {
    return "clay.shell.clientSplitPaneRight";
}
export function clientSplitPaneDown() {
    return "clay.shell.clientSplitPaneDown";
}
export function clientAddEqualPane() {
    return "clay.shell.clientAddEqualPane";
}
export function clientClosePane() {
    return "clay.shell.clientClosePane";
}
export function clientFocusPaneNext() {
    return "clay.shell.clientFocusPaneNext";
}
export function clientFocusPanePrev() {
    return "clay.shell.clientFocusPanePrev";
}
export function clientResizePaneLeft() {
    return "clay.shell.clientResizePaneLeft";
}
export function clientResizePaneRight() {
    return "clay.shell.clientResizePaneRight";
}
export function clientResizePaneUp() {
    return "clay.shell.clientResizePaneUp";
}
export function clientResizePaneDown() {
    return "clay.shell.clientResizePaneDown";
}
export function clientMovePaneNext() {
    return "clay.shell.clientMovePaneNext";
}
export function clientMovePanePrev() {
    return "clay.shell.clientMovePanePrev";
}
// Phase 22.4: shell tab-management facade. Same contract as the pane helpers:
// each export returns a stable ClientUiCommand-routed command ID usable with
// `clay.keybindings.bindKey` (scope "global"). Numbered families are 1-based
// tab positions and exist for 1..9 only ("beyond 9" is not a command ID).
export function clientTabNext() {
    return "clay.shell.clientTabNext";
}
export function clientTabPrev() {
    return "clay.shell.clientTabPrev";
}
export function clientTabNew() {
    return "clay.shell.clientTabNew";
}
export function clientTabClose() {
    return "clay.shell.clientTabClose";
}
export function clientTabMoveLeft() {
    return "clay.shell.clientTabMoveLeft";
}
export function clientTabMoveRight() {
    return "clay.shell.clientTabMoveRight";
}
export function clientTabActivate(position) {
    return tabVariantId("clay.shell.clientTabActivate", position);
}
export function clientTabMoveTo(position) {
    return tabVariantId("clay.shell.clientTabMoveTo", position);
}

function tabVariantId(family, position) {
    const n = Number(position);
    if (!Number.isInteger(n) || n < 1 || n > 9) {
        throw new RangeError(
            `clay.shell.invalid_tab_position: ${family} requires an integer position 1..9`,
        );
    }
    return `${family}.${n}`;
}
/**
 * Set the pane-focus policy (`"click"` or `"cursor"`). When `"cursor"`, moving
 * the pointer over a pane activates it without a click. The default is `"click"`.
 *
 * @param {{ paneFocusPolicy: "click" | "cursor" }} options
 * @returns {{ paneFocusPolicy: string }}
 */
export function setPaneFocusPolicy(options) {
    if (options === null || typeof options !== "object" || typeof options.paneFocusPolicy !== "string") {
        throw new Error("clay.shell.invalid_pane_focus_policy: setPaneFocusPolicy requires { paneFocusPolicy: \"click\" | \"cursor\" }");
    }
    return JSON.parse(shellOps().op_clay_shell_set_pane_focus_policy(JSON.stringify(options)));
}
