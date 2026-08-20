export type FoldingRangeInput = {
    byteStart: number;
    byteEnd: number;
    label?: string;
};
export type ServerPublishFoldingRangesOptions = {
    documentId: number;
    documentVersion: number;
    currentDocumentVersion?: number;
    packagePrefix?: string;
    ranges: FoldingRangeInput[];
};
export declare function serverPublishFoldingRanges(
    options: ServerPublishFoldingRangesOptions,
): unknown;
