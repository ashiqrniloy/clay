import type { BridgeErrorDto } from "./types";

/** Normalizes anything thrown across the bridge into a stable shape. */
export function normalizeBridgeError(error: unknown): BridgeErrorDto {
  if (isBridgeErrorDto(error)) return error;
  const message = error instanceof Error ? error.message : String(error);
  return { code: "invalidRequest", message: truncate(message) };
}

function isBridgeErrorDto(value: unknown): value is BridgeErrorDto {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as { code?: unknown; message?: unknown };
  return (
    typeof candidate.code === "string" && typeof candidate.message === "string"
  );
}

const MAX_MESSAGE_CHARS = 240;

function truncate(message: string): string {
  return message.length <= MAX_MESSAGE_CHARS
    ? message
    : message.slice(0, MAX_MESSAGE_CHARS);
}
