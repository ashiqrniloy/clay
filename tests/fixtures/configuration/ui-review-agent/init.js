// Plan 118 review fixture: agent surface opened through its profile command.
// Registers the coding profile, enables wiki/graft knowledge bases
// (extension strip truth), and auto-opens the coding-agent surface
// through the Command Centre command catalogue.

import { loadPackage } from "clay:packages";
import { profileRegister, commandDispatch } from "clay:agent";
import { knowledgeSetOptions } from "clay:agent";

// Plan 118: the landing is the launcher package, so the agent surface needs the
// agent package loaded before the profile command can activate its pane.
await loadPackage("@clay/coding-agent");

await profileRegister({
  name: "review",
  description: "Review profile",
  instructions: "You are the review assistant.",
});

await knowledgeSetOptions({
  workspaceRoot: "/tmp/clay-agent-review",
  wiki: true,
  graft: true,
  graftMode: "pull",
});

try {
  await commandDispatch({ name: "coding-agent.profile" });
} catch {
  // Live dispatch only: if the daemon was not yet ready the shell entry
  // point remains; the review proceeds with the surface opener visible.
}
