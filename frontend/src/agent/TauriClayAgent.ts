// Custom AG-UI transport over Tauri channels (Plan 097 Phase 10).
//
// `TauriClayAgent` extends `AbstractAgent` from `@ag-ui/client`; the run
// pipeline (chunk expansion, event verification, message/state application)
// is entirely upstream. This class only bridges transports:
// - Outgoing: composer text becomes the existing server-validated
//   `chat.submit` intent through the typed session bridge.
// - Incoming: one shared relay stream carries Rust-adapted AG-UI events; a
//   run observable forwards exactly its own run and completes at the
//   terminal lifecycle event.

import { AbstractAgent } from "@ag-ui/client";
import { Observable, type Subscription } from "rxjs";

import type { BaseEvent, RunAgentInput } from "@ag-ui/core";

import { pipeRelay } from "./events";
import { sendRequest } from "../bridge/client";

const RUN_TERMINAL = new Set(["RUN_FINISHED", "RUN_ERROR"]);
/** Events that may precede this run's RUN_STARTED on the shared relay. */
const PRE_RUN_NOISE = new Set([
  "STATE_SNAPSHOT",
  "MESSAGES_SNAPSHOT",
  "CUSTOM",
]);

export interface ChatIntentContext {
  /** Current package UI version for intent validation. */
  uiVersion: number;
}

/**
 * Builds the `sduiAction` payload for a chat command with a string argument.
 * Mirrors `packageIntent`/`sduiActionPayload` from the SDUI layer without a
 * declared node (the chat surface is host-rendered).
 */
function chatIntentPayload(
  uiVersion: number,
  commandId: string,
  value?: string,
): string {
  return JSON.stringify({
    family: "sduiAction",
    payload: {
      clientId: 0,
      uiVersion,
      intent: {
        commandId,
        source: { button: { nodeId: 1 } },
        arguments:
          value === undefined
            ? []
            : [{ name: "value", value: { string: value } }],
      },
    },
  });
}

export class TauriClayAgent extends AbstractAgent {
  private pendingPrompt: string | null = null;
  private uiVersion = 0;

  setUiVersion(uiVersion: number) {
    this.uiVersion = uiVersion;
  }

  /** Queues composer text for the next `runAgent()` call. */
  sendPrompt(text: string) {
    this.pendingPrompt = text;
  }

  run(input: RunAgentInput): Observable<BaseEvent> {
    // `input.messages` stays server-authoritative: the intent carries only
    // composer text, and the daemon owns conversation context.
    void input;
    const prompt = this.pendingPrompt ?? "";
    this.pendingPrompt = null;
    const uiVersion = this.uiVersion;
    return new Observable<BaseEvent>((subscriber) => {
      // Empty submits are server-side no-ops: nothing will stream.
      if (!prompt.trim()) {
        subscriber.complete();
        return;
      }
      let started = false;
      let relaySubscription: Subscription | null = null;
      relaySubscription = pipeRelay({
        next: (event) => {
          if (subscriber.closed) return;
          if (!started) {
            if (event.type === "RUN_STARTED") {
              started = true;
            } else if (
              PRE_RUN_NOISE.has(event.type) ||
              // Cancelled before the daemon opened the run.
              (event.type === "CUSTOM" &&
                (event as { name?: string }).name === "clay.diagnostic")
            ) {
              return;
            }
          }
          subscriber.next(event);
          if (RUN_TERMINAL.has(event.type)) {
            subscriber.complete();
          }
        },
        error: (error) => {
          if (!subscriber.closed) subscriber.error(error);
        },
      });
      // Fire the validated server intent; streaming arrives over the relay.
      void sendRequest(
        chatIntentPayload(uiVersion, "chat.submit", prompt),
      ).catch((error) => {
        if (!subscriber.closed) subscriber.error(error);
      });
      return () => relaySubscription?.unsubscribe();
    });
  }

  override abortRun() {
    void sendRequest(chatIntentPayload(this.uiVersion, "chat.cancel")).catch(
      () => {
        // Server unreachable; the disconnect flow owns recovery.
      },
    );
  }
}
