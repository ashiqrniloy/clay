export interface QuitOptions {
    /** Planned: quit without the save/confirm prompts. */
    force?: boolean;
    /** Planned: reason recorded for diagnostics and shutdown prompts. */
    reason?: string;
}
export interface QuitResult {
    requested: boolean;
}
export declare function quit(options?: QuitOptions): Promise<QuitResult>;
