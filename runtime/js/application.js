// Clay application facade skeleton.
//
// Application lifecycle APIs are planned user-facing Clay JS facades. They do
// not call raw ops directly; future runtime wiring will route through explicit
// Clay op wrappers after permission and lifecycle validation exists.
function plannedApi(name) {
    throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}
export async function quit(options = {}) {
    void options;
    plannedApi("application.quit");
}
