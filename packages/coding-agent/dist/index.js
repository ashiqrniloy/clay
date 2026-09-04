// @clay/coding-agent — first-party Coding Agent surface (Phase 2).
//
// Re-exports the load entry so `loadPackage("@clay/coding-agent")` invokes
// `loadCodingAgentPackage`. The UI split surface lands with its own task;
// this module is the profile/skill/command contract.
export {
  CODING_PROFILE,
  CODING_SYSTEM_PROMPT,
  CODING_TOOLS,
  CREATE_PLAN_SKILL,
  SLASH_COMMANDS,
  codingAgentPackageContract,
  loadCodingAgentPackage
} from "./load.js";
