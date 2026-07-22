export interface QuitOptions {
    force?: boolean;
}
export interface QuitResult {
    requested: boolean;
}
export declare function quit(options?: QuitOptions): Promise<QuitResult>;
