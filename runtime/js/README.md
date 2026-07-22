# Clay JavaScript Facades

`runtime/js/` owns Clay's executable JavaScript facade modules and adjacent TypeScript declarations. Each `*.js` file is the only executable body for its `clay:*` module; matching `*.d.ts` files describe public options/results without duplicating implementation.

`src/server/facades.rs` includes these checked-in JavaScript files at compile time and classifies every module as trusted-only or public to the shared third-party runtime. `ClayModuleLoader` resolves only rows admitted by that table. There is no embedded raw-string copy in `src/server/js_runtime.rs` and no runtime filesystem read or transpilation.

`runtime/js/mod.ts` is an aggregate declaration/source-tree entry point for tooling. User and package code imports domain specifiers, not this file:

- trusted-only: `clay:configuration`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:packages`, `clay:application`, `clay:editor`, `clay:theme`;
- public to both domains: `clay:sdui`, `clay:ui`, `clay:git`, `clay:behavior`, `clay:language-server`, `clay:modes`, `clay:commands`, `clay:decorations`, `clay:diagnostics`, `clay:parse`, `clay:syntax`, `clay:completion`, `clay:language`.

Facade implementations may call Clay-owned `Deno.core.ops` internally. Public exports must never expose raw op names, V8 values, native handles, or client-side JavaScript authority. Third-party security depends on privileged ops and trusted-only facades being absent from that runtime, not on hiding names in JavaScript.

Validation:

```bash
cargo test --test protocol clay_js_facade_layout::
cargo test --test security rust_visibility_api_mapping::third_party_facade_allowlist_exactly_matches_plan_public_inventory
cargo test js_runtime --lib
```
