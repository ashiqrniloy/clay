// @clay/chat — first-party Chat landing (Phase 25).
//
// Inert catalog-composed empty-tab surface. Re-exports the load entry so
// `loadPackage("@clay/chat")` invokes `loadChatPackage`.
export {
  CHAT_PROFILE,
  chatPackageContract,
  loadChatPackage
} from "./load.js";
