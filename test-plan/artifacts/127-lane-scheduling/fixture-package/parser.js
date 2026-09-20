// Deliberately slow package parse handler (Plan 127 manual fixture).
//
// BUSY_MS is the lane-hold: while it runs, the third-party general lane cannot
// run any other command in that domain. A completion provider that shares that
// lane (inline module-object registration) waits for it; a latency-lane
// provider (moduleSpecifier registration) does not.
const BUSY_MS = Number(globalThis.clayLaneBusyMs ?? 1000);

export async function parseLaneDocument(notification) {
  const deadline = Date.now() + BUSY_MS;
  while (Date.now() < deadline) {
    // Burn the lane on purpose; nothing else runs in this domain until done.
  }
  return { viewport: notification?.viewport ?? null };
}

export default parseLaneDocument;