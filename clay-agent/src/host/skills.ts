/**
 * Skills + prompt-layer assembly for ClayAgentHost (plan 130 task 2):
 * disk-backed skill seeding/discovery/registration and the user/workspace
 * system-prompt layers. Pure helpers take explicit args; host-coupled
 * functions take the host instance (facade pattern — state stays on the
 * host, logic lives here).
 */
import { access, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { readFileSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { join, sep } from "node:path";
import { parseSkillFile, resolveActiveSkills, type AgentDefinition, type Skill, type ToolDefinition } from "@arnilo/prism";
import { discoverContributions } from "@arnilo/prism/node/contribution-discovery";
import { wikiSearcherSkill, wikiMaintainerSkill } from "@arnilo/prism-memory/wiki";
import type { ClayAgentHost } from "../host.js";
import { MAX_COMPLETION_COMMANDS, MAX_COMPLETION_DESCRIPTION_CHARS, MAX_COMPLETION_NAME_CHARS, optString, reqString, type AgentRootConfig } from "./internals.js";

/** Host-side graft skill copy (the package's own skill body is not part of
 *  its export map). Registered while the graft option is bound; the
 *  skillList catalog filters it out when the option is off. */
export const graftSkill = {
  name: "graft",
  description:
    "Use the graft context graph to locate code by architecture, callers, and coupling before spelunking.",
  instructions: [
    "Query the repo's graft context graph instead of grepping blindly.",
    "- `graft_map`: orient first — dir clusters, hubs, hotspots (no API key).",
    "- `graft_ask` (question, --source for code spans) and `graft_grep` (literal identifier): find where code lives and how it works.",
    "- `graft_callers` (exact symbol) gives exact call edges; `--depth N` walks transitively for blast radius.",
    "- `graft_skeleton <file>` skims a file's API surface ~10x cheaper than reading it.",
    "- `graft_blast <path>` after an edit lists dependents worth re-checking.",
    "Graph output is agent-aid, not authority: verify against the source at the cited file:line before editing. Run `graft build` (or /graft-build) after big changes to refresh the graph.",
    "- `/graft-init` scaffolds the graph in a repo that has none: non-interactive (--yes), never user-level state (--no-global), MCP/hook/statusline wiring off.",
    "- `/graft-build-deep` runs graft's own LLM deep pass. It refuses to spawn unless the host configured a deep model (deepModel / GRAFT_PROVIDER+GRAFT_MODEL+GRAFT_API_KEY) — do not invent one.",
  ].join("\n"),
};

/** Agent-delivered skills are file-backed (plan 117): the in-code objects
 *  above are generators — `<agentConfigRoot>/skills/<name>/SKILL.md` is the
 *  delivered payload. Seeded when absent (deleted ⇒ regenerated at every
 *  start), parsed at every start (user edits apply on next daemon start),
 *  and falling back to the built-in content on malformed/oversized files —
 *  never a broken skill. `name` and `toolNames` stay daemon-owned: the file
 *  cannot rename a skill, grant tools, or break the fail-closed toolName
 *  check. Disk discovery skips these reserved names, so seeding + gated
 *  activation is the only delivery path. */
const MAX_AGENT_SKILL_FILE_BYTES = 256 * 1024;

/** Render a built-in skill as its seeded SKILL.md (frontmatter + body). */
function skillToSeedMarkdown(skill: { name: string; description?: string; instructions?: string }): string {
  return `---\nname: ${skill.name}\ndescription: ${skill.description ?? ""}\n---\n\n${skill.instructions ?? ""}\n`;
}

/** Seed provenance stamp (plan 117): after writing a seed file, record its
 *  size + mtime in `<agentConfigRoot>/.seed-manifest.json` so the Rust
 *  settings listing can badge built-in-vs-edited without knowing the built-in
 *  content. A missing/stale stamp marks the file edited — honest fallback. */
async function recordSeedStamp(agentConfigRoot: string, relName: string): Promise<void> {
  const manifestFile = join(agentConfigRoot, ".seed-manifest.json");
  let manifest: Record<string, { sizeBytes: number; mtimeMs: number }> = {};
  try {
    manifest = JSON.parse(await readFile(manifestFile, "utf8")) as typeof manifest;
  } catch {
    // Absent or malformed: start a fresh manifest (seeds are being rebuilt).
  }
  try {
    const stats = await stat(join(agentConfigRoot, relName));
    // Truncate, not round: the Rust listing derives the file's mtime with
    // `as_millis()` (also truncating), so a rounded stamp is off by up to
    // +1 ms and every untouched seed badges as "edited".
    manifest[relName] = { sizeBytes: stats.size, mtimeMs: Math.trunc(stats.mtimeMs) };
    await writeFile(manifestFile, JSON.stringify(manifest, null, 2), "utf8");
  } catch {
    // Unstamped seed: the settings page shows it as edited — acceptable.
  }
}

/** Seed absent agent-delivered SKILL.md files; present files stay untouched
 *  (the file is the source of truth — user edits are preserved). */
async function seedAgentSkillFiles(agentConfigRoot: string): Promise<void> {
  const skillsDir = join(agentConfigRoot, "skills");
  for (const { dir, skill } of AGENT_DELIVERED_SKILLS) {
    const file = join(skillsDir, dir, "SKILL.md");
    try {
      await access(file);
      continue;
    } catch {
      // Absent: seed it (first launch or user deletion).
    }
    await mkdir(join(skillsDir, dir), { recursive: true });
    await writeFile(file, skillToSeedMarkdown(skill), "utf8");
    await recordSeedStamp(agentConfigRoot, `skills/${dir}/SKILL.md`);
  }
}

/** Load the delivered skill content from disk. Every registration site
 *  (wiki enable, graft bind) registers from this map, never the in-code
 *  objects directly. Seeding runs first, so a deleted file is delivered
 *  from its fresh seed in the same start. */
export async function loadAgentSkillFiles(agentConfigRoot: string): Promise<ReadonlyMap<string, Skill>> {
  const skillsDir = join(agentConfigRoot, "skills");
  try {
    await seedAgentSkillFiles(agentConfigRoot);
  } catch (error) {
    process.stderr.write(
      `[skills] seeding agent skill files failed: ${error instanceof Error ? error.message : String(error)}\n`,
    );
  }
  const files = new Map<string, Skill>();
  for (const { dir, skill } of AGENT_DELIVERED_SKILLS) {
    const file = join(skillsDir, dir, "SKILL.md");
    let text: string;
    try {
      text = await readFile(file, "utf8");
    } catch {
      // Seeding failed (unwritable config root) — deliver the built-in content.
      files.set(dir, skill);
      continue;
    }
    try {
      if (Buffer.byteLength(text, "utf8") > MAX_AGENT_SKILL_FILE_BYTES) {
        throw new Error(`file exceeds ${MAX_AGENT_SKILL_FILE_BYTES} bytes`);
      }
      const parsed = parseSkillFile(text, file);
      // Name and toolNames are daemon-owned; the file carries description + body.
      files.set(dir, { ...parsed, name: dir, toolNames: skill.toolNames });
    } catch (error) {
      process.stderr.write(
        `[skills] ${file} unusable (${error instanceof Error ? error.message : String(error)}); using built-in seed content\n`,
      );
      files.set(dir, skill);
    }
  }
  return files;
}

/** User-owned global system-prompt layer (plan 117): seeded EMPTY at host
 *  creation (a default body would pollute every session's prompt — the file
 *  exists so the user has a stable home for global instructions), read per
 *  session build (sync, bounded — edits apply on the next session), and
 *  injected as a `user`-source system-prompt contribution so Prism's source
 *  rank orders it right after the profile base instructions and before the
 *  workspace AGENTS.md app layer. Byte-stable per session, so it rides the
 *  cached prefix. Oversized/unreadable ⇒ skipped with a warning, never a
 *  broken session. */
const MAX_PROMPT_LAYER_BYTES = 64 * 1024;
const USER_SYSTEM_PROMPT_FILE = "SYSTEM.md";
const WORKSPACE_AGENTS_FILE = "AGENTS.md";

export async function seedUserSystemPrompt(agentConfigRoot: string): Promise<void> {
  const file = join(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
  try {
    await access(file);
    return;
  } catch {
    // Absent: seed it (first launch or user deletion).
  }
  await writeFile(file, "", "utf8");
  await recordSeedStamp(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
}

/** Shared bounded prompt-layer reader (plan 117): trimmed text, "" when
 *  absent, throws on unreadable/oversized so each caller applies its own
 *  trust policy (user-owned SYSTEM.md warns; repo AGENTS.md skips). */
function readPromptLayer(file: string, maxBytes: number): string {
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return "";
    throw error;
  }
  if (Buffer.byteLength(text, "utf8") > maxBytes) {
    throw new Error(`file exceeds ${maxBytes} bytes`);
  }
  return text.trim();
}

export function loadUserSystemPrompt(agentConfigRoot: string): string {
  const file = join(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
  try {
    return readPromptLayer(file, MAX_PROMPT_LAYER_BYTES);
  } catch (error) {
    process.stderr.write(
      `[system] ${file} unusable (${error instanceof Error ? error.message : String(error)}); skipping user layer\n`,
    );
    return "";
  }
}

/** Workspace `AGENTS.md` app layer (plan 117): repo content is untrusted
 *  prompt text — bounded bytes, symlink-escape excluded (the file's
 *  realpath must stay inside the workspace root), and a silent skip when
 *  absent (most repos have none; ENOENT is not a warning). Byte-stable per
 *  session; the `app` source rank composes it after the user SYSTEM.md
 *  layer. */
export function loadWorkspaceAgentsPrompt(workspaceRoot: string): string {
  const file = join(workspaceRoot, WORKSPACE_AGENTS_FILE);
  try {
    const realFile = realpathSync(file);
    const realRoot = realpathSync(workspaceRoot);
    if (!realFile.startsWith(realRoot + sep)) return "";
    return readPromptLayer(realFile, MAX_PROMPT_LAYER_BYTES);
  } catch {
    return ""; // absent, unreadable, oversized, or escaped — all skip
  }
}

/** Skills discovered from disk never activate these names: they are
 *  agent-delivered and gated (wiki behind `/wiki-init`, graft behind
 *  binding). They seed into the agent skills dir (plan 117) but register
 *  only through their own activation paths. */
const RESERVED_AGENT_SKILL_NAMES: ReadonlySet<string> = new Set([
  "graft",
  "wiki-searcher",
  "wiki-maintainer",
]);

/** Locked cap: at most 64 skills scanned per discovery root. */
const MAX_SKILLS_PER_ROOT = 64;

export interface SkillsConfig {
  workspaceEnabled: boolean;
  configRootEnabled: boolean;
  homeEnabled: boolean;
  homePath: string;
  agentSkills: { wikiSearcher: boolean; wikiMaintainer: boolean; graft: boolean };
}

const DEFAULT_SKILLS_CONFIG: SkillsConfig = {
  workspaceEnabled: true,
  configRootEnabled: true,
  homeEnabled: true,
  homePath: "",
  agentSkills: { wikiSearcher: true, wikiMaintainer: true, graft: true },
};

function skillsWarn(message: string): void {
  process.stderr.write(`[skills] ${message}\n`);
}

/** `~`/`~/` expansion; null when the path is relative (rejected
 * fail-closed per the locked skills.json schema). */
function expandSkillsPath(value: string): string | null {
  if (value === "~") return homedir();
  if (value.startsWith("~/")) return join(homedir(), value.slice(2));
  if (value.startsWith("/")) return value;
  return null;
}

function skillsFlag(record: Record<string, unknown>, key: string, fallback: boolean): boolean {
  const value = record[key];
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") {
    skillsWarn(`ignoring non-boolean "${key}": ${JSON.stringify(value)}`);
    return fallback;
  }
  return value;
}

/** Load skills.json from the coding-agent config root. Absent/unreadable
 *  file = all roots on with defaults (stderr note, per the locked schema);
 *  unknown keys warned (tool-caps.json precedent); relative paths rejected
 *  fail-closed (root disabled, never a broken boot). */
export async function loadSkillsConfig(
  agentConfigRoot: string,
  defaultHomePath: string,
): Promise<{ config: SkillsConfig; file: string }> {
  const file = join(agentConfigRoot, "skills.json");
  let raw: unknown;
  try {
    raw = JSON.parse(await readFile(file, "utf8"));
  } catch {
    skillsWarn(`${file} absent or unreadable; using defaults (all roots enabled)`);
    return {
      config: { ...DEFAULT_SKILLS_CONFIG, homePath: defaultHomePath, agentSkills: { ...DEFAULT_SKILLS_CONFIG.agentSkills } },
      file,
    };
  }
  const config: SkillsConfig = {
    ...DEFAULT_SKILLS_CONFIG,
    homePath: defaultHomePath,
    agentSkills: { ...DEFAULT_SKILLS_CONFIG.agentSkills },
  };
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    skillsWarn(`ignoring malformed ${file}: top level must be an object`);
    return { config, file };
  }
  const root = raw as Record<string, unknown>;
  for (const key of Object.keys(root)) {
    if (key !== "roots" && key !== "agentSkills") skillsWarn(`ignoring unknown key "${key}"`);
  }
  const roots = typeof root.roots === "object" && root.roots !== null && !Array.isArray(root.roots)
    ? (root.roots as Record<string, unknown>)
    : {};
  const workspace = typeof roots.workspace === "object" && roots.workspace !== null ? (roots.workspace as Record<string, unknown>) : {};
  const configRootRecord = typeof roots.configRoot === "object" && roots.configRoot !== null ? (roots.configRoot as Record<string, unknown>) : {};
  const home = typeof roots.home === "object" && roots.home !== null ? (roots.home as Record<string, unknown>) : {};
  config.workspaceEnabled = skillsFlag(workspace, "enabled", true);
  config.configRootEnabled = skillsFlag(configRootRecord, "enabled", true);
  config.homeEnabled = skillsFlag(home, "enabled", true);
  const homePathRaw = typeof home.path === "string" && home.path.length > 0 ? home.path : defaultHomePath;
  const expanded = expandSkillsPath(homePathRaw);
  if (expanded === null) {
    skillsWarn(`ignoring relative roots.home.path ${JSON.stringify(homePathRaw)}; home root disabled`);
    config.homeEnabled = false;
  } else {
    config.homePath = expanded;
  }
  const agentSkills = typeof root.agentSkills === "object" && root.agentSkills !== null && !Array.isArray(root.agentSkills)
    ? (root.agentSkills as Record<string, unknown>)
    : {};
  config.agentSkills.wikiSearcher = skillsFlag(agentSkills, "wikiSearcher", true);
  config.agentSkills.wikiMaintainer = skillsFlag(agentSkills, "wikiMaintainer", true);
  config.agentSkills.graft = skillsFlag(agentSkills, "graft", true);
  return { config, file };
}

const AGENT_DELIVERED_SKILLS: readonly { dir: string; skill: Skill }[] = [
  { dir: "wiki-searcher", skill: wikiSearcherSkill },
  { dir: "wiki-maintainer", skill: wikiMaintainerSkill },
  { dir: "graft", skill: graftSkill },
];

/** Resolve a profile's skill names against the kernel registry, validating
 *  each skill's required tools are active (fail-closed before any turn).
 *  Wiki skills append for coding sessions in the enabled workspace — their
 *  toolNames require the wiki tools, which are active exactly then. The
 *  graft skill appends on the same workspace binding. */
export function resolveSkills(
  host: ClayAgentHost,
  def: AgentDefinition,
  tools: readonly ToolDefinition[],
  options?: { workspaceRoot?: string; agent?: AgentRootConfig },
): readonly Skill[] | undefined {
  const names = def.skills ? [...def.skills] : [];
  if (options?.workspaceRoot !== undefined && host.wiki?.workspaceRoot === options.workspaceRoot) {
    names.push(wikiSearcherSkill.name, wikiMaintainerSkill.name);
  }
  if (options?.workspaceRoot !== undefined && host.graft?.workspaceRoot === options.workspaceRoot) {
    names.push(graftSkill.name);
  }
  // Disk-discovered skills ride along when every declared toolName is
  // active for this session. Profile-declared names keep fail-closed
  // resolution; discovered ones are skipped instead of thrown so one bad
  // SKILL.md cannot brick session start.
  if (options?.workspaceRoot !== undefined) {
    const activeToolNames = new Set(tools.map((tool) => tool.name));
    // Plan 118 task 35: the config-root names come from the session's own
    // agent (the home and workspace roots are host/workspace scoped).
    const discovered = [
      ...(options.agent?.configSkillNames ?? host.configSkillNames ?? []),
      ...(host.homeSkillNames ?? []),
      ...(host.workspaceSkillNames.get(options.workspaceRoot) ?? []),
    ];
    for (const name of discovered) {
      if (names.includes(name)) continue;
      const skill = host.skills.get(name);
      if (skill?.toolNames && !skill.toolNames.every((toolName) => activeToolNames.has(toolName))) continue;
      names.push(name);
    }
  }
  if (names.length === 0) return undefined;
  return resolveActiveSkills({ registry: host.skills, names, tools });
}

/** Config-gated, per-root skill discovery. Config-root + home roots scan
 *  `<root>/skills/<name>/SKILL.md` (decision 2026-09-09-1420); the
 *  workspace keeps npx skills `<root>/.agents/skills/`. Each root is
 *  scanned once (cached). Already-registered names win on collision
 *  (registry is duplicate:"error"): built-ins and earlier roots shadow
 *  later ones. Reserved agent-delivered names never come from disk —
 *  they activate only through their own paths. A failing root logs and
 *  yields nothing — discovery never fails a session. */
export async function ensureSkillDiscovery(host: ClayAgentHost, workspaceRoot: string): Promise<void> {
  if (host.configSkillNames === undefined) {
    host.configSkillNames = host.skillsConfig.configRootEnabled
      ? await scanSkillsDir(host, join(host.agentConfigRoot, "skills"))
      : [];
  }
  if (host.homeSkillNames === undefined) {
    host.homeSkillNames = host.skillsConfig.homeEnabled
      ? await scanSkillsDir(host, join(host.skillsConfig.homePath, "skills"))
      : [];
  }
  if (!host.workspaceSkillNames.has(workspaceRoot)) {
    host.workspaceSkillNames.set(
      workspaceRoot,
      host.skillsConfig.workspaceEnabled ? await discoverWorkspaceSkills(host, workspaceRoot) : [],
    );
  }
}

/** Workspace scan via Prism's discoverContributions (symlink-escape
 *  safe, ENOENT-tolerant, npx skills layout). */
async function discoverWorkspaceSkills(host: ClayAgentHost, workspaceRoot: string): Promise<readonly string[]> {
  let discovered: readonly (Awaited<ReturnType<typeof discoverContributions>>)[number][];
  try {
    discovered = await discoverContributions({ kinds: ["skill"], workspaceRoot });
  } catch (error) {
    process.stderr.write(
      `skill discovery failed for ${workspaceRoot}: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    return [];
  }
  const names: string[] = [];
  for (const entry of discovered) {
    const skill = entry.skill;
    if (!skill || host.skills.get(skill.name)) continue;
    host.skills.register(skill);
    names.push(skill.name);
  }
  return names;
}

/** Scan `<skillsDir>/<name>/SKILL.md` (one level) and register each
 *  parseable skill. Bounded; absent dir = no skills (not a warning);
 *  a bad SKILL.md is skipped with a stderr note. */
export async function scanSkillsDir(host: ClayAgentHost, skillsDir: string): Promise<readonly string[]> {
  let entries: string[];
  try {
    entries = await readdir(skillsDir);
  } catch {
    return [];
  }
  const names: string[] = [];
  for (const entry of entries.slice(0, MAX_SKILLS_PER_ROOT)) {
    if (RESERVED_AGENT_SKILL_NAMES.has(entry)) continue;
    const file = join(skillsDir, entry, "SKILL.md");
    let text: string;
    try {
      text = await readFile(file, "utf8");
    } catch {
      continue;
    }
    try {
      const skill = parseSkillFile(text, file);
      if (host.skills.get(skill.name)) continue;
      host.skills.register(skill);
      names.push(skill.name);
    } catch (error) {
      process.stderr.write(
        `[skills] skipping ${file}: ${error instanceof Error ? error.message : String(error)}\n`,
      );
    }
  }
  return names;
}

/** Whether an agent-delivered skill is enabled by skills.json
 *  `agentSkills` (default true). Consumed by the wiki/graft activation
 *  paths (plan 117). */
export function agentSkillEnabled(
  host: ClayAgentHost,
  name: "wikiSearcher" | "wikiMaintainer" | "graft",
): boolean {
  return host.skillsConfig.agentSkills[name];
}

/** Register inert skill data on the kernel registry (re-registering a name replaces the definition). */
export function skillRegister(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const name = reqString(params, "name");
  const description = optString(params, "description");
  const instructions = optString(params, "instructions");
  const toolNames = Array.isArray(params.toolNames)
    ? params.toolNames.filter((item): item is string => typeof item === "string")
    : undefined;
  const metadata = params.metadata && typeof params.metadata === "object" && !Array.isArray(params.metadata)
    ? (params.metadata as Record<string, unknown>)
    : undefined;
  host.skills.register({
    name,
    ...(description !== undefined ? { description } : {}),
    ...(instructions !== undefined ? { instructions } : {}),
    ...(toolNames && toolNames.length > 0 ? { toolNames } : {}),
    ...(metadata !== undefined ? { metadata } : {}),
  });
  return { name, registered: true };
}

/** Progressive catalog: name + description only. Full instructions load via `load_skill`.
 *  Wiki skills appear only while the wiki option is bound (no residue when disabled). */
export function skillList(host: ClayAgentHost): unknown {
  return {
    skills: visibleSkills(host),
  };
}

/** Catalog-visible skills (wiki/graft skills hide while their option is
 *  unbound). Bounded to the environment's wire caps. */
export function visibleSkills(host: ClayAgentHost): Array<{ name: string; description: string }> {
  return host.skills
    .list()
    .filter(
      (skill) =>
        (host.wiki !== undefined || !skill.name.startsWith("wiki-")) &&
        (host.graft !== undefined || skill.name !== graftSkill.name),
    )
    .slice(0, MAX_COMPLETION_COMMANDS)
    .map((skill) => ({
      name: skill.name.slice(0, MAX_COMPLETION_NAME_CHARS),
      description: (skill.description ?? "").slice(0, MAX_COMPLETION_DESCRIPTION_CHARS),
    }));
}
