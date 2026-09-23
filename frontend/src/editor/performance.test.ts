import { describe, expect, it, vi } from "vitest";

import {
  PERFORMANCE_EVENT_CAPACITY,
  PERFORMANCE_STAGE,
  PerformanceRecorder,
} from "./performance";

describe("PerformanceRecorder", () => {
  it("does no work or retain events while disabled", () => {
    const recorder = new PerformanceRecorder();
    const traceId = recorder.trace();
    recorder.mark(PERFORMANCE_STAGE.editorTyping, traceId);
    recorder.span(PERFORMANCE_STAGE.editorTyping, traceId).end();

    expect(recorder.snapshot()).toMatchObject({
      enabled: false,
      retainedEvents: 0,
      droppedEvents: 0,
    });
  });

  it("keeps duration percentiles and counts source-free", () => {
    const recorder = new PerformanceRecorder(true);
    const traceId = recorder.trace();
    let elapsed = 0;
    vi.spyOn(performance, "now").mockImplementation(() => elapsed);

    for (const duration of [1, 2, 4]) {
      const span = recorder.span(PERFORMANCE_STAGE.editorTyping, traceId, {
        documentId: 7,
        version: 3,
        feature: "typing",
      });
      elapsed += duration;
      span.end();
    }

    recorder.mark(PERFORMANCE_STAGE.editorTyping, traceId, {
      feature: "secret source text /home/alice/note.md",
    });
    const snapshot = recorder.snapshot();
    const metric = snapshot.metrics[PERFORMANCE_STAGE.editorTyping];

    expect(metric).toMatchObject({
      count: 4,
      samples: 3,
      p50Ms: 2,
      p95Ms: 4,
      maxMs: 4,
    });
    expect(JSON.stringify(snapshot)).not.toContain("/home/alice");
    expect(JSON.stringify(snapshot)).not.toContain("secret source text");
    vi.restoreAllMocks();
  });

  it("keeps one viewport trace in stage order", () => {
    const recorder = new PerformanceRecorder(true);
    const traceId = recorder.trace();
    const stages = [
      PERFORMANCE_STAGE.browserViewport,
      PERFORMANCE_STAGE.codemirrorUpdate,
      PERFORMANCE_STAGE.bridgeEnqueue,
      PERFORMANCE_STAGE.bridgeClientDelivery,
      PERFORMANCE_STAGE.serverReceive,
      PERFORMANCE_STAGE.syntaxQueue,
      PERFORMANCE_STAGE.syntaxStart,
      PERFORMANCE_STAGE.syntaxEnd,
      PERFORMANCE_STAGE.patchDelivery,
      PERFORMANCE_STAGE.bridgeServerDelivery,
      PERFORMANCE_STAGE.bridgeForwarderDelivery,
      PERFORMANCE_STAGE.patchApply,
      PERFORMANCE_STAGE.paintAdjacent,
    ];
    for (const stage of stages) recorder.mark(stage, traceId);

    expect(
      recorder
        .snapshot()
        .events.filter((event) => event.traceId === traceId)
        .map((event) => event.stage),
    ).toEqual(stages);
  });

  it("drops events after its fixed capacity", () => {
    const recorder = new PerformanceRecorder(true);
    const traceId = recorder.trace();
    for (let index = 0; index <= PERFORMANCE_EVENT_CAPACITY; index += 1)
      recorder.mark(PERFORMANCE_STAGE.reactCommit, traceId);

    expect(recorder.snapshot()).toMatchObject({
      retainedEvents: PERFORMANCE_EVENT_CAPACITY,
      droppedEvents: 1,
    });
  });
});
