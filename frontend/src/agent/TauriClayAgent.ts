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
  thinkingLevel?: string,
): string {
  // Plan 109 I4: the prompt's portable thinking level rides the intent as a
  // second named argument; the server forwards it to the daemon, which
  // fail-closes invalid strings at its boundary.
  const args: Array<{ name: string; value: { string: string } }> =
    value === undefined
      ? []
      : [{ name: "value", value: { string: value } }];
  if (thinkingLevel !== undefined) {
    args.push({ name: "thinkingLevel", value: { string: thinkingLevel } });
  }
  return JSON.stringify({
    family: "sduiAction",
    payload: {
      clientId: 0,
      uiVersion,
      intent: {
        commandId,
        source: { button: { nodeId: 1 } },
        arguments: args,
      },
    },
  });
}

type AgentSender = (payload: string) => Promise<void>;

export class TauriClayAgent extends AbstractAgent {
  private pendingPrompt: string | null = null;
  private pendingEffort: string | undefined = undefined;
  private uiVersion = 0;
  private sender: AgentSender = sendRequest;

  setUiVersion(uiVersion: number) {
    this.uiVersion = uiVersion;
  }

  /** Pane-scoped sender (stamps tab id). Falls back to the process sendRequest. */
  setSender(sender: AgentSender) {
    this.sender = sender;
  }

  /** Queues composer text for the next `runAgent()` call. */
  sendPrompt(text: string, thinkingLevel?: string) {
    this.pendingPrompt = text;
    this.pendingEffort = thinkingLevel;
  }

  run(input: RunAgentInput): Observable<BaseEvent> {
    // `input.messages` stays server-authoritative: the intent carries only
    // composer text, and the daemon owns conversation context.
    void input;
    const prompt = this.pendingPrompt ?? "";
    this.pendingPrompt = null;
    const effort = this.pendingEffort;
    this.pendingEffort = undefined;
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
            } else if (event.type === "RUN_ERROR") {
              subscriber.next(event);
              subscriber.complete();
              return;
            } else {
              // Snapshots/diagnostics/dup noise before this run's start.
              return;
            }
          } else if (event.type === "RUN_STARTED") {
            return;
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
      void this.sender(
        chatIntentPayload(uiVersion, "chat.submit", prompt, effort),
      ).catch((error) => {
        if (!subscriber.closed) subscriber.error(error);
      });
      return () => relaySubscription?.unsubscribe();
    });
  }

  override abortRun() {
    void this.sender(chatIntentPayload(this.uiVersion, "chat.cancel")).catch(
      () => {
        // Server unreachable; the disconnect flow owns recovery.
      },
    );
  }

  /** Queues a mid-run user message (pi-parity steer, plan 108 task 9).
   *  Server-side no-op when no run is active on the tab's session. */
  steer(text: string) {
    void this.sender(chatIntentPayload(this.uiVersion, "chat.steer", text)).catch(
      () => {
        // Server unreachable; the disconnect flow owns recovery.
      },
    );
  }
}
