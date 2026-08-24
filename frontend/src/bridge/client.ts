// Frontend entry point to the typed bridge. Everything Tauri-specific lives
// here; the rest of the app consumes the store and dispatcher only.

import { Channel, invoke } from "@tauri-apps/api/core";

import { normalizeBridgeError } from "./errors";
import type { BridgeEnvelope, BootstrapDto } from "./types";

/**
 * Single-flight bootstrap: React StrictMode double-mounts effects in dev,
 * and a second concurrent `session_bootstrap` would otherwise return `busy`
 * and clobber the connection store with a false disconnect. All concurrent
 * callers share one in-flight bootstrap; `session_reconnect` stays explicit.
 */
let inFlightBootstrap: Promise<BootstrapDto> | null = null;

export function bootstrapSession(): Promise<BootstrapDto> {
  if (!inFlightBootstrap) {
    inFlightBootstrap = invoke<BootstrapDto>("session_bootstrap")
      .catch((error) => {
        throw normalizeBridgeError(error);
      })
      .finally(() => {
        inFlightBootstrap = null;
      });
  }
  return inFlightBootstrap;
}

export async function reconnectSession(): Promise<BootstrapDto> {
  try {
    return await invoke<BootstrapDto>("session_reconnect");
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

/** Subscribes the webview channel; replaces any previous subscription. */
export async function subscribeToEvents(
  onEnvelope: (envelope: BridgeEnvelope) => void,
): Promise<void> {
  const channel = new Channel<BridgeEnvelope>();
  channel.onmessage = onEnvelope;
  try {
    await invoke("session_subscribe", { onEvent: channel });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function unsubscribeFromEvents(): Promise<void> {
  try {
    await invoke("session_unsubscribe");
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

/**
 * Sends one typed request. `payload` is the JSON text of a protocol
 * `ClientMessage`; the Rust bridge size-caps, parses, stamps the session
 * identity, and forwards it.
 */
export async function sendRequest(
  payload: string,
  tabId?: number,
): Promise<void> {
  try {
    await invoke("session_request", { payload, tabId: tabId ?? null });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function openTab(workspaceRoot: string): Promise<BootstrapDto> {
  try {
    return await invoke<BootstrapDto>("tab_open", { workspaceRoot });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function closeTab(tabId: number): Promise<void> {
  try {
    await invoke("tab_close", { tabId });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function activateTab(tabId: number): Promise<void> {
  try {
    await invoke("tab_activate", { tabId });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function openFileDialog(tabId?: number): Promise<boolean> {
  try {
    return await invoke<boolean>("dialog_open_file", { tabId: tabId ?? null });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function openFolderDialog(tabId?: number): Promise<boolean> {
  try {
    return await invoke<boolean>("dialog_open_folder", {
      tabId: tabId ?? null,
    });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function openTabDialog(): Promise<BootstrapDto | null> {
  try {
    return await invoke<BootstrapDto | null>("tab_open_dialog");
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function loadLayout(): Promise<unknown> {
  try {
    return await invoke("layout_load");
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}

export async function saveLayout(state: unknown): Promise<void> {
  try {
    await invoke("layout_save", { state });
  } catch (error) {
    throw normalizeBridgeError(error);
  }
}
