# `examples/config/` — canonical starter configuration

Everything in this folder is what a new user copies into their
configuration root to get started:

```sh
cp -r examples/config/. ~/.config/clay/
```

Layout (mirrors `~/.config/clay/` on a user machine):

| Path                  | Destination                  | Purpose |
|-----------------------|------------------------------|---------|
| `init.js`             | `~/.config/clay/init.js`     | Base Clay config — every user-facing configuration surface, fully functional standalone. Ends with fault-isolated loads of the two package modules below. |
| `packages/first-party.js` | `~/.config/clay/packages/first-party.js` | Language-server grants + one-line `loadPackage` calls for bundled `@clay/*` packages (including the Coding Agent). |
| `packages/third-party.js` | `~/.config/clay/packages/third-party.js` | Commented template for third-party packages (none ship). |
| `agent/tool-caps.json` | `~/.config/clay/agent/tool-caps.json` | Coding-agent repository scan caps (`repo_list` / `repo_search` / `glob`). |

## `agent/tool-caps.json`

The coding agent's repository tools walk the workspace under caps so one
search cannot stall a run on a huge tree. The shipped file matches the
defaults; edit it to raise a cap when the tree is genuinely large:

- `maxEntries` — entries scanned (hard ceiling 100000)
- `maxFiles` — files scanned (hard ceiling 100000)
- `maxDepth` — directory depth (hard ceiling 128)
- `maxResults` — results returned (hard ceiling 10000)
- `maxMatches` — search matches (hard ceiling 10000)
- `maxScanBytes` — bytes of file content scanned (hard ceiling 1073741824)
- `exclude` — directory names skipped by the walk (never scanned)

When a tool hits a cap, Clay returns an error naming the cap, this file,
and the hard ceiling — raise the key and restart Clay. Prefer scoping
searches with the tool's `path` argument over raising caps; excluding
build trees (`target/`, `dist/`, `node_modules/`) is almost always the
right fix.
