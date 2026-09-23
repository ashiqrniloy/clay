import type { PackageAction, SduiActionIntent } from "./types";

export type IntentSender = (payload: string) => Promise<void>;

export function sduiActionPayload(
  uiVersion: number,
  intent: SduiActionIntent,
): string {
  return JSON.stringify({
    family: "sduiAction",
    payload: { clientId: 0, uiVersion, intent },
  });
}

function sourceId(id: string): number {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(id)) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return Math.max(1, hash);
}

function argumentsFrom(
  values: Record<string, string | number | boolean> | undefined,
) {
  return Object.entries(values ?? {}).map(([name, value]) => ({
    name,
    value:
      typeof value === "string"
        ? { string: value }
        : typeof value === "boolean"
          ? { bool: value }
          : { i64: value },
  }));
}

export function packageIntent(
  action: PackageAction,
  componentId: string,
  itemId?: string,
  extra?: Record<string, string | number | boolean>,
): SduiActionIntent {
  const nodeId = sourceId(componentId);
  return {
    commandId: action.commandId,
    source:
      itemId == null
        ? { button: { nodeId } }
        : { listItem: { nodeId, itemId } },
    arguments: argumentsFrom({ ...action.arguments, ...extra }),
  };
}
