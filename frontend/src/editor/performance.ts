export const PERFORMANCE_SCHEMA_VERSION = 1;
export const PERFORMANCE_EVENT_CAPACITY = 4096;

export const PERFORMANCE_STAGE = {
  browserInput: "browser.input",
  codemirrorUpdate: "codemirror.update",
  editorTyping: "editor.typing",
  browserViewport: "browser.viewport",
  editorScroll: "editor.scroll",
  editorSyntaxFresh: "editor.syntax_fresh",
  reactCommit: "react.commit",
  compartmentReconfigure: "editor.compartment_reconfigure",
  longTask: "editor.long_task",
  bridgeEnqueue: "bridge.enqueue",
  bridgeClientDelivery: "bridge.client_delivery",
  bridgeServerDelivery: "bridge.server_delivery",
  bridgeForwarderDelivery: "bridge.forwarder_delivery",
  serverReceive: "server.receive",
  serverEditAck: "server.edit_ack",
  syntaxQueue: "syntax.queue",
  syntaxStart: "syntax.start",
  syntaxEnd: "syntax.end",
  patchDelivery: "bridge.patch_delivery",
  patchApply: "editor.patch_apply",
  editorOpen: "editor.open",
  editorReady: "editor.ready",
  paintAdjacent: "editor.paint_adjacent",
} as const;

export interface PerformanceMetadata {
  documentId?: number;
  version?: number;
  transactionId?: number;
  byteCount?: number;
  feature?: string;
}

export interface PerformanceEvent {
  traceId: number;
  stage: string;
  durationMs?: number;
  documentId?: number;
  version?: number;
  transactionId?: number;
  byteCount?: number;
  feature?: string;
}

export interface PerformanceMetricSummary {
  count: number;
  samples: number;
  p50Ms: number;
  p95Ms: number;
  maxMs: number;
}

export interface PerformanceSnapshot {
  schemaVersion: number;
  enabled: boolean;
  retainedEvents: number;
  droppedEvents: number;
  events: PerformanceEvent[];
  metrics: Record<string, PerformanceMetricSummary>;
}

export interface PerformanceSpan {
  end(metadata?: PerformanceMetadata): void;
}

const NOOP_SPAN: PerformanceSpan = { end: () => undefined };
const FEATURE_PATTERN = /^[a-zA-Z0-9._-]{1,64}$/;

function now(): number {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

function safeInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : undefined;
}

function safeFeature(value: unknown): string | undefined {
  return typeof value === "string" && FEATURE_PATTERN.test(value)
    ? value
    : undefined;
}

function percentile(sorted: number[], percent: number): number {
  if (sorted.length === 0) return 0;
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((sorted.length * percent) / 100) - 1),
  );
  return sorted[index] ?? 0;
}

/**
 * Disabled-by-default, source-free browser recorder. It retains only bounded
 * numeric events and sanitized stage/feature names; callers do not construct
 * payload metadata until this recorder is enabled.
 */
export class PerformanceRecorder {
  private enabled: boolean;
  private nextTraceId = 1;
  private events: PerformanceEvent[] = [];
  private droppedEvents = 0;
  private longTaskObserver: PerformanceObserver | null = null;

  constructor(enabled = false) {
    this.enabled = enabled;
    if (enabled) this.installLongTaskObserver();
  }

  configure(enabled: boolean): void {
    if (this.enabled !== enabled) this.clear();
    this.enabled = enabled;
    if (enabled) this.installLongTaskObserver();
    else this.longTaskObserver?.disconnect();
    if (!enabled) this.longTaskObserver = null;
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  clear(): void {
    this.events = [];
    this.droppedEvents = 0;
  }

  trace(): number {
    if (!this.enabled) return 0;
    const traceId = this.nextTraceId;
    this.nextTraceId = traceId === Number.MAX_SAFE_INTEGER ? 1 : traceId + 1;
    return traceId;
  }

  mark(
    stage: string,
    traceId: number,
    metadata: PerformanceMetadata = {},
  ): void {
    if (!this.enabled) return;
    this.record(stage, traceId || this.trace(), undefined, metadata);
  }

  count(stage: string, traceId = 0, metadata: PerformanceMetadata = {}): void {
    this.mark(stage, traceId, metadata);
  }

  sample(
    stage: string,
    traceId: number,
    durationMs: number,
    metadata: PerformanceMetadata = {},
  ): void {
    if (!this.enabled) return;
    this.record(
      stage,
      traceId || this.trace(),
      Math.max(0, durationMs),
      metadata,
    );
  }

  span(
    stage: string,
    traceId: number,
    metadata: PerformanceMetadata = {},
  ): PerformanceSpan {
    if (!this.enabled || traceId <= 0) return NOOP_SPAN;
    const started = now();
    let ended = false;
    return {
      end: (endingMetadata = {}) => {
        if (ended) return;
        ended = true;
        this.record(stage, traceId, Math.max(0, now() - started), {
          ...metadata,
          ...endingMetadata,
        });
      },
    };
  }

  frame(traceId: number, metadata: PerformanceMetadata = {}): void {
    if (!this.enabled || traceId <= 0) return;
    const started = now();
    const complete = () =>
      this.record(
        PERFORMANCE_STAGE.paintAdjacent,
        traceId,
        Math.max(0, now() - started),
        metadata,
      );
    if (typeof requestAnimationFrame === "function")
      requestAnimationFrame(complete);
    else setTimeout(complete, 0);
  }

  snapshot(): PerformanceSnapshot {
    const metrics = new Map<string, { count: number; durations: number[] }>();
    for (const event of this.events) {
      const metric = metrics.get(event.stage) ?? { count: 0, durations: [] };
      metric.count += 1;
      if (event.durationMs !== undefined)
        metric.durations.push(event.durationMs);
      metrics.set(event.stage, metric);
    }
    const summaries: Record<string, PerformanceMetricSummary> = {};
    for (const [stage, metric] of metrics) {
      const durations = [...metric.durations].sort(
        (left, right) => left - right,
      );
      summaries[stage] = {
        count: metric.count,
        samples: durations.length,
        p50Ms: percentile(durations, 50),
        p95Ms: percentile(durations, 95),
        maxMs: durations.at(-1) ?? 0,
      };
    }
    return {
      schemaVersion: PERFORMANCE_SCHEMA_VERSION,
      enabled: this.enabled,
      retainedEvents: this.events.length,
      droppedEvents: this.droppedEvents,
      events: this.events.map((event) => ({ ...event })),
      metrics: summaries,
    };
  }

  private installLongTaskObserver(): void {
    if (this.longTaskObserver || typeof PerformanceObserver === "undefined")
      return;
    try {
      const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries())
          this.sample(
            PERFORMANCE_STAGE.longTask,
            this.trace(),
            entry.duration,
            { feature: "longTask" },
          );
      });
      observer.observe({ type: "longtask", buffered: true });
      this.longTaskObserver = observer;
    } catch {
      // WebKit versions without Long Tasks keep all other marks available.
    }
  }

  private record(
    stage: string,
    traceId: number,
    durationMs: number | undefined,
    metadata: PerformanceMetadata,
  ): void {
    if (!this.enabled || traceId <= 0) return;
    if (this.events.length >= PERFORMANCE_EVENT_CAPACITY) {
      this.droppedEvents += 1;
      return;
    }
    const event: PerformanceEvent = {
      traceId: safeInteger(traceId) ?? 0,
      stage: safeFeature(stage) ?? "unknown",
    };
    if (event.traceId === 0) return;
    const documentId = safeInteger(metadata.documentId);
    const version = safeInteger(metadata.version);
    const transactionId = safeInteger(metadata.transactionId);
    const byteCount = safeInteger(metadata.byteCount);
    const feature = safeFeature(metadata.feature);
    if (durationMs !== undefined) event.durationMs = durationMs;
    if (documentId !== undefined) event.documentId = documentId;
    if (version !== undefined) event.version = version;
    if (transactionId !== undefined) event.transactionId = transactionId;
    if (byteCount !== undefined) event.byteCount = byteCount;
    if (feature !== undefined) event.feature = feature;
    this.events.push(event);
  }
}

export const editorPerformance = new PerformanceRecorder(
  typeof import.meta !== "undefined" &&
    import.meta.env?.VITE_CLAY_PERF_PROFILE === "1",
);

// Editor performance harness: flush the source-free frontend snapshot
// through the desktop shell while the run is live. The Tauri command no-ops
// unless CLAY_PERF_REPORT_DIR is set (production default), and this code only
// ships in builds made with VITE_CLAY_PERF_PROFILE=1. Flushing on an interval
// (not only on pagehide) avoids losing the report to teardown races.
if (
  typeof window !== "undefined" &&
  import.meta.env?.VITE_CLAY_PERF_PROFILE === "1"
) {
  const flushReport = () => {
    const label = new URLSearchParams(window.location.search).get("perfLabel");
    void import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke("write_frontend_perf_report", {
          label,
          snapshot: editorPerformance.snapshot(),
        }),
      )
      .catch(() => undefined);
  };
  let flushedEvents = -1;
  window.setInterval(() => {
    const retained = editorPerformance.snapshot().retainedEvents;
    if (retained !== flushedEvents) {
      flushedEvents = retained;
      flushReport();
    }
  }, 10_000);
  window.addEventListener("pagehide", flushReport, { once: true });
}
