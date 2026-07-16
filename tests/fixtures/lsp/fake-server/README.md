# Generic Clay LSP fake server

One standards-shaped LSP 3.17 fixture used by Phase 18.21 bridge tests.

## Layout

- `profiles.mjs` — capability/response profiles (`rust`, `typescript`, `javascript`, `markdown`, `minimal`, `hung`, `exit-early`, `malformed`, `oversize`)
- `session.mjs` — in-process `FakeLspSession` for package bridge tests
- `server.mjs` — spawnable stdio child for `LanguageServerProcessService` tests
- `fake-server.test.mjs` — fixture/protocol coverage
- `matrix.test.mjs` — drives all four `@clay/lsp-*` bridges through the shared session

## Commands

```bash
node --test tests/fixtures/lsp/fake-server/fake-server.test.mjs tests/fixtures/lsp/fake-server/matrix.test.mjs
cargo test --test lsp_bridge
cargo test --test language_server_authority
CLAY_LSP_REAL_SMOKE=1 cargo test --test lsp_real_servers -- --nocapture
```

Ordinary `cargo test` must stay green without host language servers. Real
smoke remains opt-in via `CLAY_LSP_REAL_SMOKE=1`.
