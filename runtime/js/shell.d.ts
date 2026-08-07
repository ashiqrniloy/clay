export type ClientSplitPaneVerticalCommandId = "clay.shell.clientSplitPaneVertical";
export type ClientSplitPaneHorizontalCommandId = "clay.shell.clientSplitPaneHorizontal";
export type ClientAddEqualPaneCommandId = "clay.shell.clientAddEqualPane";
export type ClientClosePaneCommandId = "clay.shell.clientClosePane";
export type ClientFocusPaneNextCommandId = "clay.shell.clientFocusPaneNext";
export type ClientFocusPanePrevCommandId = "clay.shell.clientFocusPanePrev";
export type ClientResizePaneLeftCommandId = "clay.shell.clientResizePaneLeft";
export type ClientResizePaneRightCommandId = "clay.shell.clientResizePaneRight";
export type ClientResizePaneUpCommandId = "clay.shell.clientResizePaneUp";
export type ClientResizePaneDownCommandId = "clay.shell.clientResizePaneDown";
export type ClientMovePaneNextCommandId = "clay.shell.clientMovePaneNext";
export type ClientMovePanePrevCommandId = "clay.shell.clientMovePanePrev";
export type ClientTabNextCommandId = "clay.shell.clientTabNext";
export type ClientTabPrevCommandId = "clay.shell.clientTabPrev";
export type ClientTabNewCommandId = "clay.shell.clientTabNew";
export type ClientTabCloseCommandId = "clay.shell.clientTabClose";
export type ClientTabMoveLeftCommandId = "clay.shell.clientTabMoveLeft";
export type ClientTabMoveRightCommandId = "clay.shell.clientTabMoveRight";
/** 1-based tab positions: only 1..9 exist ("beyond 9" is not a command ID). */
export type ClientTabPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
export type ClientTabActivateCommandId = `clay.shell.clientTabActivate.${ClientTabPosition}`;
export type ClientTabMoveToCommandId = `clay.shell.clientTabMoveTo.${ClientTabPosition}`;

export declare function clientSplitPaneVertical(): ClientSplitPaneVerticalCommandId;
export declare function clientSplitPaneHorizontal(): ClientSplitPaneHorizontalCommandId;
export declare function clientAddEqualPane(): ClientAddEqualPaneCommandId;
export declare function clientClosePane(): ClientClosePaneCommandId;
export declare function clientFocusPaneNext(): ClientFocusPaneNextCommandId;
export declare function clientFocusPanePrev(): ClientFocusPanePrevCommandId;
export declare function clientResizePaneLeft(): ClientResizePaneLeftCommandId;
export declare function clientResizePaneRight(): ClientResizePaneRightCommandId;
export declare function clientResizePaneUp(): ClientResizePaneUpCommandId;
export declare function clientResizePaneDown(): ClientResizePaneDownCommandId;
export declare function clientMovePaneNext(): ClientMovePaneNextCommandId;
export declare function clientMovePanePrev(): ClientMovePanePrevCommandId;
export declare function clientTabNext(): ClientTabNextCommandId;
export declare function clientTabPrev(): ClientTabPrevCommandId;
export declare function clientTabNew(): ClientTabNewCommandId;
export declare function clientTabClose(): ClientTabCloseCommandId;
export declare function clientTabMoveLeft(): ClientTabMoveLeftCommandId;
export declare function clientTabMoveRight(): ClientTabMoveRightCommandId;
export declare function clientTabActivate(position: ClientTabPosition): ClientTabActivateCommandId;
export declare function clientTabMoveTo(position: ClientTabPosition): ClientTabMoveToCommandId;
export interface PaneFocusPolicyOptions {
    paneFocusPolicy: "click" | "cursor";
}

export interface PaneFocusPolicyResult {
    paneFocusPolicy: string;
}

export declare function setPaneFocusPolicy(options: PaneFocusPolicyOptions): PaneFocusPolicyResult;
