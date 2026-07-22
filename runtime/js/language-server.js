// Configuration-only language-server authority facade.
const ops = globalThis.Deno?.core?.ops;
/** Approve one fixed package contribution for current directory workspace roots.
 * Call from init.js before the first loadPackage call. Package loading seals
 * authority mutation for this runtime generation. */
export async function authorizeLanguageServer(options) {
    if (!ops) {
        throw new Error("clay.language_server.runtime_unavailable: API requires the server runtime");
    }
    return JSON.parse(await ops.op_clay_language_server_authorize(JSON.stringify(options ?? null)));
}
/** Start an authorized language-server session for one contribution + root. */
export async function startLanguageServerSession(options) {
    if (!ops) {
        throw new Error("clay.language_server.runtime_unavailable: API requires the server runtime");
    }
    // The session owner is the host-stamped executing package; the op response
    // carries the host-resolved package/contribution identity used by the
    // bounded session calls below.
    const started = JSON.parse(await ops.op_clay_language_server_start_session(JSON.stringify(options ?? null)));
    const sessionId = started.sessionId;
    const identity = JSON.stringify({
        sessionId,
        package: started.package,
        contribution: started.contribution,
    });
    return {
        sessionId,
        sendBytes: (bytes) => {
            if (!(bytes instanceof Uint8Array)) {
                throw new Error("clay.language_server.invalid_bytes: bytes must be a Uint8Array");
            }
            return ops.op_clay_language_server_send_bytes(identity, bytes);
        },
        readBytes: (maxBytes, timeoutMs) => ops.op_clay_language_server_read_bytes(JSON.stringify({
            sessionId,
            package: started.package,
            contribution: started.contribution,
            maxBytes,
            timeoutMs,
        })),
        send: (message) => ops.op_clay_language_server_send_message(JSON.stringify({
            sessionId,
            package: started.package,
            contribution: started.contribution,
            message,
        })).then(JSON.parse),
        read: (maxBytes, timeoutMs) => ops.op_clay_language_server_read_message(JSON.stringify({
            sessionId,
            package: started.package,
            contribution: started.contribution,
            maxBytes,
            timeoutMs,
        }))
            .then(JSON.parse)
            .then((result) => result.message),
        stop: () => ops.op_clay_language_server_stop_session(identity).then(JSON.parse),
    };
}
