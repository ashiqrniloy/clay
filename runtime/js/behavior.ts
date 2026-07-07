// Clay behavior manifest facade skeleton.
//
// Behavior manifests keep hot-path client behavior inert and predictable. These
// planned APIs query or inspect manifests; they do not execute arbitrary
// JavaScript in the Rust client.

export interface BehaviorManifestSummary {
  id: string;
  documentId?: string;
  version: number;
  clientFirstBehaviors: string[];
}

export interface BehaviorRoute {
  input: string;
  runtimePath: "client-first" | "server-first" | "background";
  apiId?: string;
}

export interface CodeEditingManifestOptions {
  indentSize: number;
  lineComment?: string;
  blockCommentStart?: string;
  blockCommentEnd?: string;
  pairs?: Array<{ open: string; close: string }>;
  electricOutdentCharacters?: string[];
  autocompleteTriggers?: string[];
}

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.behavior.runtime_unavailable: Clay behavior APIs require the server runtime");
  }
  return ops;
}

export async function getActiveBehaviorManifest(documentId?: string): Promise<BehaviorManifestSummary> {
  return JSON.parse(requireOps().op_clay_behavior_get_active_manifest(JSON.stringify(documentId ?? null)));
}

export async function listBehaviorRoutes(documentId?: string): Promise<BehaviorRoute[]> {
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
export function buildCodeEditingManifest(options: CodeEditingManifestOptions): Record<string, unknown> {
  const pairs = (options.pairs ?? [
    { open: "(", close: ")" },
    { open: "[", close: "]" },
    { open: "{", close: "}" },
    { open: '"', close: '"' },
    { open: "'", close: "'" }
  ]).filter((pair) => pair.open.length > 0 && pair.close.length > 0);

  const comments: Array<Record<string, string>> = [];
  if (options.lineComment && options.lineComment.length > 0) {
    comments.push({
      linePrefix: options.lineComment,
      continuePrefix: `${options.lineComment} `
    });
  }

  const electricCharacters: Array<{ trigger: string; effect: string }> = [];
  for (const character of options.electricOutdentCharacters ?? []) {
    if (character === "}") {
      electricCharacters.push({ trigger: character, effect: "outdent-one-level" });
    }
  }

  const autocompleteTriggers: Array<{ trigger: string }> = [];
  for (const trigger of options.autocompleteTriggers ?? []) {
    if (trigger.length > 0) {
      autocompleteTriggers.push({ trigger });
    }
  }

  return {
    enter: { kind: "preserveLeadingWhitespace" },
    pairs,
    comments,
    tabSpaces: options.indentSize,
    electricCharacters,
    autocompleteTriggers
  };
}
