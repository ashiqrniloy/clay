// Clay agent host facade types.
export interface CompactOptions {
    sessionId: string;
    /** Compaction strategy: "default" (secret-redacting), "llm" (provider
     *  summary; requires a summary provider), "om" (observational-memory
     *  projection). Default for OM-attached sessions is "om", otherwise
     *  "default". */
    strategy?: "default" | "llm" | "om" | (string & {});
    /** Observational-memory auto-compaction threshold override in tokens
     *  (decision 2158 default 80000; Prism package default 81000). Only
     *  meaningful for OM-attached sessions; also governs post-run
     *  auto-compaction. Positive integer. */
    compactAfterTokens?: number;
}
export interface CompactResult {
    sessionId: string;
    summary?: unknown;
    entryId?: string;
    strategy: string;
}
export interface SearchSessionsOptions {
    sessionId: string;
    query?: string;
    limit?: number;
}
export interface SessionSearchHit {
    sessionId: string;
    leafId?: string;
    updatedAt?: string;
    label?: string;
    summary?: string;
    snippet?: string;
    metadata?: Record<string, unknown>;
}
export interface SearchSessionsResult {
    hits: SessionSearchHit[];
    nextCursor?: string;
}
export interface SetFullAutonomyOptions {
    sessionId: string;
    /** Default false: gated tool calls require approval. Host-set only. */
    enabled: boolean;
}
export interface SetFullAutonomyResult {
    sessionId: string;
    fullAutonomy: boolean;
}
export interface ResumeRunOptions {
    sessionId: string;
    runId: string;
    expectedVersion: number;
    decision?: "approve" | "deny";
    decisions?: Array<Record<string, unknown>>;
}
export interface ResumeRunResult {
    sessionId: string;
    runId: string;
    status: string;
    version?: number;
    interruption?: unknown;
}
export type SessionTreeMethod = "checkout" | "fork" | "clone" | "checkpoint";
export interface SessionTreeOptions {
    sessionId: string;
    method: SessionTreeMethod;
    entryId: string;
}
export interface SessionTreeResult {
    sessionId: string;
    leafId?: string;
    [key: string]: unknown;
}
export declare function compact(options: CompactOptions): Promise<CompactResult>;
export declare function searchSessions(options: SearchSessionsOptions): Promise<SearchSessionsResult>;
export declare function setFullAutonomy(options: SetFullAutonomyOptions): Promise<SetFullAutonomyResult>;
export declare function resumeRun(options: ResumeRunOptions): Promise<ResumeRunResult>;
export declare function sessionTree(options: SessionTreeOptions): Promise<SessionTreeResult>;
export interface ProfileRegisterOptions {
    /** Profile name; sessions select it by name. Duplicate names fail closed. */
    name: string;
    description?: string;
    /** Coding system prompt layer for the profile. */
    instructions?: string;
    /** Tool names resolved against the daemon tool registry; unknown names
     *  fail closed at session start, before any provider turn. */
    tools?: string[];
    /** Skill names resolved against the daemon skill registry; register
     *  skills (agent.skillRegister) before the profile that references them. */
    skills?: string[];
}
export interface ProfileRegisterResult {
    name?: string;
    registered?: boolean;
    /** Present when the daemon was not yet running: the declaration is
     *  queued server-side and applied right after the daemon's next
     *  initialize handshake, before any session command. In runtimes with
     *  no agent host at all, the declaration queues process-global and
     *  applies when a host installs. */
    queued?: boolean;
    [key: string]: unknown;
}
export interface SkillRegisterOptions {
    name: string;
    description?: string;
    instructions?: string;
    toolNames?: string[];
}
export interface SkillRegisterResult {
    name?: string;
    registered?: boolean;
    /** See ProfileRegisterResult.queued: load entries never spawn the daemon or fail for a missing host. */
    queued?: boolean;
}
export declare function profileRegister(options: ProfileRegisterOptions): Promise<ProfileRegisterResult>;
export declare function skillRegister(options: SkillRegisterOptions): Promise<SkillRegisterResult>;
export interface CommandRegisterOptions {
    /** Daemon command name. Slash names ("/compact") are prompt-invocable. */
    name: string;
    /** Host-built-in handler (compact, newSession, checkout, forkSession,
     *  cloneSession, tree, openSession, openSessionAsFork, startRun,
     *  startWorkflow, steer). Omitted handlers are data-only. */
    handler?: string;
    description?: string;
}
export interface CommandRegisterResult {
    name?: string;
    registered?: boolean;
    /** See ProfileRegisterResult.queued: load entries never spawn the daemon or fail for a missing host. */
    queued?: boolean;
}
export interface CommandDispatchOptions {
    name: string;
    sessionId?: string;
    args?: Record<string, unknown>;
}
/** Live dispatch only: commands act on daemon session state, so an
 *  unavailable daemon is a typed failure, never a deferred execution. */
export declare function commandRegister(options: CommandRegisterOptions): Promise<CommandRegisterResult>;
export declare function commandDispatch(options: CommandDispatchOptions): Promise<Record<string, unknown>>;
export interface KnowledgeSetOptionsOptions {
    /** Absolute workspace root the knowledge base binds to. */
    workspaceRoot: string;
    /** Opt-in wiki knowledge base (decision 2156): false disables and
     *  leaves no residue (default). Absent = no change. At least one of
     *  `wiki`/`graft` is required. */
    wiki?: boolean;
    /** Opt-in graft context-graph knowledge base: false disables and
     *  leaves no residue (default). Absent = no change. */
    graft?: boolean;
    /** Graft extension mode: `pull` (CLI tools + commands + skill, default),
     *  `push` (retrieval pack + first-turn orientation + edit blast radius),
     *  or `both`. */
    graftMode?: "pull" | "push" | "both";
    /** Explicit path to a `graft` CLI entry (host-owned). Without a resolvable
     *  CLI (path, package root, or @nanonets/graft peer) graft stays off. */
    graftCliPath?: string;
}
export interface KnowledgeSetOptionsResult {
    workspaceRoot: string;
    wiki?: boolean;
    /** Present when `graft` was requested: true when bound, false when the
     *  CLI did not resolve (fail closed — tools hidden, nothing loaded). */
    graft?: boolean;
    queued?: boolean;
}
/** Per-workspace knowledge options, forwarded to the daemon (queued while
 *  the daemon is down so load entries never block on its boot). */
export declare function knowledgeSetOptions(options: KnowledgeSetOptionsOptions): Promise<KnowledgeSetOptionsResult>;
export interface SetRunOptionsOptions {
    /** Prism policy cap. Positive safe integer, or `null` to disable. Default `null`. */
    maxInputTokens?: number | null;
    /** Prism policy cap. Positive safe integer, or `null` to disable. Default `null`. */
    maxOutputTokens?: number | null;
    /** Fork-bomb fence. Positive safe integer, or `null` to disable. Default 64. */
    maxTurns?: number | null;
    /** Fork-bomb fence. Positive safe integer, or `null` to disable. Default 64. */
    maxToolRounds?: number | null;
    /** Fork-bomb fence. Positive safe integer, or `null` to disable. Default 256. */
    maxToolCalls?: number | null;
    /** Wall-clock fence in ms. Positive safe integer, or `null` to disable. Default 1_800_000 (30 min). */
    maxWallTimeMs?: number | null;
    /** Auto-compact trigger in tokens. Default 800_000. Stored; unused until
     *  auto-compact is wired. Distinct from agent.compact compactAfterTokens
     *  (OM threshold, decision 2158 default 80000). */
    compactAfterTokens?: number;
    /** Default compaction when agent.compact / /compact omit strategy.
     *  `default` (local), `llm` (provider summary), `om`. Default `llm`. */
    compaction?: "default" | "llm" | "om";
}
export interface SetRunOptionsResult {
    maxInputTokens: number | null;
    maxOutputTokens: number | null;
    maxTurns: number | null;
    maxToolRounds: number | null;
    maxToolCalls: number | null;
    maxWallTimeMs: number | null;
    compactAfterTokens: number;
    compaction: "default" | "llm" | "om";
    queued?: boolean;
}
/** Coding-run policy caps and default compact strategy. Queued while the
 *  daemon is down so init.js / package config never block on boot. */
export declare function setRunOptions(options: SetRunOptionsOptions): Promise<SetRunOptionsResult>;
