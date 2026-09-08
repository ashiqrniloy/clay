/**
 * Phase 1 coding tools: the nine Prism coding tools plus ask_user_decision,
 * backed by Clay document reverse-RPC for read/write/edit and the logged
 * default acceptance policy (2026-08-30-2157): inside workspace free,
 * outside-workspace reads free, outside-workspace mutations need approval
 * unless full autonomy is enabled for the session.
 */
import type { ExecutionAction, ExecutionDecision, ExecutionPolicy, JsonObject, ToolDefinition, ToolExecutionContext, ToolResult } from "@arnilo/prism";
import {
  createAskUserDecisionTool,
  createCodingTools,
  type AskUserDecisionAnswer,
  type AskUserDecisionRequest,
} from "@arnilo/prism-coding-tools/agent";
import {
  assertPathInsideRoots,
  evaluateCommandRules,
} from "@arnilo/prism-coding-tools/security";
import { createClayDocumentOps, type ReverseRequest } from "./document-ops.js";

/** Repository scan caps user-configurable via tool-caps.json (decision
 *  2026-09-05): partial overrides over Prism defaults, clamped to Prism's
 *  hard caps by resolveRepositoryLimits. Keys mirror Prism option names. */
export interface RepositoryToolCaps {
  readonly exclude?: readonly string[];
  readonly maxEntries?: number;
  readonly maxFiles?: number;
  readonly maxDepth?: number;
  readonly maxResults?: number;
  readonly maxScanBytes?: number;
  readonly maxMatches?: number;
}

const BASE_REPO_EXCLUDE = Object.freeze([".git", "node_modules", "dist", "target"]);

/** truncatedBy value → config key + Prism hard cap, for walk/scan wall-hits.
 *  Pagination (`results`, `matches`) is not a cap: the tool already tells the
 *  model to pass offset/maxResults. Stamping those as errors made repo_list
 *  with maxResults:1 look broken. */
const CAP_REMEDY: Record<string, { key: string; hard: number }> = {
  entries: { key: "maxEntries", hard: 100_000 },
  files: { key: "maxFiles", hard: 100_000 },
  depth: { key: "maxDepth", hard: 128 },
  bytes: { key: "maxScanBytes", hard: 1_073_741_824 },
  scan: { key: "maxScanBytes", hard: 1_073_741_824 },
  time: { key: "maxTimeMs", hard: 3_600_000 },
};

/** Repo tools whose truncation metadata must surface as a user-visible
 *  error naming the remedy. */
const REPO_TOOL_NAMES = new Set(["repo_list", "repo_search", "glob"]);

/** Translate Prism truncation metadata into an error result that tells the
 *  user which cap was hit, where to raise it, and the hard ceiling. */
function withCapErrors(tools: ToolDefinition[], capsFile: string | undefined): ToolDefinition[] {
  if (!capsFile) return tools;
  return tools.map((tool) => {
    if (!REPO_TOOL_NAMES.has(tool.name)) return tool;
    const execute = tool.execute.bind(tool);
    return {
      ...tool,
      execute: async (args: JsonObject, context: ToolExecutionContext): Promise<ToolResult> => {
        const result = await execute(args, context);
        const metadata = result?.metadata as { truncated?: boolean; truncatedBy?: string } | undefined;
        const by = metadata?.truncated ? metadata.truncatedBy : undefined;
        const remedy = by !== undefined && by !== "" ? CAP_REMEDY[by] : undefined;
        if (!remedy) return result;
        const message =
          `Clay hit a repository scan cap (${remedy.key}). ` +
          `Raise it by adding {"${remedy.key}": <n>} to ${capsFile} ` +
          `(hard ceiling ${remedy.hard.toLocaleString("en-US")}) and restarting Clay.`;
        return {
          ...result,
          content: [...(result.content ?? []), { type: "text", text: message }],
          error: { message },
        };
      },
    };
  });
}

/** Validate + normalize a parsed tool-caps.json document. Unknown keys are
 *  dropped; malformed values are rejected so the caller can warn and fall
 *  back to defaults instead of passing junk into Prism. */
export function normalizeToolCaps(raw: unknown): RepositoryToolCaps {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return {};
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    if (key === "exclude") {
      if (Array.isArray(value) && value.every((v) => typeof v === "string")) {
        out.exclude = Object.freeze([...value]);
      }
      continue;
    }
    if (
      ["maxEntries", "maxFiles", "maxDepth", "maxResults", "maxScanBytes", "maxMatches"].includes(key) &&
      typeof value === "number" &&
      Number.isInteger(value) &&
      value > 0
    ) {
      out[key] = value;
    }
  }
  return out as RepositoryToolCaps;
}

/** Mirrors Prism's internal mutating-kind table (delete/move included). */
function isMutatingKind(kind: string): boolean {
  return kind === "shell" || kind === "write" || kind === "edit" || kind === "delete" || kind === "move";
}

const APPROVAL_TIMEOUT_MS = 30_000;

function withTimeout(promise: Promise<boolean>): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    const timer = setTimeout(() => resolve(false), APPROVAL_TIMEOUT_MS);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      () => {
        clearTimeout(timer);
        resolve(false);
      },
    );
  });
}

/** Symlink-aware containment against the realpaths of the roots. */
async function insideRoots(roots: readonly string[], paths: readonly string[]): Promise<boolean> {
  for (const path of paths) {
    if (!(await assertPathInsideRoots(roots, path))) return false;
  }
  return true;
}

export interface ClayAcceptancePolicyOptions {
  /** Workspace roots; inside-root mutations run free (2157). */
  readonly roots: readonly string[];
  /** Live per-session full-autonomy flag (default off). */
  readonly fullAutonomy: () => boolean;
  /**
   * Host approval callback for outside-workspace mutations. Omission fails
   * closed. Production wires this to the server approval path; tests inject
   * a stub.
   */
  readonly approve?: (action: ExecutionAction) => boolean | Promise<boolean>;
}

function approveGate(
  action: ExecutionAction,
  options: ClayAcceptancePolicyOptions,
): Promise<ExecutionDecision> {
  if (options.fullAutonomy()) {
    return Promise.resolve({ allowed: true, exclusive: action.kind === "shell" });
  }
  if (!options.approve) {
    return Promise.resolve({ allowed: false, reason: "approval required" });
  }
  return withTimeout(Promise.resolve(options.approve(action))).then((approved) =>
    approved
      ? { allowed: true, exclusive: action.kind === "shell" }
      : { allowed: false, reason: "approval denied" },
  );
}

/**
 * Default Clay acceptance policy. Deliberately not Prism's
 * `createCodingApprovalPolicy`: that one rejects out-of-root paths outright,
 * while 2157 routes them to host approval. Reads are free everywhere;
 * in-root mutations are free; out-of-root mutations need approval unless
 * full autonomy is on. No approval cache: every out-of-root mutation asks
 * again (safer; run-scope caching returns with durable runs in task 7).
 */
export function createClayAcceptancePolicy(options: ClayAcceptancePolicyOptions): ExecutionPolicy {
  return {
    async check(action: ExecutionAction): Promise<ExecutionDecision> {
      if (options.roots.length === 0) {
        return { allowed: false, reason: "no trusted roots configured" };
      }
      if (action.kind === "shell" && action.command) {
        // Workspace-cwd shell: deny metacharacters, otherwise free (2157).
        const evaluation = evaluateCommandRules(action.command, [], { denyMetacharacters: true });
        if (evaluation.action === "deny") {
          return { allowed: false, reason: evaluation.reason ?? "command denied by policy" };
        }
        if (evaluation.action === "requireApproval") {
          return approveGate(action, options);
        }
        return { allowed: true, exclusive: true };
      }
      if (!isMutatingKind(action.kind)) {
        // Reads are free inside and outside the workspace (2157).
        return { allowed: true };
      }
      if (await insideRoots(options.roots, action.paths ?? [])) {
        return { allowed: true };
      }
      return approveGate(action, options);
    },
  };
}

export const CODING_TOOL_NAMES = [
  "shell",
  "read",
  "write",
  "edit",
  "repo_list",
  "repo_search",
  "glob",
  "delete",
  "move",
  "ask_user_decision",
] as const;

export interface CodingToolsOptions {
  /** Workspace root bound to the tool cwd. */
  readonly workspaceRoot: string;
  /** Reverse-RPC transport for document read/write/edit backends. */
  readonly request: ReverseRequest;
  /** Live full-autonomy flag for the session. */
  readonly fullAutonomy: () => boolean;
  /** User-configured repository scan caps (tool-caps.json). */
  readonly toolCaps?: RepositoryToolCaps;
  /** Caps file path, cited in truncation errors so users can raise caps. */
  readonly capsFile?: string;
  /** Host approval callback (optional; omission fails closed out-of-root). */
  readonly approve?: (action: ExecutionAction) => boolean | Promise<boolean>;
  /** Host ask callback for ask_user_decision (optional; omit to skip tool). */
  readonly ask?: (request: AskUserDecisionRequest) => Promise<AskUserDecisionAnswer>;
}

/**
 * Build the Phase 1 coding tool set for one session. Inert until the caller
 * registers the returned definitions on an agent; Chat profiles never list
 * these tools, so Chat stays no-tools.
 */
export function buildCodingTools(options: CodingToolsOptions): ToolDefinition[] {
  const ops = createClayDocumentOps({ request: options.request });
  const policy = createClayAcceptancePolicy({
    roots: [options.workspaceRoot],
    fullAutonomy: options.fullAutonomy,
    ...(options.approve ? { approve: options.approve } : {}),
  });
  const tools = withCapErrors(
    [
      ...createCodingTools(options.workspaceRoot, {
        executionPolicy: policy,
        // Defaults exclude build trees so the scan budget reaches real
        // sources; tool-caps.json overrides caps/exclude per user config.
        repository: {
          exclude: [...(options.toolCaps?.exclude ?? BASE_REPO_EXCLUDE)],
          ...Object.fromEntries(
            Object.entries(options.toolCaps ?? {}).filter(([k]) => k !== "exclude"),
          ),
        },
        read: { operations: ops.read },
        write: { operations: ops.write },
        edit: { operations: ops.edit },
      }),
    ],
    options.capsFile,
  );
  if (options.ask) {
    tools.push(createAskUserDecisionTool({ ask: options.ask }));
  }
  return tools;
}