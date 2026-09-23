// Clay agent host facade.
//
// User-facing host controls for the coding agent daemon: compaction,
// autonomy, session search, run approval resume, session-tree navigation,
// and first-party profile/skill registration. Server-first and off the
// editor hot path; these APIs forward to the daemon's validated RPC surface
// and grant no filesystem, network, or shell authority by existing. Search
// hits are metadata only and are never auto-injected into agent context.
function agentOps() {
    const ops = globalThis.Deno?.core?.ops;
    if (typeof ops?.op_clay_agent_compact_session !== "function" ||
        typeof ops?.op_clay_agent_search_sessions !== "function" ||
        typeof ops?.op_clay_agent_set_autonomy !== "function" ||
        typeof ops?.op_clay_agent_resume_run !== "function" ||
        typeof ops?.op_clay_agent_session_tree !== "function" ||
        typeof ops?.op_clay_agent_profile_register !== "function" ||
        typeof ops?.op_clay_agent_skill_register !== "function" ||
        typeof ops?.op_clay_agent_command_register !== "function" ||
        typeof ops?.op_clay_agent_command_dispatch !== "function" ||
        typeof ops?.op_clay_agent_knowledge_set_options !== "function" ||
        typeof ops?.op_clay_agent_run_set_options !== "function") {
        throw new Error("clay:agent runtime ops are unavailable in this environment");
    }
    return ops;
}
export async function compact(options) {
    return JSON.parse(await agentOps().op_clay_agent_compact_session(JSON.stringify(options)));
}
export async function searchSessions(options) {
    return JSON.parse(await agentOps().op_clay_agent_search_sessions(JSON.stringify(options)));
}
export async function setFullAutonomy(options) {
    return JSON.parse(await agentOps().op_clay_agent_set_autonomy(JSON.stringify(options)));
}
export async function resumeRun(options) {
    return JSON.parse(await agentOps().op_clay_agent_resume_run(JSON.stringify(options)));
}
export async function sessionTree(options) {
    return JSON.parse(await agentOps().op_clay_agent_session_tree(JSON.stringify(options)));
}
export async function profileRegister(options) {
    return JSON.parse(await agentOps().op_clay_agent_profile_register(JSON.stringify(options)));
}
export async function skillRegister(options) {
    return JSON.parse(await agentOps().op_clay_agent_skill_register(JSON.stringify(options)));
}
export async function commandRegister(options) {
    return JSON.parse(await agentOps().op_clay_agent_command_register(JSON.stringify(options)));
}
export async function commandDispatch(options) {
    return JSON.parse(await agentOps().op_clay_agent_command_dispatch(JSON.stringify(options)));
}
export async function knowledgeSetOptions(options) {
    return JSON.parse(await agentOps().op_clay_agent_knowledge_set_options(JSON.stringify(options)));
}
export async function setRunOptions(options) {
    return JSON.parse(await agentOps().op_clay_agent_run_set_options(JSON.stringify(options)));
}
