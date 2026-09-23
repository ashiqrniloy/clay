# Plan 117 launch gate (2026-09-10)

- Real `clay server` + `clay-desktop` on dedicated endpoint
  `/run/user/1000/clay-launch.sock`, configuration root isolated to
  `/tmp/clay-launch-home/.config/clay` (canonical `examples/config/` copied
  verbatim + sample MCP fixture server + scratch workspace skill). Never
  reads the ambient Clay configuration.
- `window.png`: window-cropped capture of the healthy connected state
  (status bar `Workspace · Connected`, zero wire errors in the launch log
  after the frontend dist + desktop binary rebuild).
- Server-side daemon evidence recorded in the module 17 plan 117 execution
  record: scratch per-agent config root seeded + read in isolation,
  scratch skills.json parsed, MCP fixture connected
  (`{serverId: "fixture", connected: true, tools: 2}`), skills discovered
  from the workspace + home roots.
- Interactive GUI legs remain the documented host ceiling (no
  input-synthesis path); card rendering pinned by the frontend suite.
