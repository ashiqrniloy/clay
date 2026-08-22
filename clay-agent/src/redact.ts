const REDACTED = "[REDACTED]";

const SHAPES: readonly RegExp[] = [
  /sk-[A-Za-z0-9_-]{8,}/g,
  /Bearer\s+\S+/gi,
  /(?<=(?:api[_-]?key|secret|token|passphrase)\s*[:=]\s*")[^"]+/gi,
];

export function redactText(text: string, secrets: Iterable<string | undefined> = []): string {
  let out = text;
  for (const secret of secrets) {
    if (secret && secret.length > 0) out = out.split(secret).join(REDACTED);
  }
  for (const shape of SHAPES) out = out.replace(shape, REDACTED);
  return out;
}

export function redactUnknown(value: unknown, secrets: Iterable<string | undefined> = []): unknown {
  if (typeof value === "string") return redactText(value, secrets);
  if (Array.isArray(value)) return value.map((item) => redactUnknown(item, secrets));
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) out[key] = redactUnknown(item, secrets);
    return out;
  }
  return value;
}
