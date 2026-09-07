// Active icon-pack wire types (Plan 112 task 6). Bounded d-string geometry is
// the canonical contract shape; parsed command internals never cross the wire.

export interface IconPathData {
  d: string;
  opacity?: number;
}

export interface IconGeometry {
  viewBox: [number, number, number, number];
  paths: IconPathData[];
}

export interface IconPackSnapshot {
  specifier: string;
  schemaVersion: number;
  generation: number;
  provenance: {
    packageName: string;
    packageVersion: string;
    apiPrefix: string;
    trustDomain: string;
  };
  icons: Record<string, IconGeometry>;
}
