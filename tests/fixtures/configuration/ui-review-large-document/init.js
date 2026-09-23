// Plan 126 access-path fixture: the same provider and completion binding as
// `ui-review-completion`, but the harness opens the generated ≥4 MiB `review.rs`
// instead of the small `review.rs` the completion fixture uses.
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
