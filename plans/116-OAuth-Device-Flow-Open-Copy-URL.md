# OAuth Device Flow: Auto-Open, Clickable, and Copyable Authorization URL

- Task ID: `116-oauth-url-open-copy`
- Status: `done`

## Context

When configuring a provider (X.AI etc.) from the Command Centre, the OAuth
device-code journey shows a single row ("Device code ABCD-EFGH") with the
authorization URL as detail text. Nothing opens the URL in the default OS
browser, and there is no affordance to open the URL manually or copy it.

Server-side flow: the user picks OAuth → `menus.rs` handles
`AgentPickerActivate::StartOauth` → `host.start_oauth(provider)` returns
`AgentOauthStart { login_id, user_code, verification_uri, authorization_url }`
→ `picker.enter_oauth(login_id, user_code, uri)`. The OAuth stage renders one
item (`poll_oauth`) whose Enter activation polls for completion.

Requests (all three):
1. Automatically *try* to open the authorization URL in the default OS browser.
2. Make the URL clickable so the user can manually open it in the default browser.
3. Provide a copy-URL option as a fallback (e.g., to open in a non-default browser).

## Todos

- [x] [task-1] Add a server-side `src/server/open.rs` helper module with
      `open_url()` (platform default-browser opener) and
      `copy_to_clipboard()` (OS clipboard), plus an http/https scheme guard.
      Acceptance: compiles on Unix+Windows behind `cfg(any(unix, windows))`;
      URL scheme guard unit-tested; no shell-command string is ever built from
      the URL (fixed argv program + URL as one argument).
- [x] [task-2] Extend `AgentPickerActivate` in `src/server/agent_picker.rs`
      with `OpenOauthUrl { uri }` / `CopyOauthUrl { uri }`, map new item ids in
      `activate_oauth`, and render `open_oauth_url` / `copy_oauth_url` rows in
      the OAuth stage of `visible_items` (only when a URL exists). Poll row
      stays first. Acceptance: `oauth_labels_distinguish_device_code_from_redirect`
      updated + new assertions for the two action rows and their activation ids.
- [x] [task-3] In `src/server/connection/menus.rs`: auto-open the URI when the
      OAuth stage is entered (`StartOauth` outcome, best-effort) and emit a
      warning `RuntimeDiagnostic` when auto-open fails; handle the new
      `OpenOauthUrl` / `CopyOauthUrl` outcomes by calling the helpers and
      keeping the picker open, with info/warning diagnostics.
      Acceptance: `cargo test` green (targeted suites), clippy clean.
- [x] [task-4] Verify: run `cargo test` for `agent_picker`, `menus`,
      `transient_menu`-related suites and the root test suite subset; run
      `scripts/check.sh` (or equivalent) if cheap.

## Notes

- No protocol (`src/protocol/menu.rs`) change required: the OAuth journey stays
  inside the existing server-owned transient-menu machinery; each new action is
  just another menu row whose Enter/click activation routes back to the server.
- Windows open uses a direct `rundll32 url.dll,FileProtocolHandler <url>` spawn;
  Windows copy pipes text into the standard `clip` utility. Linux uses
  `xdg-open` / try `wl-copy` → `xclip` → `xsel`; macOS uses `/usr/bin/open` and
  `/usr/bin/pbcopy`. No shell metacharacter parsing; argv is fixed.
- Only http/https URLs are opened (blocks custom-scheme/file launches).
- Copy is server-driven so it works identically in the webview and the native
  client; failures surface as a warning diagnostic instead of failing silently.
- Decisions and evidence recorded as tasks complete.

## Completion notes (2026-09-08)

- task-1: `src/server/open.rs` — `is_safe_http_url`/`open_url`/`copy_to_clipboard`;
  fixed-argv spawns only (`/usr/bin/open`, `xdg-open`, `rundll32 url.dll,FileProtocolHandler`;
  `pbcopy`/`wl-copy`/`xclip`/`xsel`/`clip`), scheme guard unit-tested (3 tests in module).
  Registered as `mod open` in `src/server/mod.rs:47`.
- task-2: `OpenOauthUrl { uri }` / `CopyOauthUrl { uri }` variants; `activate_oauth`
  maps `open_oauth_url`/`copy_oauth_url` ids (falls back to `StayOpen` when the
  stage has no URL); OAuth stage renders the poll row first, then "Open in
  browser" and "Copy URL" rows only when a URI exists. New tests:
  `oauth_stage_offers_browser_open_and_copy_url_actions`,
  `oauth_stage_without_url_keeps_only_the_poll_row`;
  `oauth_labels_distinguish_device_code_from_redirect` still passes.
- task-3: `StartOauth` arm auto-opens once after `enter_oauth` (best-effort) and
  emits warning `agent.oauth_auto_open_failed` with a pointer to the fallback rows;
  `OpenOauthUrl`/`CopyOauthUrl` arms keep the picker open and emit info
  (`agent.oauth_open_url`, `agent.oauth_copy_url`) or error
  (`agent.oauth_open_url_failed`, `agent.oauth_copy_url_failed`) diagnostics.
- task-4: `cargo test --lib` 1289 passed / 0 failed (includes 15 `agent_picker` +
  3 `open::` unit tests); `cargo test --test protocol` 209 passed / 0 failed;
  `cargo clippy --all-targets` clean (one `clippy::redundant_locals` in the new
  test fixed); `cargo fmt --check` clean.
- Client side needs no change: the rows are ordinary transient-menu items, so
  the webview and native clients render and activate them through the existing
  machinery.