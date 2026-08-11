export type ClientSplitPaneVerticalCommandId = "shell.clientSplitPaneVertical";
export type ClientSplitPaneHorizontalCommandId = "shell.clientSplitPaneHorizontal";
export type ClientSplitPaneRightCommandId = "shell.clientSplitPaneRight";
export type ClientSplitPaneDownCommandId = "shell.clientSplitPaneDown";
export type ClientAddEqualPaneCommandId = "shell.clientAddEqualPane";
export type ClientClosePaneCommandId = "shell.clientClosePane";
export type ClientFocusPaneNextCommandId = "shell.clientFocusPaneNext";
export type ClientFocusPanePrevCommandId = "shell.clientFocusPanePrev";
export type ClientResizePaneLeftCommandId = "shell.clientResizePaneLeft";
export type ClientResizePaneRightCommandId = "shell.clientResizePaneRight";
export type ClientResizePaneUpCommandId = "shell.clientResizePaneUp";
export type ClientResizePaneDownCommandId = "shell.clientResizePaneDown";
export type ClientMovePaneNextCommandId = "shell.clientMovePaneNext";
export type ClientMovePanePrevCommandId = "shell.clientMovePanePrev";
export type ClientTabNextCommandId = "shell.clientTabNext";
export type ClientTabPrevCommandId = "shell.clientTabPrev";
export type ClientTabNewCommandId = "shell.clientTabNew";
export type ClientTabCloseCommandId = "shell.clientTabClose";
export type ClientTabMoveLeftCommandId = "shell.clientTabMoveLeft";
export type ClientTabMoveRightCommandId = "shell.clientTabMoveRight";
/** 1-based tab positions: only 1..9 exist ("beyond 9" is not a command ID). */
export type ClientTabPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
export type ClientTabActivateCommandId = `shell.clientTabActivate.${ClientTabPosition}`;
export type ClientTabMoveToCommandId = `shell.clientTabMoveTo.${ClientTabPosition}`;

export declare function clientSplitPaneVertical(): ClientSplitPaneVerticalCommandId;
export declare function clientSplitPaneHorizontal(): ClientSplitPaneHorizontalCommandId;
export declare function clientSplitPaneRight(): ClientSplitPaneRightCommandId;
export declare function clientSplitPaneDown(): ClientSplitPaneDownCommandId;
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
