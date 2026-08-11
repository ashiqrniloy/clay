// Clay behavior manifest facade skeleton.
//
// Behavior manifests keep hot-path client behavior inert and predictable. These
// planned APIs query or inspect manifests; they do not execute arbitrary
// JavaScript in the Rust client.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("behavior.runtime_unavailable: Clay behavior APIs require the server runtime");
    }
    return ops;
}
export async function getActiveBehaviorManifest(documentId) {
    return JSON.parse(requireOps().op_clay_behavior_get_active_manifest(JSON.stringify(documentId ?? null)));
}
export async function listBehaviorRoutes(documentId) {
    return JSON.parse(requireOps().op_clay_behavior_list_routes(JSON.stringify(documentId ?? null)));
}
/**
 * Build a generic C-family code-editing behavior manifest from language-specific
 * parameters. The returned object is the `editorRules` shape accepted by
 * `clay:modes` registration/activation and by the server-side validator.
 *
 * The helper emits inert declarative rules only; it never produces executable
 * callbacks, client JavaScript, native handles, or raw authority fields.
 */
export function buildCodeEditingManifest(options) {
    const pairs = (options.pairs ?? [
        { open: "(", close: ")" },
        { open: "[", close: "]" },
        { open: "{", close: "}" },
        { open: '"', close: '"' },
        { open: "'", close: "'" }
    ]).filter((pair) => pair.open.length > 0 && pair.close.length > 0);
    const comments = [];
    if (options.lineComment && options.lineComment.length > 0) {
        comments.push({
            linePrefix: options.lineComment,
            continuePrefix: `${options.lineComment} `
        });
    }
    const electricCharacters = [];
    const seenElectric = new Set();
    for (const character of options.electricOutdentCharacters ?? []) {
        if ([...character].length === 1 && !seenElectric.has(character)) {
            seenElectric.add(character);
            electricCharacters.push({ trigger: character, effect: "outdent-one-level" });
        }
    }
    const autocompleteTriggers = [];
    const seenAutocomplete = new Set();
    for (const trigger of options.autocompleteTriggers ?? []) {
        if ([...trigger].length === 1 && !seenAutocomplete.has(trigger) && seenAutocomplete.size < 32) {
            seenAutocomplete.add(trigger);
            autocompleteTriggers.push({ trigger });
        }
    }
    return {
        enter: options.enter ?? { kind: "preserveLeadingWhitespace" },
        pairs,
        comments,
        tabSpaces: options.indentSize,
        electricCharacters,
        autocompleteTriggers,
        // Plan 071 task 11: optional movement/caret appearance overrides.
        // Packages pass plain declarative objects; the server-side validator
        // (modes.rs) owns field-by-field parsing and fallback, so unknown or
        // malformed values never reach the client.
        ...(isPlainObject(options.movement) ? { movement: options.movement } : {}),
        ...(isPlainObject(options.caretStyle) ? { caretStyle: options.caretStyle } : {})
    };
}

function isPlainObject(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}
