/**
 * Fire-and-forget shell calls (plan 119 further action).
 *
 * Optimistic controls (menus, tab chrome, SDUI intents, layout persistence)
 * must not make the UI wait on a round trip: a refused request is a dropped
 * command, not a crash. Without a handler the rejected promise surfaces as an
 * unhandled rejection — Vitest fails the run on it and the webview reports an
 * unhandled promise — while the user sees nothing.
 *
 * Callers that *wait* on a bridge call (document sessions, tab open/close
 * flows) keep their own error paths and never route through here.
 */
export function detached(promise: Promise<unknown> | undefined): void {
  void promise?.catch((error: unknown) => {
    if (import.meta.env.DEV) {
      console.warn("clay: detached call failed", error);
    }
  });
}
