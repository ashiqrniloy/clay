# Clay: Architecture Specification

## 1. The Vision: A Programmable Environment

Clay is not just a text editor; it is a general-purpose, AI-native application environment.

Out of the box, it provides a world-class text editor. However, its core identity is an infinitely extensible, malleable canvas. Because the UI is driven by an embedded JavaScript engine, AI agents and users can mold Clay into a specialized IDE, a Kanban board, a visual node graph, or a LaTeX renderer on the fly.

## 2. High-Level Architecture Pattern

Clay utilizes a **Thick Client / Asynchronous Server** model communicating over local Inter-Process Communication (IPC).

- **The Server (The Brain):** An asynchronous Rust binary that acts as the authoritative source of truth. It can be run locally or inside remote Docker/SSH environments.
- **The Client (The Canvas):** A bespoke, highly optimized GUI running natively on the host OS. It relies entirely on Server-Driven UI (SDUI) and optimistic updates to ensure zero-latency interactions.

## 3. Technology Stack & Libraries

### The Client (The Canvas)

- **Widget Engine:** `masonry` (Linebender). Provides the foundational widget tree, handles OS event routing (via `winit`), hit-testing, and layout (via `taffy`). It is completely unopinionated about state.
- **Graphics Renderer:** `vello` (Linebender). A state-of-the-art compute-shader 2D vector graphics engine. It talks to the GPU (via `wgpu`) to draw pixel-perfect text, complex mathematical shapes (LaTeX), and dynamic UI panels.
- **Text Layout:** `parley` (Linebender). A rich text engine that handles bi-directional text, font fallback, and translates strings into glyph coordinates for `vello`.

### The Server (The Brain)

- **Core Data Structure:** `crop`. An ultra-fast, B-Tree-based rope used to store and mutate massive text buffers or structured document states in memory.
- **Extension Engine:** `deno_core` (Embedded V8). Evaluates the user's `init.js` file on startup. It executes AI-generated JavaScript/TypeScript extensions natively, serving as the bridge between AI models and the Rust core.
- **Concurrency:** `tokio` asynchronous runtime and actor-model message passing (via `mpsc` channels) to isolate the V8 thread from the master file-system and network threads.

### The Bridge (Internal Communication)

- **Transport:** Raw Unix Domain Sockets (Linux/macOS) / Named Pipes (Windows).
- **Serialization:** `rkyv`. A zero-copy serialization framework. It allows the UI to read massive text chunks and Server-Driven UI (SDUI) payloads directly from memory buffers in `O(1)` time, without allocating new memory.

## 4. Data, UI, & State Management

### Server-Driven UI (SDUI)

Because Clay is a do-it-all app, the Client must not be hardcoded to just edit text.

- **The Philosophy:**
  - The Client owns high-frequency local state (scrolling, cursor blinking, window resizing).
  - The Server owns the logic and the layout of dynamic applications.
- **The Flow:**
  - When an AI agent generates a new tool (e.g., a custom dashboard), the V8 thread constructs a declarative `rkyv` payload.
  - The Client receives this payload and dynamically maps it to native `masonry` widgets, painting them with `vello`.

### Text & Document Synchronization

- **The Canonical State:** The Server holds the authoritative `crop` rope.
- **The Shadow State:** The Client holds a lightweight shadow copy of the visible `crop` rope to enable optimistic, zero-latency typing.
- **Conflict Resolution:** Managed via **Version Tracking & Region Locking**.
  - The Server increments a version ID on every mutation.
  - Stale AI edits are rejected.
  - If an AI agent is performing a heavy rewrite, the Server places a *Region Lock* on that specific byte range, temporarily preventing conflicting user input in that block.

## 5. Extensibility: The `init.js` Paradigm

Clay does not have a static configuration file (like JSON). Its entry point is an `init.js` (or `.ts`) file executed by the embedded V8 engine on startup.

- **Turing-Complete Config:** Users and AI agents can write logical configuration (e.g., fetching data, conditionally rendering themes).
- **Native Hooks:** The Rust core exposes its internal APIs (buffer management, Vello painting contexts) directly to the V8 global scope. AI agents can natively invoke Rust functions to mutate the app's behavior in real time.

## 6. Development Roadmap

Building Clay requires strict, iterative milestones to manage the complexity of the Linebender rendering stack.

1. **Phase 1: The IPC Bridge**
   - Scaffold the `tokio` Server and `winit` Client.
   - Wire up a local IPC socket and verify zero-copy communication using `rkyv`.
2. **Phase 2: The Masonry Shell**
   - Integrate `masonry` into the Client.
   - Render a basic window and ensure OS events (resizing, closing) are routing properly.
3. **Phase 3: The Virtualized Canvas**
   - The hardest phase.
   - Write a custom `masonry` widget that introduces the `crop` rope, maps window bounds to string slices, feeds them to `parley`, and paints them with `vello`.
   - Implement cursor hit-testing and optimistic typing math.
4. **Phase 4: State Synchronization**
   - Connect the UI to the Server.
   - Implement the version-tracked diffing logic so the Client can send keystrokes to the canonical rope.
5. **Phase 5: The V8 Brain**
   - Embed `deno_core` in a dedicated background thread.
   - Expose the first Rust APIs to JavaScript, evaluate `init.js`, and allow an AI command to dynamically spawn a new SDUI widget on the Client canvas.
