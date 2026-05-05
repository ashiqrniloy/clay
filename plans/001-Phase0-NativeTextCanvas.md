# Phase 0 Native Text Editor Module Prototype

## Objectives
- Prove the real Clay client risk: `winit`, `vello`, `parley`, `masonry`, and `crop` working together as a native text editor module.
- Replace the temporary `softbuffer` visibility compromise with the intended Vello rendering path immediately.
- Keep the prototype single-process and local, but make it structurally close to the future client canvas: native window, retained widget boundary, rope-backed text state, Parley layout, and Vello painting.

## Expected Outcome
- `cargo run` opens a native window titled for Clay Phase 0.
- The window is rendered through Vello, not `softbuffer`.
- A minimal editor surface displays text laid out with Parley from a local `crop` rope-backed model.
- Typing printable text mutates the local rope, redraws the Vello scene, and visibly updates the text; Backspace removes the previous character; Escape or window close exits.
- The prototype establishes where Masonry owns event/widget routing and where the custom text editor module owns rope, layout, and painting behavior.
- `cargo fmt`, `cargo test`, and `cargo check` pass.
- No IPC, `deno_core`, SDUI protocol, server process, extension loading, filesystem access, or network access is introduced in this phase.

## Tasks

- [x] Establish a baseline native `winit` application shell
  - Acceptance Criteria:
    - Functional: `cargo run` creates one native window and exits cleanly on CloseRequested or Escape.
    - Performance: The event loop uses wait/redraw-on-demand behavior rather than a busy polling loop.
    - Code Quality: Application state is held in a small `App` struct implementing `ApplicationHandler`, without speculative client/server abstractions.
    - Security: No filesystem, network, shell, extension, or untrusted-input execution is added.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/rust-windowing/winit`: `ApplicationHandler`, `EventLoop`, `ActiveEventLoop`, `WindowEvent`, `WindowAttributes`, keyboard input, and redraw handling for winit 0.30-style applications.
      - Local `winit` 0.30.13 crate docs: Winit does not draw window contents and warns that platforms may show garbage or unreliable results if the app does not render before the compositor displays the window.
      - Context7 `/rust-windowing/softbuffer`: `Context`, `Surface`, `resize`, `buffer_mut`, and `present` for filling a winit window with a software buffer.
    - Options Considered:
      - Use the modern `ApplicationHandler` API: matches current `winit` 0.30 design and avoids deprecated closure-loop patterns.
      - Use an older closure-based event loop: simpler in older examples but likely incompatible with current dependency expectations.
      - Use `softbuffer` temporarily: proved that the shell and manual window visibility work, but does not prove Clay's rendering stack.
    - Chosen Approach:
      - Keep the working `ApplicationHandler` shell as a baseline only. Treat `softbuffer` as a temporary compromise to be removed by the next task.
    - API Notes and Examples:
      ```rust
      use winit::application::ApplicationHandler;
      use winit::event_loop::{ActiveEventLoop, EventLoop};
      use winit::window::Window;

      impl ApplicationHandler for App {
          fn resumed(&mut self, event_loop: &ActiveEventLoop) {
              self.window = Some(event_loop.create_window(Window::default_attributes()).unwrap());
          }
      }
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Current baseline native shell.
      - `Cargo.toml`: Current dependencies.
    - References:
      - Context7 docs response for `/rust-windowing/winit` on `ApplicationHandler`, `window_event`, `CloseRequested`, `RedrawRequested`, and keyboard input.
      - User manual smoke test confirming the baseline window appears and exits.
  - Test Cases to Write:
    - Manual smoke test: Run `cargo run`, confirm the window opens, and confirm close/Escape exits the process.
    - Build check: Run `cargo check` to validate API usage against pinned dependencies.

- [x] Replace `softbuffer` with a minimal Vello-backed renderer
  - Acceptance Criteria:
    - Functional: `RedrawRequested` renders a visible background and simple Vello shape through a window-backed Vello/wgpu surface; `softbuffer` is removed from runtime code and dependencies.
    - Performance: Vello `RenderContext`, surface, device renderer, and reusable scene/render state are initialized once per window/surface lifecycle, not recreated per frame.
    - Code Quality: Rendering state is isolated in a small `VelloState` or equivalent, with explicit resize/suspend handling and no broad architecture rewrites beyond removing `softbuffer`.
    - Security: Rendering code does not load arbitrary external assets, fonts, files, scripts, or network resources.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/linebender/vello/v0_6_0`: `RenderContext`, `RenderSurface`, `Renderer`, `RendererOptions`, `Scene`, `RenderParams`, `AaConfig`, `AaSupport`, and winit integration.
      - Local/current `winit` docs: render in response to `WindowEvent::RedrawRequested`; create window/surfaces after `resumed`; drop surface resources on `suspended`.
    - Options Considered:
      - Continue with `softbuffer`: useful for mapping a visible window, but avoids the actual Clay rendering risk.
      - Jump directly to Masonry: closer to target stack but makes Vello failures harder to isolate.
      - First swap `softbuffer` for direct Vello: smallest step that proves the intended GPU renderer and removes the current compromise.
    - Chosen Approach:
      - Replace the software fill with Vello's window rendering setup, draw a background and a simple rectangle/circle, and keep the existing `winit` event loop until Vello rendering is stable.
    - API Notes and Examples:
      ```rust
      use vello::{AaConfig, AaSupport, Renderer, RendererOptions, RenderParams, Scene};
      use vello::util::RenderContext;

      let mut render_cx = RenderContext::new();
      let renderer = Renderer::new(
          &device_handle.device,
          RendererOptions { antialiasing_support: AaSupport::area_only(), ..Default::default() },
      )?;
      renderer.render_to_texture(&device, &queue, &scene, &texture, &RenderParams {
          base_color,
          width,
          height,
          antialiasing_method: AaConfig::Area,
      })?;
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Remove `softbuffer` state and add Vello render state/redraw path.
      - `Cargo.toml`: Removed `softbuffer` and added direct `pollster` usage for one-time async Vello/wgpu surface initialization.
      - `Cargo.lock`: Updated dependency graph after dependency changes.
    - References:
      - Context7 docs response `ctx7:docs:13c5f871605fcadb4158651b` for Vello 0.6 winit integration.
  - Test Cases to Write:
    - Manual smoke test: Run `cargo run`, confirm a Vello-rendered non-blank scene appears and resizes correctly.
    - Build check: Run `cargo check` after removing `softbuffer`.

- [x] Replace the temporary `String` model with a `crop` rope-backed editor model
  - Acceptance Criteria:
    - Functional: The editor model supports appending text, deleting the previous Unicode scalar value, and exposing visible text for layout from a `crop` rope.
    - Performance: Append/delete operations avoid full-buffer cloning; converting to `String` is limited to the visible/layout slice needed for the prototype.
    - Code Quality: Rope editing is isolated behind a small `EditorBuffer` API so future server synchronization can replace the local owner without touching rendering code heavily.
    - Security: Input remains inert text; no command, script, path, or markup interpretation is introduced.
  - Approach:
    - Documentation Reviewed:
      - Project `concept.md`: canonical state is a `crop` rope; the client eventually keeps a lightweight visible shadow rope for optimistic local typing.
      - `crop` crate examples/docs lookup was attempted; public docs were not available through Context7 due name ambiguity.
      - Local `crop` 0.4.3 crate source/docs: `Rope::new`/`Default`, `Rope::byte_len`, `Rope::insert`, `Rope::delete`, `Rope::chars`, and `Display`/`ToString` for visible text extraction.
    - Options Considered:
      - Keep `String`: easy but no longer proves the text-editor-module risk the prototype should target.
      - Use `crop` now: introduces the intended rope semantics early and exposes integration issues before IPC/server work.
    - Chosen Approach:
      - Introduce `EditorBuffer` backed by `crop`, keep operations minimal, and retain unit tests for append/backspace behavior.
    - API Notes and Examples:
      ```rust
      struct EditorBuffer {
          rope: crop::Rope,
      }

      impl EditorBuffer {
          fn insert_str(&mut self, text: &str) {
              self.rope.insert(self.rope.byte_len(), text);
          }

          fn backspace(&mut self) {
              let Some(last_char) = self.rope.chars().next_back() else { return; };
              let end = self.rope.byte_len();
              self.rope.delete(end - last_char.len_utf8()..end);
          }
      }
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Replace `TextBuffer`/`String` internals with `EditorBuffer` backed by `crop`.
    - References:
      - `concept.md` section 3 and section 4 on `crop` and client shadow text state.
      - Local `crop` 0.4.3 source at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/crop-0.4.3/src/rope/rope.rs`.
  - Test Cases to Write:
    - `editor_buffer_appends_input`: Validates inserted strings are visible in order.
    - `editor_buffer_backspace_removes_last_scalar`: Validates Backspace removes one Unicode scalar value and does not panic when empty.

- [x] Render rope text with Parley into the Vello scene
  - Acceptance Criteria:
    - Functional: The current `crop`-backed buffer is displayed as text in the Vello-rendered window, with an initial placeholder prompt when empty.
    - Performance: `FontContext` and `LayoutContext` are reused across redraws; layout is recalculated only from the visible text slice and current width.
    - Code Quality: Text layout/painter code is localized and does not attempt cursor hit-testing, selection, complex wrapping, or IME composition in this step.
    - Security: Text is rendered as glyphs only; input text is not parsed as markup, paths, scripts, or commands.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/linebender/parley`: `FontContext`, `LayoutContext`, ranged builders, default `StyleProperty`, `break_all_lines`, and `align`.
      - Context7 `/linebender/vello/v0_6_0`: `Scene` drawing and render submission APIs.
    - Options Considered:
      - Draw placeholder Vello shapes only: proves renderer but not text editor stack.
      - Use Parley immediately: proves the intended layout engine and makes typed text visibility meaningful.
    - Chosen Approach:
      - Build a plain Parley layout from the editor buffer's visible text, break lines to the window width, and draw glyphs into the Vello scene using the supported Vello/Parley integration discovered during implementation.
    - API Notes and Examples:
      ```rust
      use parley::{FontContext, LayoutContext};
      use parley::style::StyleProperty;

      let mut font_cx = FontContext::new();
      let mut layout_cx: LayoutContext<()> = LayoutContext::new();
      let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
      builder.push_default(StyleProperty::FontSize(16.0));
      let mut layout = builder.build(text);
      layout.break_all_lines(Some(width));
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Added reusable `TextLayoutState` with Parley `FontContext`/`LayoutContext`, builds a plain layout from the rope-visible text or placeholder, and emits glyph runs into the Vello `Scene` with `Scene::draw_glyphs`.
    - References:
      - Context7 docs response `ctx7:docs:7baddd99b9f5002a3777ec2d` for Parley simple text layout.
      - Context7 docs responses `ctx7:docs:1ff95dc8b933806d06305aea` and `ctx7:docs:6ee72b0f34aa5c4a9037d595` for Parley glyph-run iteration and Vello `Scene::draw_glyphs` usage.
  - Test Cases to Write:
    - Manual smoke test: Empty buffer shows placeholder text; non-empty initial/test text is visible.
    - Build check: `cargo check` passed after adding Parley/Vello glyph rendering.
    - Regression check: `cargo fmt` and `cargo test` passed after adding Parley/Vello glyph rendering.

- [ ] Wire keyboard editing to the rope and visible renderer
  - Acceptance Criteria:
    - Functional: Printable text input appends to the `crop` buffer; Backspace deletes; Escape exits; each edit requests a redraw and the text visibly updates.
    - Performance: Input handling performs no blocking work and no renderer/layout reinitialization per key event.
    - Code Quality: Keyboard handling is explicit and small, with named-key handling separated from printable text handling where practical.
    - Security: Control sequences and shortcuts are not interpreted as commands; only plain text insertion/deletion is supported.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/rust-windowing/winit`: `WindowEvent::KeyboardInput`, `ElementState`, logical keys, `NamedKey`, and `request_redraw` usage.
    - Options Considered:
      - Use physical key codes: useful for games but wrong for text because it ignores keyboard layout.
      - Use logical/text input from winit events: better for basic text entry and keyboard layout awareness.
    - Chosen Approach:
      - Handle key press events, inspect logical named keys for Escape/Backspace, append printable text when provided by the key event, mutate `EditorBuffer`, and request redraw.
    - API Notes and Examples:
      ```rust
      use winit::event::{ElementState, WindowEvent};
      use winit::keyboard::{Key, NamedKey};

      match event {
          WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
              match &event.logical_key {
                  Key::Named(NamedKey::Escape) => event_loop.exit(),
                  Key::Named(NamedKey::Backspace) => editor.backspace(),
                  Key::Character(text) => editor.insert_str(text),
                  _ => {}
              }
              window.request_redraw();
          }
          _ => {}
      }
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Add keyboard-to-editor wiring and redraw requests.
    - References:
      - Context7 docs response for `/rust-windowing/winit` keyboard input and event-loop examples.
  - Test Cases to Write:
    - Unit tests remain on `EditorBuffer`; GUI input is covered by manual smoke testing.
    - Manual smoke test: Type `abc`, confirm displayed text includes `abc`; press Backspace, confirm `ab`; press Escape, confirm exit.

- [ ] Introduce a minimal Masonry editor module boundary
  - Acceptance Criteria:
    - Functional: A minimal Masonry-backed app/window or custom widget boundary owns event/widget routing, while the custom editor module owns `crop` state, Parley layout, and Vello painting.
    - Performance: Masonry integration does not introduce a busy redraw loop or recreate editor layout/render state unnecessarily.
    - Code Quality: The boundary between Masonry widget/application driver code and editor rendering/model code is explicit enough to evolve into Clay's client canvas.
    - Security: Masonry integration remains local-only and does not add IPC, extension loading, file access, or network access.
  - Approach:
    - Documentation Reviewed:
      - `masonry`/`masonry_winit` docs.rs examples via code search: Masonry owns a platform-independent widget tree; `masonry_winit::app::run_with` runs windows; `AppDriver::on_action` handles widget actions; Masonry is designed for Vello/wgpu rendering.
      - Project `concept.md`: Masonry is the client widget engine, Vello is the renderer, Parley is the text stack, and the custom text canvas is the hard integration point.
    - Options Considered:
      - Use Masonry's built-in `TextInput`: proves Masonry works, but not the custom text editor module with `crop`/Parley/Vello.
      - Keep raw `winit` forever: proves Vello/Parley/crop but avoids Clay's intended retained widget/event system.
      - Add a minimal custom editor module boundary after direct Vello/Parley works: isolates renderer/text issues first, then proves Masonry wiring.
    - Chosen Approach:
      - After direct Vello text rendering is stable, introduce the smallest Masonry boundary that can host or wrap the editor module. Prefer a custom widget if the pinned API is tractable; otherwise document the exact API blocker and use a Masonry shell with the editor module kept as a clearly separated component for the next iteration.
    - API Notes and Examples:
      ```rust
      use masonry::core::{ErasedAction, NewWidget, Widget, WidgetId};
      use masonry::theme::default_property_set;
      use masonry_winit::app::{AppDriver, DriverCtx, NewWindow};

      impl AppDriver for Driver {
          fn on_action(&mut self, _window_id: WindowId, _ctx: &mut DriverCtx<'_, '_>, _widget_id: WidgetId, _action: ErasedAction) {}
      }
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Either migrate to a minimal `masonry_winit` app shell or introduce a clearly separated editor module that is ready to be hosted by Masonry.
      - Additional module files under `src/` may be created if the editor model/rendering code becomes too large for `main.rs`.
    - References:
      - Docs.rs/code-search results for `masonry` 0.4.0 and `masonry_winit` 0.4.0 examples.
      - `concept.md` Phase 2 and Phase 3 roadmap.
  - Test Cases to Write:
    - Manual smoke test: Window still opens, renders editor text, accepts typing/backspace, and exits.
    - Build check: Run `cargo check` after Masonry integration.

- [ ] Verify and document Phase 0 completion
  - Acceptance Criteria:
    - Functional: The final Phase 0 prototype visibly edits rope-backed text through Vello/Parley and has a documented Masonry integration boundary.
    - Performance: No deliberate busy-loop redraw is left in place; redraws occur on startup, resize, and text edits.
    - Code Quality: `cargo fmt`, `cargo test`, and `cargo check` pass; model behavior is covered by unit tests.
    - Security: The completed phase still has no IPC listener, no JS/V8 execution, no extension loading, and no external file/network access.
  - Approach:
    - Documentation Reviewed:
      - Cargo command behavior for `cargo fmt`, `cargo test`, and `cargo check` is standard Rust tooling; no external API lookup needed.
    - Options Considered:
      - Stop after visual smoke test: insufficient because API correctness and model behavior should be checked automatically.
      - Run formatting, tests, and check: gives a clear baseline before IPC/server work.
    - Chosen Approach:
      - Run `cargo fmt`, `cargo test`, `cargo check`, and a manual GUI smoke test; update this plan's checkboxes only after implementation and verification pass.
    - API Notes and Examples:
      ```bash
      cargo fmt
      cargo test
      cargo check
      cargo run
      ```
    - Files to Create/Edit:
      - `plans/001-Phase0-NativeTextCanvas.md`: Mark completed tasks and fill final post-implementation notes.
    - References:
      - Repository `Cargo.toml`, `concept.md`, and current plan acceptance criteria.
  - Test Cases to Write:
    - `cargo test`: Runs unit tests for rope-backed editor behavior.
    - Manual GUI smoke test: Confirms launch, Vello render, visible text, typing, backspace, resize, and exit behavior.

## Compromises Made
- The implementation uses `pollster` to block only during renderer initialization so the existing synchronous `winit` `ApplicationHandler` shell can stay small until later Masonry integration.
- `EditorBuffer::visible_text` currently materializes the entire prototype buffer into a `String`; this is acceptable for the Phase 0 visible-slice prototype but should become viewport/range based after the initial native text canvas path is proven.
- Parley layout is rebuilt on each redraw from the current visible text slice; the required `FontContext` and `LayoutContext` are reused, and finer dirty-state caching is deferred until editing, cursor, and viewport behavior exist.

## Further Actions
- Wire keyboard printable text and Backspace to `EditorBuffer` now that rendered text is visible.
- Introduce the Masonry boundary after direct `winit`/Vello/Parley/crop rendering and editing are verified together.
